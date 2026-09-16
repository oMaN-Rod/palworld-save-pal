use std::net::IpAddr;
use std::path::PathBuf;

use clap::{Parser, Subcommand};

mod bootstrap;

use bootstrap::{default_data_home, AssetPaths};

/// PalStudio launcher/server. Subcommands pick the mode; with none given, an
/// interactive terminal asks once (choice remembered), otherwise the webapp
/// runs — which is also what background services install.
#[derive(Parser, Debug)]
#[command(name = "palstudio", version)]
struct Cli {
    #[command(subcommand)]
    command: Option<Mode>,
}

#[derive(Subcommand, Debug, Clone)]
enum Mode {
    /// Run the network-facing server (full network settings) — the verb
    /// background services use.
    Serve(ServerArgs),
    /// Alias of serve for humans: "host this instance for others".
    Host(ServerArgs),
    /// Run a LOCAL webapp (localhost-only; the Network page offers just the
    /// port) and open it in your browser.
    Webapp(ServerArgs),
    /// Launch the desktop app (bundled as bin/palstudio-desktop when the
    /// install included it).
    Desktop,
}

#[derive(clap::Args, Debug, Clone)]
struct ServerArgs {
    /// Host to bind. Listen MODES (localhost/lan/tailscale/wan) are enforced
    /// per-peer by the network policy — this is only the socket address.
    #[arg(long, default_value = "127.0.0.1")]
    host: IpAddr,
    /// Port to run the server on. Omitted, the network policy's port
    /// (Network page / PS_PORT) applies.
    #[arg(long)]
    port: Option<u16>,
    /// Root directory holding ui/, data/ and the database (default: the
    /// platform data home, e.g. ~/.local/share/palstudio).
    #[arg(long)]
    data_home: Option<PathBuf>,
    /// Directory containing the built SvelteKit UI. Skips provisioning when
    /// given together with --data-dir.
    #[arg(long)]
    ui_dir: Option<PathBuf>,
    /// Directory containing json/ game data. Skips provisioning when given
    /// together with --ui-dir.
    #[arg(long)]
    data_dir: Option<PathBuf>,
    /// SQLite database file.
    #[arg(long)]
    db: Option<PathBuf>,
    /// Development mode (debug logging).
    #[arg(long)]
    dev: bool,
}

impl Default for ServerArgs {
    fn default() -> Self {
        ServerArgs {
            host: IpAddr::from([127, 0, 0, 1]),
            port: None,
            data_home: None,
            ui_dir: None,
            data_dir: None,
            db: None,
            dev: false,
        }
    }
}

/// Where the remembered launch mode lives: <data-home>/launcher.json.
fn launcher_pref_path(data_home: &std::path::Path) -> std::path::PathBuf {
    data_home.join("launcher.json")
}

enum LaunchChoice {
    Desktop,
    Webapp,
}

fn read_preferred_mode(data_home: &std::path::Path) -> Option<LaunchChoice> {
    let raw = std::fs::read_to_string(launcher_pref_path(data_home)).ok()?;
    match serde_json::from_str::<serde_json::Value>(&raw).ok()?["mode"].as_str()? {
        "desktop" => Some(LaunchChoice::Desktop),
        "webapp" => Some(LaunchChoice::Webapp),
        _ => None,
    }
}

fn write_preferred_mode(data_home: &std::path::Path, mode: &str) {
    let path = launcher_pref_path(data_home);
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let _ = std::fs::write(&path, serde_json::json!({ "mode": mode }).to_string());
}

/// A prompt is only meaningful on a real terminal; `curl | sh` installers
/// and services must never block on stdin.
fn stdin_is_tty() -> bool {
    use std::io::IsTerminal;
    std::io::stdin().is_terminal()
}

/// Whether this install carries the desktop app (bin/palstudio-desktop
/// next to this launcher) — decides the non-interactive default.
fn desktop_binary_available() -> bool {
    let Ok(exe) = std::env::current_exe() else {
        return false;
    };
    let Some(install_dir) = exe.parent().and_then(|bin| bin.parent()) else {
        return false;
    };
    let name = if cfg!(windows) {
        "palstudio-desktop.exe"
    } else {
        "palstudio-desktop"
    };
    install_dir.join("bin").join(name).is_file()
}

fn ask_mode_once(data_home: &std::path::Path) -> LaunchChoice {
    if let Some(mode) = read_preferred_mode(data_home) {
        return mode;
    }
    if stdin_is_tty() {
        println!("How should PalStudio start?");
        println!("  1) desktop — opens the PalStudio desktop window (default)");
        println!("  2) webapp  — runs a local server and opens your browser");
        println!(
            "(The choice is remembered in {}.)",
            launcher_pref_path(data_home).display()
        );
        print!("Choice [1]: ");
        use std::io::Write;
        let _ = std::io::stdout().flush();
        let mut answer = String::new();
        let _ = std::io::stdin().read_line(&mut answer);
        match answer.trim() {
            "2" | "w" | "webapp" => {
                write_preferred_mode(data_home, "webapp");
                LaunchChoice::Webapp
            }
            _ => {
                write_preferred_mode(data_home, "desktop");
                LaunchChoice::Desktop
            }
        }
    } else {
        // Non-interactive: prefer the desktop app when the install carries
        // it; headless/server bundles fall back to the webapp.
        if desktop_binary_available() {
            LaunchChoice::Desktop
        } else {
            LaunchChoice::Webapp
        }
    }
}

