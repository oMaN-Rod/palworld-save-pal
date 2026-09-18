//! Whether a target's game or server is running, answered once and
//! asynchronously so the synchronous `RunningCheck` the deployer takes can be
//! handed a settled snapshot.
use ps_db::mod_targets::ModTarget;

use super::deploy::RunningCheck;
use crate::services::ServerServices;

pub struct Snapshot(pub bool);

impl RunningCheck for Snapshot {
    fn is_running(&self, _target: &ModTarget) -> bool {
        self.0
    }
}

/// The executables a Palworld client runs as. A prefix match, because the
/// Steam build is `Palworld-Win64-Shipping.exe` and a `-Cmd` variant exists.
pub fn is_game_process_name(name: &str) -> bool {
    let name = name.to_ascii_lowercase();
    name.starts_with("palworld-win64-shipping")
        || name.starts_with("palworld-wingdk-shipping")
        || name == "palworld.exe"
}

pub async fn target_is_running(
    services: &ServerServices,
    db: &dyn ps_db::DbDriver,
    target: &ModTarget,
) -> Result<bool, ps_db::DbError> {
    if let Some(running) = services.running_override {
        return Ok(running);
    }
    if let Some(server_id) = target.server_id {
        let Some(record) = ps_db::servers::get_server(db, server_id).await? else {
            return Ok(false);
        };
        // No status at all means the container could not be inspected, which
        // gates writes as surely as a running one.
        return Ok(crate::servers_handlers::server_status(services, &record)
            .await
            .map(|status| status.running)
            .unwrap_or(true));
    }
    Ok(client_game_running().await)
}

/// A process scan that could not complete reads as running: this answer gates
/// writes into the game directory, so it must not fail open.
async fn client_game_running() -> bool {
    tokio::task::spawn_blocking(|| {
        crate::services::native_process::any_process_named(is_game_process_name)
    })
    .await
    .unwrap_or(true)
}

#[cfg(test)]
mod tests {
    use super::{is_game_process_name, target_is_running};
    use crate::servers_handlers::test_env::TestEnv;

    #[tokio::test]
    async fn a_docker_server_that_cannot_be_inspected_counts_as_running() {
        let env = TestEnv::new().await;
        let db = &*env.app.driver;
        let record = ps_db::servers::create_server(
            db,
            ps_db::servers::NewServer {
                name: "Alpha".to_string(),
                container_name: "alpha".to_string(),
                server_type: "docker".to_string(),
                mods_path: "/srv/alpha/mods".to_string(),
                logicmods_path: "/srv/alpha/logicmods".to_string(),
                nativemods_path: "/srv/alpha/nativemods".to_string(),
                ..Default::default()
            },
        )
        .await
        .unwrap();
        let target_id = ps_db::mod_targets::ensure_server_target(db, &record, "/srv")
            .await
            .unwrap()
            .unwrap();
        let target = ps_db::mod_targets::get(db, &target_id)
            .await
            .unwrap()
            .unwrap();

        assert!(
            !target_is_running(&env.services, db, &target).await.unwrap(),
            "a container that does not exist is not running"
        );
        env.docker
            .fail_inspect
            .lock()
            .unwrap()
            .insert("alpha".to_string());
        assert!(target_is_running(&env.services, db, &target).await.unwrap());
    }

    #[test]
    fn client_executables_match_in_any_case() {
        for name in [
            "Palworld-Win64-Shipping.exe",
            "palworld-win64-shipping-cmd.exe",
            "PALWORLD-WIN64-SHIPPING-CMD.EXE",
            "Palworld-WinGDK-Shipping.exe",
            "palworld-wingdk-shipping.exe",
        ] {
            assert!(is_game_process_name(name), "{name}");
        }
    }

    #[test]
    fn servers_and_unrelated_processes_do_not_match() {
        for name in [
            "PalServer.exe",
            "PalServer-Win64-Shipping-Cmd.exe",
            "steam.exe",
        ] {
            assert!(!is_game_process_name(name), "{name}");
        }
    }
}
