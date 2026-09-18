use std::sync::{Arc, Mutex, PoisonError};

pub const KEYRING_SERVICE: &str = "palstudio.nexus";
pub const KEYRING_USER: &str = "api_key";
pub const MAX_KEY_LEN: usize = 512;

#[derive(Clone, PartialEq, Eq)]
pub struct ApiKey(String);

impl ApiKey {
    pub fn parse(raw: &str) -> Option<Self> {
        let trimmed = raw.trim();
        let usable = !trimmed.is_empty()
            && trimmed.len() <= MAX_KEY_LEN
            && trimmed.bytes().all(|byte| byte.is_ascii_graphic());
        usable.then(|| Self(trimmed.to_string()))
    }

    pub fn expose(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Debug for ApiKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("ApiKey(<redacted>)")
    }
}

#[derive(Debug, thiserror::Error)]
pub enum KeyStoreError {
    #[error("the system keyring is unavailable: {0}")]
    Unavailable(String),
}

impl KeyStoreError {
    pub fn code(&self) -> &'static str {
        "keyring_unavailable"
    }
}

pub trait NexusKeyStore: Send + Sync {
    fn get(&self) -> Result<Option<ApiKey>, KeyStoreError>;
    fn set(&self, key: &ApiKey) -> Result<(), KeyStoreError>;
    fn clear(&self) -> Result<(), KeyStoreError>;
}

#[derive(Default)]
pub struct MemoryKeyStore {
    key: Mutex<Option<ApiKey>>,
}

impl MemoryKeyStore {
    pub fn with_key(key: &str) -> Self {
        Self {
            key: Mutex::new(Some(ApiKey::parse(key).expect("a usable test key"))),
        }
    }
}

impl NexusKeyStore for MemoryKeyStore {
    fn get(&self) -> Result<Option<ApiKey>, KeyStoreError> {
        Ok(self
            .key
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone())
    }

    fn set(&self, key: &ApiKey) -> Result<(), KeyStoreError> {
        *self.key.lock().unwrap_or_else(PoisonError::into_inner) = Some(key.clone());
        Ok(())
    }

    fn clear(&self) -> Result<(), KeyStoreError> {
        *self.key.lock().unwrap_or_else(PoisonError::into_inner) = None;
        Ok(())
    }
}

pub struct NoKeyStore;

impl NexusKeyStore for NoKeyStore {
    fn get(&self) -> Result<Option<ApiKey>, KeyStoreError> {
        Ok(None)
    }

    fn set(&self, _key: &ApiKey) -> Result<(), KeyStoreError> {
        Err(KeyStoreError::Unavailable(
            "this build does not store a Nexus Mods key".to_string(),
        ))
    }

    fn clear(&self) -> Result<(), KeyStoreError> {
        Ok(())
    }
}

#[cfg(feature = "desktop")]
pub struct KeyringStore {
    service: String,
    user: String,
}

#[cfg(feature = "desktop")]
impl KeyringStore {
    pub fn palstudio() -> Self {
        Self {
            service: KEYRING_SERVICE.to_string(),
            user: KEYRING_USER.to_string(),
        }
    }

    fn entry(&self) -> Result<keyring::Entry, KeyStoreError> {
        keyring::Entry::new(&self.service, &self.user)
            .map_err(|error| KeyStoreError::Unavailable(error.to_string()))
    }
}

#[cfg(feature = "desktop")]
impl NexusKeyStore for KeyringStore {
    fn get(&self) -> Result<Option<ApiKey>, KeyStoreError> {
        match self.entry()?.get_password() {
            Ok(secret) => Ok(ApiKey::parse(&secret)),
            Err(keyring::Error::NoEntry) => Ok(None),
            Err(error) => Err(KeyStoreError::Unavailable(error.to_string())),
        }
    }

    fn set(&self, key: &ApiKey) -> Result<(), KeyStoreError> {
        self.entry()?
            .set_password(key.expose())
            .map_err(|error| KeyStoreError::Unavailable(error.to_string()))
    }

    fn clear(&self) -> Result<(), KeyStoreError> {
        match self.entry()?.delete_credential() {
            Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
            Err(error) => Err(KeyStoreError::Unavailable(error.to_string())),
        }
    }
}

pub fn system_key_store() -> Arc<dyn NexusKeyStore> {
    #[cfg(feature = "desktop")]
    {
        Arc::new(KeyringStore::palstudio())
    }
    #[cfg(not(feature = "desktop"))]
    {
        Arc::new(NoKeyStore)
    }
}

pub async fn read_key(store: &Arc<dyn NexusKeyStore>) -> Result<Option<ApiKey>, KeyStoreError> {
    let store = Arc::clone(store);
    tokio::task::spawn_blocking(move || store.get())
        .await
        .map_err(|error| KeyStoreError::Unavailable(error.to_string()))?
}

pub async fn write_key(store: &Arc<dyn NexusKeyStore>, key: ApiKey) -> Result<(), KeyStoreError> {
    let store = Arc::clone(store);
    tokio::task::spawn_blocking(move || store.set(&key))
        .await
        .map_err(|error| KeyStoreError::Unavailable(error.to_string()))?
}

pub async fn clear_key(store: &Arc<dyn NexusKeyStore>) -> Result<(), KeyStoreError> {
    let store = Arc::clone(store);
    tokio::task::spawn_blocking(move || store.clear())
        .await
        .map_err(|error| KeyStoreError::Unavailable(error.to_string()))?
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_key_is_trimmed_and_must_be_plain_ascii() {
        assert_eq!(
            ApiKey::parse("  abc123==--  ").unwrap().expose(),
            "abc123==--"
        );
        assert!(ApiKey::parse("").is_none());
        assert!(ApiKey::parse("   ").is_none());
        assert!(ApiKey::parse("has space").is_none());
        assert!(ApiKey::parse("ünicode").is_none());
        assert!(ApiKey::parse(&"k".repeat(MAX_KEY_LEN + 1)).is_none());
        assert!(ApiKey::parse(&"k".repeat(MAX_KEY_LEN)).is_some());
    }

    #[test]
    fn debug_output_never_shows_the_key() {
        let key = ApiKey::parse("super-secret").unwrap();
        assert!(!format!("{key:?}").contains("super-secret"));
        let store = MemoryKeyStore::with_key("super-secret");
        assert!(!format!("{:?}", store.get().unwrap()).contains("super-secret"));
    }

    #[tokio::test]
    async fn the_memory_store_round_trips_through_the_async_helpers() {
        let store: Arc<dyn NexusKeyStore> = Arc::new(MemoryKeyStore::default());
        assert_eq!(read_key(&store).await.unwrap(), None);
        write_key(&store, ApiKey::parse("abc").unwrap())
            .await
            .unwrap();
        assert_eq!(read_key(&store).await.unwrap().unwrap().expose(), "abc");
        clear_key(&store).await.unwrap();
        assert_eq!(read_key(&store).await.unwrap(), None);
        clear_key(&store).await.unwrap();
    }

    #[test]
    fn the_server_build_stores_nothing() {
        let store = NoKeyStore;
        assert_eq!(store.get().unwrap(), None);
        let error = store.set(&ApiKey::parse("abc").unwrap()).unwrap_err();
        assert_eq!(error.code(), "keyring_unavailable");
        store.clear().unwrap();
    }
}
