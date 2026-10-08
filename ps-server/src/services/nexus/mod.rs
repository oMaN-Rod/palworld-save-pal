//! Nexus Mods: OAuth sign-in and the session it yields, the HTTP client,
//! nxm:// link delivery and the OS link handler.
pub mod api;
pub mod links;
pub mod oauth;
pub mod protocol;
pub mod session;
pub mod token_store;

pub(crate) fn now_secs() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|elapsed| elapsed.as_secs())
        .unwrap_or(0)
}
