//! Xbox/WinStore (wgs) container format support.

pub mod convert;
pub mod format;
pub mod scan;
pub mod store;

#[cfg(any(test, feature = "test-fixtures"))]
pub mod fixture;

#[derive(Debug, Default, Clone)]
pub struct PlayerSavBytes {
    pub sav: Option<Vec<u8>>,
    pub dps: Option<Vec<u8>>,
}
