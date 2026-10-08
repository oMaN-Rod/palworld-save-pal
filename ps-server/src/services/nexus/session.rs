use std::sync::Arc;

use tokio_util::sync::CancellationToken;

use super::api::NexusError;
use super::now_secs;
use super::oauth::{AccessToken, NexusOAuth, OAuthError, Tokens};
use super::token_store::{self, NexusTokenStore, TokenStoreError};

/// How long before its expiry an access token stops being handed out.
const EXPIRY_LEEWAY_SECS: u64 = 60;

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum SessionError {
    #[error(transparent)]
    OAuth(#[from] OAuthError),
    #[error(transparent)]
    Store(#[from] TokenStoreError),
}

impl SessionError {
    pub fn code(&self) -> &'static str {
        match self {
            SessionError::OAuth(error) => error.code(),
            SessionError::Store(error) => error.code(),
        }
    }
}

/// Why a request made on the user's behalf did not go through.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum CallError {
    #[error(transparent)]
    Session(#[from] SessionError),
    #[error(transparent)]
    Nexus(#[from] NexusError),
}

struct Cached {
    token: AccessToken,
    expires_at: u64,
}

/// The signed-in Nexus Mods account: a refresh token in the token store and
/// the access token it last bought, held in memory only.
pub struct NexusSession {
    store: Arc<dyn NexusTokenStore>,
    oauth: Arc<dyn NexusOAuth>,
    access: tokio::sync::Mutex<Option<Cached>>,
}

impl NexusSession {
    pub fn new(store: Arc<dyn NexusTokenStore>, oauth: Arc<dyn NexusOAuth>) -> Self {
        Self {
            store,
            oauth,
            access: tokio::sync::Mutex::new(None),
        }
    }

    /// Runs the browser sign-in. The tokens it returns are not kept until
    /// they are handed to [`adopt`](Self::adopt).
    pub async fn authorize(&self, cancel: &CancellationToken) -> Result<Tokens, SessionError> {
        Ok(self.oauth.sign_in(cancel).await?)
    }

    pub async fn adopt(&self, tokens: Tokens) -> Result<AccessToken, SessionError> {
        let mut access = self.access.lock().await;
        self.keep(&mut access, tokens).await
    }

    /// Makes `call` with the current access token; `None` while signed out.
    /// A token Nexus Mods turns down is renewed and the call made once more,
    /// and a second refusal means the grant itself is gone, so it signs out.
    pub async fn authorized<T, F, Fut>(&self, call: F) -> Result<Option<T>, CallError>
    where
        F: Fn(AccessToken) -> Fut,
        Fut: std::future::Future<Output = Result<T, NexusError>>,
    {
        let Some(token) = self.access_token().await? else {
            return Ok(None);
        };
        match call(token.clone()).await {
            Err(NexusError::Unauthorized) => {}
            outcome => return Ok(Some(outcome?)),
        }
        self.reject_access(&token).await;
        let Some(token) = self.access_token().await? else {
            return Ok(None);
        };
        match call(token.clone()).await {
            Err(NexusError::Unauthorized) => {}
            outcome => return Ok(Some(outcome?)),
        }
        if self.sign_out_of(&token).await.map_err(SessionError::from)? {
            Err(NexusError::Unauthorized.into())
        } else {
            Err(NexusError::Api {
                status: 401,
                message: "the sign-in changed while the request was under way".to_string(),
            }
            .into())
        }
    }

    /// Signs out only while `token` is still the session's own, so a request
    /// that outlived its sign-in cannot erase the one that replaced it.
    async fn sign_out_of(&self, token: &AccessToken) -> Result<bool, TokenStoreError> {
        let mut access = self.access.lock().await;
        if access.as_ref().is_none_or(|cached| cached.token != *token) {
            return Ok(false);
        }
        *access = None;
        token_store::clear_token(&self.store).await?;
        Ok(true)
    }

    /// `None` while signed out. Renews an access token that is about to
    /// expire; a grant Nexus Mods no longer honours signs the user out.
    pub async fn access_token(&self) -> Result<Option<AccessToken>, SessionError> {
        let mut access = self.access.lock().await;
        if let Some(cached) = access.as_ref() {
            if now_secs() + EXPIRY_LEEWAY_SECS < cached.expires_at {
                return Ok(Some(cached.token.clone()));
            }
        }
        let Some(refresh) = token_store::read_token(&self.store).await? else {
            *access = None;
            return Ok(None);
        };
        match self.oauth.refresh(&refresh).await {
            Ok(tokens) => self.keep(&mut access, tokens).await.map(Some),
            Err(OAuthError::Expired) => {
                *access = None;
                token_store::clear_token(&self.store).await?;
                Err(OAuthError::Expired.into())
            }
            Err(error) => Err(error.into()),
        }
    }

    /// Nexus Mods turned `token` down, so the next request renews it instead
    /// of offering it again. A token renewed meanwhile is left alone.
    pub async fn reject_access(&self, token: &AccessToken) {
        let mut access = self.access.lock().await;
        if access.as_ref().is_some_and(|cached| cached.token == *token) {
            *access = None;
        }
    }

    /// The lock is held until the stored grant is gone, so a request made
    /// meanwhile cannot renew it and sign the user back in.
    pub async fn sign_out(&self) -> Result<(), TokenStoreError> {
        let mut access = self.access.lock().await;
        *access = None;
        token_store::clear_token(&self.store).await
    }

    async fn keep(
        &self,
        access: &mut Option<Cached>,
        tokens: Tokens,
    ) -> Result<AccessToken, SessionError> {
        if let Some(refresh) = tokens.refresh {
            token_store::write_token(&self.store, refresh).await?;
        }
        *access = Some(Cached {
            token: tokens.access.clone(),
            expires_at: now_secs() + tokens.expires_in,
        });
        Ok(tokens.access)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::services::nexus::token_store::{MemoryTokenStore, RefreshToken};
    use std::sync::Mutex;

    #[derive(Default)]
    struct ScriptedOAuth {
        refreshes: Mutex<Vec<String>>,
        lifetime: Mutex<u64>,
        fail_with: Mutex<Option<OAuthError>>,
    }

    impl ScriptedOAuth {
        fn tokens(&self, serial: usize) -> Tokens {
            Tokens {
                access: AccessToken::new(format!("access-{serial}")),
                refresh: RefreshToken::parse(&format!("refresh-{serial}")),
                expires_in: *self.lifetime.lock().unwrap(),
            }
        }
    }

    #[async_trait::async_trait]
    impl NexusOAuth for ScriptedOAuth {
        async fn sign_in(&self, _cancel: &CancellationToken) -> Result<Tokens, OAuthError> {
            Ok(self.tokens(0))
        }

        async fn refresh(&self, token: &RefreshToken) -> Result<Tokens, OAuthError> {
            if let Some(error) = self.fail_with.lock().unwrap().clone() {
                return Err(error);
            }
            let mut refreshes = self.refreshes.lock().unwrap();
            refreshes.push(token.expose().to_string());
            Ok(self.tokens(refreshes.len()))
        }
    }

    fn session(
        stored: Option<&str>,
        lifetime: u64,
    ) -> (NexusSession, Arc<MemoryTokenStore>, Arc<ScriptedOAuth>) {
        let store = Arc::new(stored.map(MemoryTokenStore::with_token).unwrap_or_default());
        let oauth = Arc::new(ScriptedOAuth::default());
        *oauth.lifetime.lock().unwrap() = lifetime;
        (
            NexusSession::new(store.clone(), oauth.clone()),
            store,
            oauth,
        )
    }

    fn stored(store: &MemoryTokenStore) -> Option<String> {
        store.get().unwrap().map(|token| token.expose().to_string())
    }

    #[tokio::test]
    async fn signed_out_has_no_token_and_asks_nexus_nothing() {
        let (session, _, oauth) = session(None, 3600);
        assert_eq!(session.access_token().await.unwrap(), None);
        assert!(oauth.refreshes.lock().unwrap().is_empty());
    }

    #[tokio::test]
    async fn signing_in_keeps_the_refresh_token_and_reuses_the_access_token() {
        let (session, store, oauth) = session(None, 3600);
        let tokens = session.authorize(&CancellationToken::new()).await.unwrap();
        assert_eq!(stored(&store), None, "nothing is kept before it is adopted");
        let token = session.adopt(tokens).await.unwrap();
        assert_eq!(token.expose(), "access-0");
        assert_eq!(stored(&store).as_deref(), Some("refresh-0"));

        assert_eq!(session.access_token().await.unwrap(), Some(token));
        assert!(oauth.refreshes.lock().unwrap().is_empty());
    }

    #[tokio::test]
    async fn a_stored_grant_is_renewed_and_its_rotated_refresh_token_kept() {
        let (session, store, oauth) = session(Some("refresh-old"), 3600);
        let token = session.access_token().await.unwrap().unwrap();
        assert_eq!(token.expose(), "access-1");
        assert_eq!(oauth.refreshes.lock().unwrap().as_slice(), ["refresh-old"]);
        assert_eq!(stored(&store).as_deref(), Some("refresh-1"));

        session.access_token().await.unwrap();
        assert_eq!(oauth.refreshes.lock().unwrap().len(), 1);
    }

    #[tokio::test]
    async fn an_expiring_or_rejected_access_token_is_renewed() {
        let (session, _, oauth) = session(Some("refresh-old"), EXPIRY_LEEWAY_SECS);
        session.access_token().await.unwrap();
        session.access_token().await.unwrap();
        assert_eq!(
            oauth.refreshes.lock().unwrap().len(),
            2,
            "inside the leeway"
        );

        *oauth.lifetime.lock().unwrap() = 3600;
        let current = session.access_token().await.unwrap().unwrap();
        session
            .reject_access(&AccessToken::new("some-older-token"))
            .await;
        assert_eq!(session.access_token().await.unwrap().unwrap(), current);
        session.reject_access(&current).await;
        let renewed = session.access_token().await.unwrap().unwrap();
        assert_eq!(renewed.expose(), "access-4");
    }

    #[tokio::test]
    async fn a_revoked_grant_signs_out_but_an_outage_does_not() {
        let (session, store, oauth) = session(Some("refresh-old"), 3600);
        *oauth.fail_with.lock().unwrap() = Some(OAuthError::Network("offline".to_string()));
        let offline = session.access_token().await.unwrap_err();
        assert_eq!(offline.code(), "network");
        assert_eq!(stored(&store).as_deref(), Some("refresh-old"));

        *oauth.fail_with.lock().unwrap() = Some(OAuthError::Expired);
        let expired = session.access_token().await.unwrap_err();
        assert_eq!(expired.code(), "sign_in_expired");
        assert_eq!(stored(&store), None);

        *oauth.fail_with.lock().unwrap() = None;
        assert_eq!(session.access_token().await.unwrap(), None);
    }

    #[tokio::test]
    async fn signing_out_forgets_both_tokens() {
        let (session, store, _) = session(None, 3600);
        let tokens = session.authorize(&CancellationToken::new()).await.unwrap();
        session.adopt(tokens).await.unwrap();
        session.sign_out().await.unwrap();
        assert_eq!(stored(&store), None);
        assert_eq!(session.access_token().await.unwrap(), None);
    }

    /// Accepts only the tokens in `good`, and records every token it is offered.
    fn accepting<'a>(
        good: &'a [&'a str],
        offered: &'a Mutex<Vec<String>>,
    ) -> impl Fn(AccessToken) -> std::future::Ready<Result<&'static str, NexusError>> + 'a {
        move |token| {
            offered.lock().unwrap().push(token.expose().to_string());
            std::future::ready(if good.contains(&token.expose()) {
                Ok("done")
            } else {
                Err(NexusError::Unauthorized)
            })
        }
    }

    #[tokio::test]
    async fn a_refused_access_token_is_renewed_and_the_call_made_once_more() {
        let (session, store, oauth) = session(Some("refresh-old"), 3600);
        let offered = Mutex::new(Vec::new());

        let outcome = session.authorized(accepting(&["access-2"], &offered)).await;

        assert_eq!(outcome, Ok(Some("done")));
        assert_eq!(offered.lock().unwrap().as_slice(), ["access-1", "access-2"]);
        assert_eq!(oauth.refreshes.lock().unwrap().len(), 2);
        assert_eq!(stored(&store).as_deref(), Some("refresh-2"));
    }

    #[tokio::test]
    async fn a_second_refusal_signs_out_and_other_failures_keep_the_grant() {
        let (session, store, _) = session(Some("refresh-old"), 3600);
        let offered = Mutex::new(Vec::new());
        let outage = session
            .authorized(|_| std::future::ready(Err::<(), _>(NexusError::Network("x".to_string()))))
            .await;
        assert_eq!(
            outage,
            Err(CallError::Nexus(NexusError::Network("x".to_string())))
        );
        assert_eq!(stored(&store).as_deref(), Some("refresh-1"));

        let refused = session.authorized(accepting(&[], &offered)).await;
        assert_eq!(refused, Err(CallError::Nexus(NexusError::Unauthorized)));
        assert_eq!(offered.lock().unwrap().len(), 2, "one retry, no more");
        assert_eq!(stored(&store), None);

        let signed_out = session.authorized(accepting(&[], &offered)).await;
        assert_eq!(signed_out, Ok(None));
    }

    #[tokio::test]
    async fn a_request_that_outlived_its_sign_in_leaves_the_new_one_alone() {
        let (session, store, _) = session(Some("refresh-old"), 3600);
        let calls = std::sync::atomic::AtomicUsize::new(0);
        let fresh = || Tokens {
            access: AccessToken::new("access-fresh"),
            refresh: RefreshToken::parse("refresh-fresh"),
            expires_in: 3600,
        };

        let outcome = session
            .authorized(|_| {
                let (session, calls) = (&session, &calls);
                async move {
                    if calls.fetch_add(1, std::sync::atomic::Ordering::SeqCst) == 1 {
                        session.adopt(fresh()).await.unwrap();
                    }
                    Err::<(), _>(NexusError::Unauthorized)
                }
            })
            .await;

        let error = outcome.unwrap_err();
        assert_ne!(error, CallError::Nexus(NexusError::Unauthorized));
        assert_eq!(stored(&store).as_deref(), Some("refresh-fresh"));
        let current = session.access_token().await.unwrap().unwrap();
        assert_eq!(current.expose(), "access-fresh");
    }

    /// A store whose `clear` reports that it was entered, then waits to be let go.
    struct SlowClearStore {
        inner: MemoryTokenStore,
        entered: Mutex<std::sync::mpsc::Sender<()>>,
        release: Mutex<std::sync::mpsc::Receiver<()>>,
    }

    impl NexusTokenStore for SlowClearStore {
        fn get(&self) -> Result<Option<RefreshToken>, TokenStoreError> {
            self.inner.get()
        }

        fn set(&self, token: &RefreshToken) -> Result<(), TokenStoreError> {
            self.inner.set(token)
        }

        fn clear(&self) -> Result<(), TokenStoreError> {
            self.entered.lock().unwrap().send(()).unwrap();
            self.release.lock().unwrap().recv().unwrap();
            self.inner.clear()
        }
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn a_request_made_during_sign_out_cannot_sign_back_in() {
        let (entered_tx, entered_rx) = std::sync::mpsc::channel();
        let (release_tx, release_rx) = std::sync::mpsc::channel();
        let store = Arc::new(SlowClearStore {
            inner: MemoryTokenStore::with_token("refresh-old"),
            entered: Mutex::new(entered_tx),
            release: Mutex::new(release_rx),
        });
        let oauth = Arc::new(ScriptedOAuth::default());
        *oauth.lifetime.lock().unwrap() = 3600;
        let session = Arc::new(NexusSession::new(store.clone(), oauth.clone()));
        session.access_token().await.unwrap().unwrap();

        let signing_out = {
            let session = session.clone();
            tokio::spawn(async move { session.sign_out().await })
        };
        tokio::task::spawn_blocking(move || entered_rx.recv().unwrap())
            .await
            .unwrap();
        let overlapping = {
            let session = session.clone();
            tokio::spawn(async move { session.access_token().await })
        };
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
        release_tx.send(()).unwrap();

        signing_out.await.unwrap().unwrap();
        assert_eq!(overlapping.await.unwrap(), Ok(None));
        assert_eq!(session.access_token().await, Ok(None));
        assert_eq!(store.inner.get().unwrap(), None);
        assert_eq!(
            oauth.refreshes.lock().unwrap().len(),
            1,
            "no renewal slipped in"
        );
    }
}
