//! Xbox/WinStore (wgs) container format support.
//! Port of palworld_save_pal/utils/gamepass/{container_types,container_utils}.py.

pub mod format;
pub mod store;

#[cfg(any(test, feature = "test-fixtures"))]
pub mod fixture;

/// Player save payloads keyed by kind, decoupled from session types.
#[derive(Debug, Default, Clone)]
pub struct PlayerSavBytes {
    pub sav: Option<Vec<u8>>,
    pub dps: Option<Vec<u8>>,
}
