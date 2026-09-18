//! Nexus Mods: the stored API key, the HTTP client, nxm:// link delivery and
//! the OS link handler.
pub mod api;
pub mod keystore;
pub mod links;
pub mod protocol;

pub(crate) fn now_secs() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|elapsed| elapsed.as_secs())
        .unwrap_or(0)
}
