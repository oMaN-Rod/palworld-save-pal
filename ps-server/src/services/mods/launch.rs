//! How a client target's game is started. The command is decided purely from
//! the target and its layout; `GameLauncher` performs it.
use std::path::PathBuf;

use ps_core::mods::TargetLayout;
use ps_db::mod_targets::ModTarget;

pub const STEAM_RUN_URL: &str = "steam://run/1623730";

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LaunchCommand {
    Open(String),
    Spawn {
        program: PathBuf,
        current_dir: PathBuf,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum LaunchError {
    #[error("only a game install can be launched")]
    NotAClient,
    #[error("launching is not supported on {0}")]
    UnsupportedPlatform(String),
    #[error("this install has no game executable to start")]
    Unavailable,
}

/// Palworld cannot be told which world to open, so every command starts the
/// game plainly.
pub fn launch_command(
    target: &ModTarget,
    layout: &TargetLayout,
) -> Result<LaunchCommand, LaunchError> {
    if target.kind != "client" {
        return Err(LaunchError::NotAClient);
    }
    match target.platform.as_str() {
        "win64" | "linux" => Ok(LaunchCommand::Open(STEAM_RUN_URL.to_string())),
        "wingdk" => match (&layout.executable, &layout.binaries_dir) {
            (Some(program), Some(dir)) => Ok(LaunchCommand::Spawn {
                program: program.clone(),
                current_dir: dir.clone(),
            }),
            _ => Err(LaunchError::Unavailable),
        },
        other => Err(LaunchError::UnsupportedPlatform(other.to_string())),
    }
}

pub trait GameLauncher: Send + Sync {
    fn launch(&self, command: &LaunchCommand) -> std::io::Result<()>;
}

pub struct SystemLauncher;

impl GameLauncher for SystemLauncher {
    fn launch(&self, command: &LaunchCommand) -> std::io::Result<()> {
        match command {
            LaunchCommand::Open(url) => opener::open(url).map_err(std::io::Error::other),
            LaunchCommand::Spawn {
                program,
                current_dir,
            } => std::process::Command::new(program)
                .current_dir(current_dir)
                .spawn()
                .map(|_| ()),
        }
    }
}

#[cfg(test)]
#[derive(Default)]
pub struct RecordingLauncher {
    pub launched: std::sync::Mutex<Vec<LaunchCommand>>,
    pub fail: std::sync::atomic::AtomicBool,
}

#[cfg(test)]
impl GameLauncher for RecordingLauncher {
    fn launch(&self, command: &LaunchCommand) -> std::io::Result<()> {
        if self.fail.load(std::sync::atomic::Ordering::SeqCst) {
            return Err(std::io::Error::other("refused by test"));
        }
        self.launched.lock().unwrap().push(command.clone());
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn target(kind: &str, platform: &str) -> ps_db::mod_targets::ModTarget {
        ps_db::mod_targets::ModTarget {
            id: "client-x".into(),
            kind: kind.into(),
            server_id: None,
            name: "x".into(),
            root_path: "/g".into(),
            platform: platform.into(),
            ue4ss_mode: "none".into(),
            layout_overrides: "{}".into(),
            detected: "{}".into(),
            last_scanned_at: None,
            created_at: String::new(),
            updated_at: String::new(),
        }
    }

    fn layout(executable: Option<&str>, binaries: Option<&str>) -> ps_core::mods::TargetLayout {
        ps_core::mods::TargetLayout {
            root: PathBuf::from("/g"),
            binaries_dir: binaries.map(PathBuf::from),
            executable: executable.map(PathBuf::from),
            ue4ss_dir: None,
            ue4ss_mods_dir: None,
            mods_txt: None,
            paks_mods_dir: PathBuf::from("/g/p"),
            logicmods_dir: PathBuf::from("/g/l"),
            palschema_mods_dir: None,
            nativemods_dir: None,
            palmodsettings_ini: None,
            workshop_local_dir: None,
        }
    }

    #[test]
    fn steam_clients_launch_through_the_steam_url() {
        for platform in ["win64", "linux"] {
            assert_eq!(
                launch_command(&target("client", platform), &layout(None, None)).unwrap(),
                LaunchCommand::Open(STEAM_RUN_URL.to_string())
            );
        }
    }

    #[test]
    fn game_pass_spawns_the_executable_from_the_binaries_dir() {
        let command = launch_command(
            &target("client", "wingdk"),
            &layout(
                Some("/g/Pal/Binaries/WinGDK/Palworld-WinGDK-Shipping.exe"),
                Some("/g/Pal/Binaries/WinGDK"),
            ),
        )
        .unwrap();
        assert_eq!(
            command,
            LaunchCommand::Spawn {
                program: PathBuf::from("/g/Pal/Binaries/WinGDK/Palworld-WinGDK-Shipping.exe"),
                current_dir: PathBuf::from("/g/Pal/Binaries/WinGDK"),
            }
        );
    }

    #[test]
    fn game_pass_without_an_executable_is_unavailable() {
        assert_eq!(
            launch_command(&target("client", "wingdk"), &layout(None, None)),
            Err(LaunchError::Unavailable)
        );
    }

    #[test]
    fn mac_and_servers_are_refused() {
        assert_eq!(
            launch_command(&target("client", "mac"), &layout(None, None)),
            Err(LaunchError::UnsupportedPlatform("mac".into()))
        );
        assert_eq!(
            launch_command(&target("server", "win64"), &layout(None, None)),
            Err(LaunchError::NotAClient)
        );
    }
}
