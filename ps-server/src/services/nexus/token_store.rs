use std::sync::{Arc, Mutex, PoisonError};

pub const KEYRING_SERVICE: &str = "palstudio.nexus";
pub const KEYRING_USER: &str = "refresh_token";
pub const MAX_TOKEN_LEN: usize = 1024;

/// The long-lived half of a Nexus Mods OAuth grant, and the only credential
/// that is ever written to disk.
#[derive(Clone, PartialEq, Eq)]
pub struct RefreshToken(String);

impl RefreshToken {
    pub fn parse(raw: &str) -> Option<Self> {
        let trimmed = raw.trim();
        let usable = !trimmed.is_empty()
            && trimmed.len() <= MAX_TOKEN_LEN
            && trimmed.bytes().all(|byte| byte.is_ascii_graphic());
        usable.then(|| Self(trimmed.to_string()))
    }

    pub fn expose(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Debug for RefreshToken {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("RefreshToken(<redacted>)")
    }
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum TokenStoreError {
    #[error("the system keyring is unavailable: {0}")]
    Unavailable(String),
}

impl TokenStoreError {
    pub fn code(&self) -> &'static str {
        "keyring_unavailable"
    }
}

pub trait NexusTokenStore: Send + Sync {
    fn get(&self) -> Result<Option<RefreshToken>, TokenStoreError>;
    fn set(&self, token: &RefreshToken) -> Result<(), TokenStoreError>;
    fn clear(&self) -> Result<(), TokenStoreError>;
}

#[derive(Default)]
pub struct MemoryTokenStore {
    token: Mutex<Option<RefreshToken>>,
}

impl MemoryTokenStore {
    pub fn with_token(token: &str) -> Self {
        Self {
            token: Mutex::new(Some(
                RefreshToken::parse(token).expect("a usable test token"),
            )),
        }
    }
}

impl NexusTokenStore for MemoryTokenStore {
    fn get(&self) -> Result<Option<RefreshToken>, TokenStoreError> {
        Ok(self
            .token
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone())
    }

    fn set(&self, token: &RefreshToken) -> Result<(), TokenStoreError> {
        *self.token.lock().unwrap_or_else(PoisonError::into_inner) = Some(token.clone());
        Ok(())
    }

    fn clear(&self) -> Result<(), TokenStoreError> {
        *self.token.lock().unwrap_or_else(PoisonError::into_inner) = None;
        Ok(())
    }
}

pub struct NoTokenStore;

impl NexusTokenStore for NoTokenStore {
    fn get(&self) -> Result<Option<RefreshToken>, TokenStoreError> {
        Ok(None)
    }

    fn set(&self, _token: &RefreshToken) -> Result<(), TokenStoreError> {
        Err(TokenStoreError::Unavailable(
            "this build does not keep a Nexus Mods sign-in".to_string(),
        ))
    }

    fn clear(&self) -> Result<(), TokenStoreError> {
        Ok(())
    }
}

#[cfg(feature = "desktop")]
pub struct KeyringStore {
    service: String,
    user: String,
    legacy_removed: std::sync::Once,
}

#[cfg(feature = "desktop")]
impl KeyringStore {
    /// Entries earlier builds wrote a pasted personal API key under.
    const LEGACY_USERS: [&'static str; 2] = ["api_key", "sso_key"];

    pub fn palstudio() -> Self {
        Self {
            service: KEYRING_SERVICE.to_string(),
            user: KEYRING_USER.to_string(),
            legacy_removed: std::sync::Once::new(),
        }
    }

    fn entry(&self) -> Result<keyring::Entry, TokenStoreError> {
        keyring::Entry::new(&self.service, &self.user)
            .map_err(|error| TokenStoreError::Unavailable(error.to_string()))
    }

    fn remove_legacy(&self) {
        self.legacy_removed.call_once(|| {
            for user in Self::LEGACY_USERS {
                if let Ok(entry) = keyring::Entry::new(&self.service, user) {
                    let _ = entry.delete_credential();
                }
            }
        });
    }
}

#[cfg(feature = "desktop")]
impl NexusTokenStore for KeyringStore {
    fn get(&self) -> Result<Option<RefreshToken>, TokenStoreError> {
        self.remove_legacy();
        match self.entry()?.get_password() {
            Ok(secret) => Ok(RefreshToken::parse(&secret)),
            Err(keyring::Error::NoEntry) => Ok(None),
            Err(error) => Err(TokenStoreError::Unavailable(error.to_string())),
        }
    }