/// The desktop binary ships beside this one in the install bundle; run it
/// with the install dir as cwd so its unpackaged asset fallback (ui/ + data/
/// next to the binary) resolves.
fn launch_desktop() -> anyhow::Result<()> {
    let exe = std::env::current_exe()?;
    let install_dir = exe
        .parent()
        .and_then(|bin| bin.parent())
        .ok_or_else(|| anyhow::anyhow!("could not locate the install directory"))?;
    let name = if cfg!(windows) {
        "palstudio-desktop.exe"
    } else {
        "palstudio-desktop"
    };
    let desktop = install_dir.join("bin").join(name);
    anyhow::ensure!(
        desktop.is_file(),
        "the desktop app ({}) is not part of this install — the bundle was built without it.\n\
         Use `palstudio webapp`, or install the desktop app from the releases page:\n\
         https://github.com/oMaN-Rod/palworld-save-pal/releases",
        desktop.display()
    );
    let status = std::process::Command::new(&desktop)
        .current_dir(install_dir)
        .status()
        .map_err(|error| anyhow::anyhow!("could not launch {}: {error}", desktop.display()))?;
    if !status.success() {
        // Mirror the child's exit code so shells and services see it.
        std::process::exit(status.code().unwrap_or(1));
    }
    Ok(())
}

/// A portable install layout carried next to the launcher binary. The
/// Windows standalone zip (`bin/ + ui_build/ + data/`) and the server tarball
/// (`bin/ + ui/ + data/`) both ship the UI and game data, so server modes use
/// them in place instead of provisioning a second copy into the data home.
/// The database lives at the install root too ("alongside the exe").
struct BundledInstall {
    assets: AssetPaths,
    db_path: PathBuf,
}

/// The install layout rooted at `root`, when it actually carries a built UI
/// and the game data; anything less is not a complete bundle.
fn bundled_install_at(root: &std::path::Path) -> Option<BundledInstall> {
    let ui_dir = ["ui_build", "ui"]
        .iter()
        .map(|name| root.join(name))
        .find(|dir| dir.join("index.html").is_file())?;
    let data_dir = root.join("data");
    if !data_dir.join("json").is_dir() {
        return None;
    }
    Some(BundledInstall {
        assets: AssetPaths { ui_dir, data_dir },
        db_path: root.join("ps-rs.db"),
    })
}

/// Resolves the bundled layout around the given launcher path: `bin/palstudio`
/// looks one level up (the zip/tarball layout), a bare binary looks beside
/// itself.
fn bundled_install_for(exe: &std::path::Path) -> Option<BundledInstall> {
    let dir = exe.parent()?;
    let root = if dir.file_name().is_some_and(|name| name == "bin") {
        dir.parent()?
    } else {
        dir
    };
    bundled_install_at(root)
}

fn bundled_install() -> Option<BundledInstall> {
    bundled_install_for(&std::env::current_exe().ok()?)
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();
    let dev = cli.command.as_ref().is_some_and(|mode| match mode {
        Mode::Serve(args) | Mode::Host(args) | Mode::Webapp(args) => args.dev,
        Mode::Desktop => false,
    });
    tracing_subscriber::fmt()
        .with_max_level(if dev {
            tracing::Level::DEBUG
        } else {
            tracing::Level::INFO
        })
        .init();

    match cli.command {
        // Serve/host and services are network-facing contexts: the full
        // network policy applies. The webapp is a hand-launched local tool.
        Some(Mode::Serve(args)) | Some(Mode::Host(args)) => {
            let data_home = args.data_home.clone().map_or_else(default_data_home, Ok)?;
            run_server(&args, &data_home, false, true).await
        }
        Some(Mode::Webapp(args)) => {
            let data_home = args.data_home.clone().map_or_else(default_data_home, Ok)?;
            write_preferred_mode(&data_home, "webapp");
            run_server(&args, &data_home, true, false).await
        }
        Some(Mode::Desktop) => launch_desktop(),
        None => {
            let data_home = default_data_home()?;
            match ask_mode_once(&data_home) {
                LaunchChoice::Desktop => launch_desktop(),
                LaunchChoice::Webapp => {
                    write_preferred_mode(&data_home, "webapp");
                    run_server(&ServerArgs::default(), &data_home, true, false).await
                }
            }
        }
    }
}

