//! Which mod target a bridge instance belongs to: an auto-discovered instance
//! through its game process's executable, a saved one through its stored target.
use std::path::{Component, Path, PathBuf};

use ps_db::mod_targets::ModTarget;

const CASE_INSENSITIVE: bool = cfg!(any(windows, target_os = "macos"));

pub fn ancestor_target<'a>(exe: &Path, targets: &'a [ModTarget]) -> Option<&'a ModTarget> {
    if !exe.is_absolute()
        || exe
            .components()
            .any(|c| matches!(c, Component::ParentDir | Component::CurDir))
    {
        return None;
    }
    let exe_key = ps_core::mods::normalize_physical_path(&exe.to_string_lossy(), CASE_INSENSITIVE);
    targets
        .iter()
        .filter(|target| target.kind == "client")
        .filter_map(|target| {
            let root = ps_core::mods::normalize_physical_path(&target.root_path, CASE_INSENSITIVE);
            if root.is_empty() {
                return None;
            }
            let under = exe_key
                .strip_prefix(&root)
                .is_some_and(|rest| rest.starts_with('/'));
            under.then_some((root.len(), target))
        })
        .max_by_key(|(length, _)| *length)
        .map(|(_, target)| target)
}

/// Same rule as [`bound_target_id`]'s `"auto"` arm, but takes an
/// already-loaded target list so a caller resolving many auto instances in one
/// reply loads it once instead of once per instance.
pub fn bound_auto_target(
    pid: u32,
    targets: &[ModTarget],
    exe_of: impl Fn(u32) -> Option<PathBuf>,
) -> Option<&ModTarget> {
    let exe = exe_of(pid)?;
    ancestor_target(&exe, targets)
}

pub fn process_exe(pid: u32) -> Option<PathBuf> {
    let pid = sysinfo::Pid::from_u32(pid);
    let mut system = sysinfo::System::new();
    system.refresh_processes_specifics(
        sysinfo::ProcessesToUpdate::Some(&[pid]),
        true,
        sysinfo::ProcessRefreshKind::nothing().with_exe(sysinfo::UpdateKind::OnlyIfNotSet),
    );
    system.process(pid)?.exe().map(Path::to_path_buf)
}

