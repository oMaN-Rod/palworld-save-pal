use std::net::IpAddr;
use std::path::PathBuf;

use clap::Parser;

use ps_server::{start_server, ServerConfig};

/// PalStudio server.
#[derive(Parser, Debug)]
#[command(name = "ps-server", version)]
struct Cli {
    /// Host to bind (web default 0.0.0.0; desktop uses 127.0.0.1). Listen
    /// MODES (localhost/lan/tailscale/wan) are enforced per-peer by the
    /// network policy, not by this address.
    #[arg(long, default_value = "0.0.0.0")]
    host: IpAddr,
    /// Port to run the server on. Omitted, the port from the network policy
    /// (Network page / PS_PORT) applies, so the UI can change it later.
    #[arg(long)]
    port: Option<u16>,
    /// Directory containing json/ game data.
    #[arg(long, default_value = "data")]
    data_dir: PathBuf,
    /// Directory containing the built SvelteKit UI.
    #[arg(long, default_value = "ui")]
    ui_dir: PathBuf,
    /// SQLite database file.
    #[arg(long, default_value = "ps-rs.db")]
    db: PathBuf,
    /// Development mode (debug logging).
    #[arg(long)]
    dev: bool,
    /// Mark this as a network-facing (hosted) context with the full
    /// network policy; the Docker image passes it in its CMD.
    #[arg(long)]
    hosted: bool,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();
    // sqlx logs every executed statement at DEBUG, which turns the background
    // pollers (the 3s bridge reconciler alone is two queries per tick) into
    // console spam under --dev. Keep app-level debug, but drop per-query
    // logs; RUST_LOG still overrides everything for deep debugging.
    let default_filter = if cli.dev {
        "debug,sqlx::query=warn"
    } else {
        "info,sqlx::query=warn"
    };
    let filter = tracing_subscriber::EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new(default_filter));
    tracing_subscriber::fmt().with_env_filter(filter).init();

    let config = ServerConfig {
        host: cli.host,
        port: cli.port,
        ui_dir: cli.ui_dir,
        data_dir: cli.data_dir,
        db_path: cli.db,
        desktop_mode: false,
        hosted: cli.hosted,
    };
    loop {
        let handle = start_server(config.clone()).await?;
        tracing::info!("ps-server listening on http://{}", handle.addr);
        // The listener ends on external shutdown, a Network-page port
        // change, or a runtime-mode switch; services are torn down inside
        // wait_or_restart either way.
        match handle.wait_or_restart().await {
            ps_server::ListenerExit::RebindRequested => {
                tracing::info!("network policy requested a rebind; restarting listener");
                // With no CLI port pin, start_server picks the newly
                // configured port from the network policy.
            }
            ps_server::ListenerExit::ExitRequested => {
                tracing::info!("runtime mode switched; exiting");
                return Ok(());
            }
            ps_server::ListenerExit::Stopped => return Ok(()),
            ps_server::ListenerExit::Failed(error) => {
                return Err(anyhow::anyhow!("ps-server listener failed: {error}"));
            }
        }
    }
}
