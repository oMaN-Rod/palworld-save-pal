use uuid::Uuid;

use psp_core::domain::blueprint::BaseBlueprint;

/// The most blueprints one connection holds at once. Capture-once-export-many
/// and load-then-place both need only a handful; the cap bounds memory.
const MAX_HANDLES: usize = 8;

#[derive(Default)]
pub struct BlueprintRegistry {
    entries: Vec<(Uuid, BaseBlueprint)>,
}

impl BlueprintRegistry {
    pub fn insert(&mut self, blueprint: BaseBlueprint) -> Uuid {
        if self.entries.len() >= MAX_HANDLES {
            self.entries.remove(0);
        }
        let handle = Uuid::new_v4();
        self.entries.push((handle, blueprint));
        handle
    }

    pub fn get(&self, handle: &Uuid) -> Option<&BaseBlueprint> {
        self.entries
            .iter()
            .find(|(id, _)| id == handle)
            .map(|(_, blueprint)| blueprint)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use psp_core::domain::blueprint::{capture, CaptureOptions};
    use psp_core::props;

    /// Loads the committed world1 Level.sav, captures its first base as a
    /// blueprint. world1 has a single base with 13 structures — enough to be a
    /// non-trivial value to store.
    fn a_blueprint() -> BaseBlueprint {
        let level_sav = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../tests/fixtures/saves/world1/Level.sav");
        let session = load_session(&level_sav);
        let base_id = session
            .base_camp_map()
            .and_then(|m| m.first())
            .and_then(|entry| props::as_uuid(&entry.key))
            .expect("world1 has a base camp");
        capture::capture(&session, base_id, CaptureOptions::blueprint(), "Home")
            .expect("capture world1 base")
    }

    /// Builds a SaveSession from a Level.sav path, the same entry point
    /// `handle_select_save` uses for the steam load path: no LevelMeta/WorldOption,
    /// no players, no top-level progress frame.
    fn load_session(level_sav: &std::path::Path) -> psp_core::session::SaveSession {
        let level_sav_bytes = std::fs::read(level_sav).expect("read world1 Level.sav");
        psp_core::session::SaveSession::load(
            psp_core::session::SaveKind::Steam {
                level_path: level_sav.to_path_buf(),
            },
            level_sav.to_string_lossy().into_owned(),
            "steam",
            &level_sav_bytes,
            None,
            None,
            std::collections::BTreeMap::new(),
            None,
            false,
            &psp_core::progress::null_progress(),
        )
        .expect("load world1 Level.sav")
    }

    #[test]
    fn insert_mints_distinct_handles_that_both_resolve() {
        let mut registry = BlueprintRegistry::default();
        let bp = a_blueprint();
        let a = registry.insert(bp.clone());
        let b = registry.insert(bp.clone());
        assert_ne!(a, b, "each insert mints a fresh handle");
        assert!(registry.get(&a).is_some(), "first handle still resolves");
        assert!(registry.get(&b).is_some(), "second handle resolves");
        assert!(registry.get(&Uuid::new_v4()).is_none(), "unknown handle does not");
    }

    #[test]
    fn the_oldest_handle_is_evicted_past_the_cap() {
        let mut registry = BlueprintRegistry::default();
        let bp = a_blueprint();
        let first = registry.insert(bp.clone());
        let mut newer = Vec::new();
        for _ in 0..MAX_HANDLES {
            newer.push(registry.insert(bp.clone()));
        }
        assert!(
            registry.get(&first).is_none(),
            "the oldest handle is evicted once the cap is exceeded"
        );
        for handle in &newer {
            assert!(registry.get(handle).is_some(), "every newer handle survives");
        }
    }
}