pub async fn bound_target_id(
    db: &dyn ps_db::DbDriver,
    instance_id: &str,
    exe_of: impl Fn(u32) -> Option<PathBuf>,
) -> Result<Option<String>, ps_db::DbError> {
    let Some((source, rest)) = instance_id.split_once(':') else {
        return Ok(None);
    };
    match source {
        "auto" => {
            let Ok(pid) = rest.parse::<u32>() else {
                return Ok(None);
            };
            let targets = ps_db::mod_targets::list(db).await?;
            Ok(bound_auto_target(pid, &targets, exe_of).map(|target| target.id.clone()))
        }
        "saved" => {
            let Ok(row) = rest.parse::<i64>() else {
                return Ok(None);
            };
            let Some(instance) = ps_db::amity_instances::get_instance(db, row).await? else {
                return Ok(None);
            };
            match instance.target_id {
                Some(id) if ps_db::mod_targets::get(db, &id).await?.is_some() => Ok(Some(id)),
                _ => Ok(None),
            }
        }
        _ => Ok(None),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn target(id: &str, kind: &str, root_path: &str) -> ModTarget {
        ModTarget {
            id: id.to_string(),
            kind: kind.to_string(),
            server_id: None,
            name: String::new(),
            root_path: root_path.to_string(),
            platform: String::new(),
            ue4ss_mode: String::new(),
            layout_overrides: String::new(),
            detected: String::new(),
            last_scanned_at: None,
            created_at: String::new(),
            updated_at: String::new(),
        }
    }

    /// A fixed absolute base under the platform's own temp directory, so tests
    /// build genuinely absolute paths on every OS instead of a Windows-only
    /// `C:/...` literal (`Path::is_absolute()` is false for those on Unix).
    fn base_dir() -> PathBuf {
        std::env::temp_dir().join("ps-binding-test")
    }

    #[test]
    fn the_longest_ancestor_root_wins() {
        let base = base_dir();
        let exe = base.join("Games/Palworld/Pal/Binaries/Win64/Palworld-Win64-Shipping.exe");
        let games_root = base.join("Games");
        let palworld_root = base.join("Games/Palworld");
        let targets = [
            target("games", "client", &games_root.to_string_lossy()),
            target("palworld", "client", &palworld_root.to_string_lossy()),
        ];
        let found = ancestor_target(&exe, &targets).unwrap();
        assert_eq!(found.id, "palworld");
    }

    #[test]
    fn a_sibling_prefix_and_a_server_target_are_never_returned() {
        let base = base_dir();
        let exe = base.join("Games/Palworld/Pal/Binaries/Win64/Palworld-Win64-Shipping.exe");
        let sibling_root = base.join("Games/Pal");
        let palworld_root = base.join("Games/Palworld");
        let targets = [
            target("pal-sibling", "client", &sibling_root.to_string_lossy()),
            target("server", "server", &palworld_root.to_string_lossy()),
        ];
        assert!(ancestor_target(&exe, &targets).is_none());
    }

    #[test]
    fn an_empty_root_path_is_never_returned() {
        let base = base_dir();
        let exe = base.join("Games/Palworld/Pal/Binaries/Win64/Palworld-Win64-Shipping.exe");
        let targets = [target("empty-root", "client", "")];
        assert!(ancestor_target(&exe, &targets).is_none());
    }

    #[cfg(windows)]
    #[test]
    fn matching_is_case_insensitive_on_windows() {
        let exe = Path::new("C:/Games/Palworld/x.exe");
        let targets = [target("palworld", "client", "c:/games/palworld")];
        let found = ancestor_target(exe, &targets).unwrap();
        assert_eq!(found.id, "palworld");
    }

    #[test]
    fn bound_auto_target_resolves_against_an_already_loaded_list() {
        let base = base_dir();
        let exe = base.join("Games/Palworld/Pal/Binaries/Win64/Palworld-Win64-Shipping.exe");
        let palworld_root = base.join("Games/Palworld");
        let targets = [target(
            "palworld",
            "client",
            &palworld_root.to_string_lossy(),
        )];

        let exe_of = {
            let exe = exe.clone();
            move |pid: u32| {
                assert_eq!(pid, 4242);
                Some(exe.clone())
            }
        };
        let found = bound_auto_target(4242, &targets, exe_of).unwrap();
        assert_eq!(found.id, "palworld");

        assert!(bound_auto_target(4242, &targets, |_| None).is_none());
    }

    async fn test_driver() -> ps_db::SqlxSqliteDriver {
        let dir = tempfile::tempdir().unwrap();
        let pool = ps_db::open(&dir.path().join("ps-rs.db")).await.unwrap();
        std::mem::forget(dir);
        ps_db::SqlxSqliteDriver::new(pool)
    }

    fn new_target(id: &str, kind: &str, root_path: &str) -> ps_db::mod_targets::NewModTarget {
        ps_db::mod_targets::NewModTarget {
            id: id.to_string(),
            kind: kind.to_string(),
            server_id: None,
            name: id.to_string(),
            root_path: root_path.to_string(),
            platform: "win64".to_string(),
            ue4ss_mode: "standard".to_string(),
            layout_overrides: "{}".to_string(),
            detected: "{}".to_string(),
        }
    }

    #[tokio::test]
    async fn auto_id_resolves_through_the_process_executable() {
        let db = test_driver().await;
        ps_db::mod_targets::upsert(&db, &new_target("palworld", "client", "C:/Games/Palworld"))
            .await
            .unwrap();

        let exe_of = |pid: u32| {
            assert_eq!(pid, 4242);
            Some(PathBuf::from(
                "C:/Games/Palworld/Pal/Binaries/Win64/Palworld-Win64-Shipping.exe",
            ))
        };
        let found = bound_target_id(&db, "auto:4242", exe_of).await.unwrap();
        assert_eq!(found.as_deref(), Some("palworld"));
    }

    #[tokio::test]
    async fn auto_id_with_no_process_executable_is_unbound() {
        let db = test_driver().await;
        let found = bound_target_id(&db, "auto:4242", |_| None).await.unwrap();
        assert!(found.is_none());
    }

    #[tokio::test]
    async fn a_saved_instance_resolves_through_its_stored_target_until_it_is_removed() {
        let db = test_driver().await;
        ps_db::mod_targets::upsert(&db, &new_target("palworld", "client", "C:/Games/Palworld"))
            .await
            .unwrap();
        let row = ps_db::amity_instances::insert_instance(
            &db,
            &ps_db::amity_instances::NewAmityInstance {
                name: "Box".to_string(),
                host: "10.0.0.1".to_string(),
                port: 8788,
                token: "t".to_string(),
            },
        )
        .await
        .unwrap();
        ps_db::amity_instances::set_target(&db, row, Some("palworld"))
            .await
            .unwrap();

        let id = format!("saved:{row}");
        let found = bound_target_id(&db, &id, |_| None).await.unwrap();
        assert_eq!(found.as_deref(), Some("palworld"));

        ps_db::mod_targets::remove(&db, "palworld").await.unwrap();
        let found = bound_target_id(&db, &id, |_| None).await.unwrap();
        assert!(found.is_none());
    }

    #[tokio::test]
    async fn an_unknown_saved_row_or_source_is_unbound() {
        let db = test_driver().await;
        assert!(bound_target_id(&db, "saved:999", |_| None)
            .await
            .unwrap()
            .is_none());
        assert!(bound_target_id(&db, "other:1", |_| None)
            .await
            .unwrap()
            .is_none());
    }
}