    fn set(&self, token: &RefreshToken) -> Result<(), TokenStoreError> {
        self.entry()?
            .set_password(token.expose())
            .map_err(|error| TokenStoreError::Unavailable(error.to_string()))
    }

    fn clear(&self) -> Result<(), TokenStoreError> {
        match self.entry()?.delete_credential() {
            Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
            Err(error) => Err(TokenStoreError::Unavailable(error.to_string())),
        }
    }
}

pub fn system_token_store() -> Arc<dyn NexusTokenStore> {
    #[cfg(feature = "desktop")]
    {
        Arc::new(KeyringStore::palstudio())
    }
    #[cfg(not(feature = "desktop"))]
    {
        Arc::new(NoTokenStore)
    }
}

pub async fn read_token(
    store: &Arc<dyn NexusTokenStore>,
) -> Result<Option<RefreshToken>, TokenStoreError> {
    let store = Arc::clone(store);
    tokio::task::spawn_blocking(move || store.get())
        .await
        .map_err(|error| TokenStoreError::Unavailable(error.to_string()))?
}

pub async fn write_token(
    store: &Arc<dyn NexusTokenStore>,
    token: RefreshToken,
) -> Result<(), TokenStoreError> {
    let store = Arc::clone(store);
    tokio::task::spawn_blocking(move || store.set(&token))
        .await
        .map_err(|error| TokenStoreError::Unavailable(error.to_string()))?
}

pub async fn clear_token(store: &Arc<dyn NexusTokenStore>) -> Result<(), TokenStoreError> {
    let store = Arc::clone(store);
    tokio::task::spawn_blocking(move || store.clear())
        .await
        .map_err(|error| TokenStoreError::Unavailable(error.to_string()))?
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_token_is_trimmed_and_must_be_plain_ascii() {
        assert_eq!(
            RefreshToken::parse("  abc123==--  ").unwrap().expose(),
            "abc123==--"
        );
        assert!(RefreshToken::parse("").is_none());
        assert!(RefreshToken::parse("   ").is_none());
        assert!(RefreshToken::parse("has space").is_none());
        assert!(RefreshToken::parse("ünicode").is_none());
        assert!(RefreshToken::parse(&"k".repeat(MAX_TOKEN_LEN + 1)).is_none());
        assert!(RefreshToken::parse(&"k".repeat(MAX_TOKEN_LEN)).is_some());
    }

    #[test]
    fn debug_output_never_shows_the_token() {
        let token = RefreshToken::parse("super-secret").unwrap();
        assert!(!format!("{token:?}").contains("super-secret"));
        let store = MemoryTokenStore::with_token("super-secret");
        assert!(!format!("{:?}", store.get().unwrap()).contains("super-secret"));
    }

    #[tokio::test]
    async fn the_memory_store_round_trips_through_the_async_helpers() {
        let store: Arc<dyn NexusTokenStore> = Arc::new(MemoryTokenStore::default());
        assert_eq!(read_token(&store).await.unwrap(), None);
        write_token(&store, RefreshToken::parse("abc").unwrap())
            .await
            .unwrap();
        assert_eq!(read_token(&store).await.unwrap().unwrap().expose(), "abc");
        clear_token(&store).await.unwrap();
        assert_eq!(read_token(&store).await.unwrap(), None);
        clear_token(&store).await.unwrap();
    }

    #[test]
    fn the_server_build_stores_nothing() {
        let store = NoTokenStore;
        assert_eq!(store.get().unwrap(), None);
        let error = store.set(&RefreshToken::parse("abc").unwrap()).unwrap_err();
        assert_eq!(error.code(), "keyring_unavailable");
        store.clear().unwrap();
    }
}