#[allow(clippy::too_many_arguments)]
async fn run_server(
    args: &ServerArgs,
    data_home: &std::path::Path,
    mut open_browser: bool,
    hosted: bool,
) -> anyhow::Result<()> {
    let (assets, bundled_db) = match (args.ui_dir.clone(), args.data_dir.clone()) {
        // Explicit dirs mean the operator owns the layout; do not touch them.
        (Some(ui_dir), Some(data_dir)) => (AssetPaths { ui_dir, data_dir }, None),
        // Half a pair is almost certainly a typo; error instead of silently
        // ignoring the given flag and falling back to bundled/provisioned
        // assets.
        (Some(_), None) | (None, Some(_)) => anyhow::bail!(
            "--ui-dir and --data-dir must be given together — pass both to use an \
             explicit layout, or neither to use the bundled/install assets"
        ),
        // A bundle layout around this binary wins over provisioning a fresh
        // copy into the data home.
        _ => match bundled_install() {
            Some(BundledInstall { assets, db_path }) => (assets, Some(db_path)),
            None => (bootstrap::ensure_assets(data_home).await?, None),
        },
    };
    let db_path = args
        .db
        .clone()
        .or(bundled_db)
        .unwrap_or_else(|| data_home.join("ps-rs.db"));

    let config = ps_server::ServerConfig {
        host: args.host,
        port: args.port,
        ui_dir: assets.ui_dir.clone(),
        data_dir: assets.data_dir.clone(),
        db_path: db_path.clone(),
        desktop_mode: false,
        hosted,
    };
    loop {
        let handle = ps_server::start_server(config.clone()).await?;
        tracing::info!(
            "PalStudio v{} listening on http://{} — ui {}, data {}, db {}",
            env!("CARGO_PKG_VERSION"),
            handle.addr,
            assets.ui_dir.display(),
            assets.data_dir.display(),
            db_path.display(),
        );
        if open_browser {
            // Best effort: a missing opener must not take the server down.
            let url = format!("http://{}/", handle.addr);
            if let Err(error) = opener::open(&url) {
                tracing::warn!(%error, "could not open the browser for {url}");
            }
        }
        // The listener ends on external shutdown, a Network-page port
        // change, or a runtime-mode switch; services are torn down inside
        // wait_or_restart either way. The browser follows the new address
        // on the next webapp launch.
        match handle.wait_or_restart().await {
            ps_server::ListenerExit::RebindRequested => {
                tracing::info!("network policy requested a rebind; restarting listener");
                open_browser = false;
                // With no CLI port pin, start_server picks the newly
                // configured port from the network policy.
            }
            ps_server::ListenerExit::ExitRequested => {
                tracing::info!("runtime mode switched; exiting");
                return Ok(());
            }
            ps_server::ListenerExit::Stopped => return Ok(()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{bundled_install_at, bundled_install_for};

    fn lay_out(root: &std::path::Path, ui_name: &str) {
        std::fs::create_dir_all(root.join(ui_name)).unwrap();
        std::fs::write(root.join(ui_name).join("index.html"), "<html></html>").unwrap();
        std::fs::create_dir_all(root.join("data").join("json")).unwrap();
    }

    #[test]
    fn recognizes_the_zip_layout_around_bin() {
        let root = tempfile::tempdir().expect("tempdir");
        lay_out(root.path(), "ui_build");
        // bin/palstudio → install root one level up.
        let install = bundled_install_for(&root.path().join("bin").join("palstudio"))
            .expect("bundled layout detected");
        assert_eq!(install.assets.ui_dir, root.path().join("ui_build"));
        assert_eq!(install.assets.data_dir, root.path().join("data"));
        assert_eq!(install.db_path, root.path().join("ps-rs.db"));
    }

    #[test]
    fn recognizes_the_tarball_spelling_ui_dir() {
        let root = tempfile::tempdir().expect("tempdir");
        lay_out(root.path(), "ui");
        let install =
            bundled_install_for(&root.path().join("bin").join("palstudio")).expect("detected");
        assert_eq!(install.assets.ui_dir, root.path().join("ui"));
    }

    #[test]
    fn a_bare_binary_uses_its_own_directory() {
        let root = tempfile::tempdir().expect("tempdir");
        lay_out(root.path(), "ui_build");
        assert!(bundled_install_for(&root.path().join("palstudio")).is_some());
    }

    #[test]
    fn incomplete_layouts_are_ignored() {
        let root = tempfile::tempdir().expect("tempdir");
        // UI but no game data.
        lay_out(root.path(), "ui_build");
        std::fs::remove_dir_all(root.path().join("data")).unwrap();
        assert!(bundled_install_at(root.path()).is_none());
        // Nothing at all.
        let empty = tempfile::tempdir().expect("tempdir");
        assert!(bundled_install_at(empty.path()).is_none());
    }
}
