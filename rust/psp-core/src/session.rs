use crate::error::CoreError;

/// Where the currently loaded save came from.
#[derive(Debug)]
pub enum SaveKind {
    Steam {
        level_path: std::path::PathBuf,
    },
    GamePass {
        container_id: String,
    },
    /// Web zip upload — nothing on disk.
    InMemory,
}

/// A loaded world save. Typed indexes and caches are added by later phase plans.
pub struct SaveSession {
    pub kind: SaveKind,
    pub world_name: String,
    pub level: uesave::Save,
}

/// Per-WS-connection state (spec §3: per-connection sessions fix multi-tab clobbering).
#[derive(Default)]
pub struct Session {
    pub save: Option<SaveSession>,
    /// Transfer-source save (Phase 3).
    pub source: Option<SaveSession>,
}

impl Session {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn save_mut(&mut self) -> Result<&mut SaveSession, CoreError> {
        self.save.as_mut().ok_or(CoreError::SaveNotLoaded)
    }
}

#[cfg(test)]
mod tests {
    use super::Session;
    use crate::error::CoreError;

    #[test]
    fn save_mut_without_loaded_save_is_save_not_loaded() {
        let mut session = Session::new();
        assert!(matches!(session.save_mut(), Err(CoreError::SaveNotLoaded)));
    }
}
