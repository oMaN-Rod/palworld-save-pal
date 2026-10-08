use std::collections::HashMap;
use std::sync::{Arc, Mutex, PoisonError};
use std::time::Duration;

use async_trait::async_trait;
use axum::extract::{Query, State};
use axum::response::Html;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use base64::Engine;
use sha2::{Digest, Sha256};
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;

use super::token_store::RefreshToken;

pub const AUTHORIZE_URL: &str = "https://users.nexusmods.com/oauth/authorize";
pub const TOKEN_URL: &str = "https://users.nexusmods.com/oauth/token";
pub const CLIENT_ID: &str = "palstudio";
pub const CLIENT_ID_ENV: &str = "PS_NEXUS_CLIENT_ID";
/// Part of the callback URI registered with Nexus Mods, so it cannot change
/// without registering the new one.
pub const CALLBACK_PORT: u16 = 47310;
pub const CALLBACK_PATH: &str = "/callback";
pub const SIGN_IN_TIMEOUT: Duration = Duration::from_secs(300);
const TOKEN_TIMEOUT: Duration = Duration::from_secs(30);
const DEFAULT_LIFETIME_SECS: u64 = 3600;

const DONE_PAGE: &str = "<!doctype html><meta charset=\"utf-8\"><title>PalStudio</title>\
<body style=\"font-family:sans-serif;text-align:center;padding-top:4rem\">\
<h1>PalStudio</h1><p>You can close this tab and return to PalStudio.</p></body>";

#[derive(Clone, PartialEq, Eq)]
pub struct AccessToken(String);

impl AccessToken {
    pub fn new(token: impl Into<String>) -> Self {
        Self(token.into())
    }

    pub fn expose(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Debug for AccessToken {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("AccessToken(<redacted>)")
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Tokens {
    pub access: AccessToken,
    pub refresh: Option<RefreshToken>,
    pub expires_in: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum OAuthError {
    #[error("signing in to Nexus Mods was cancelled")]
    Cancelled,
    #[error("signing in to Nexus Mods took too long")]
    TimedOut,
    #[error("could not reach Nexus Mods: {0}")]
    Network(String),
    #[error("Nexus Mods refused the sign-in: {0}")]
    Refused(String),
    #[error("the Nexus Mods sign-in has expired")]
    Expired,
    #[error("could not open the browser: {0}")]
    Browser(String),
    #[error("could not listen for the sign-in to come back: {0}")]
    Callback(String),
}

impl OAuthError {
    pub fn code(&self) -> &'static str {
        match self {
            OAuthError::Cancelled => "sign_in_cancelled",
            OAuthError::TimedOut => "sign_in_timed_out",
            OAuthError::Network(_) => "network",
            OAuthError::Refused(_) => "sign_in_refused",
            OAuthError::Expired => "sign_in_expired",
            OAuthError::Browser(_) => "browser_failed",
            OAuthError::Callback(_) => "callback_unavailable",
        }
    }
}

#[async_trait]
pub trait NexusOAuth: Send + Sync {
    async fn sign_in(&self, cancel: &CancellationToken) -> Result<Tokens, OAuthError>;
    async fn refresh(&self, token: &RefreshToken) -> Result<Tokens, OAuthError>;
}

pub type BrowserOpener = Arc<dyn Fn(&str) -> Result<(), String> + Send + Sync>;

pub fn client_id() -> String {
    std::env::var(CLIENT_ID_ENV)
        .ok()
        .map(|id| id.trim().to_string())
        .filter(|id| !id.is_empty())
        .unwrap_or_else(|| CLIENT_ID.to_string())
}

/// Authorization code flow with PKCE for a public client: no client secret,
/// and the code comes back to a listener on this machine.
pub struct PkceOAuth {
    http: reqwest::Client,
    authorize_url: String,
    token_url: String,
    client_id: String,
    callback_port: u16,
    timeout: Duration,
    open: BrowserOpener,
}

impl PkceOAuth {
    pub fn production() -> Self {
        Self::new(
            AUTHORIZE_URL,
            TOKEN_URL,
            &client_id(),
            CALLBACK_PORT,
            Arc::new(|url| opener::open(url).map_err(|error| error.to_string())),
        )
    }

    pub fn new(
        authorize_url: &str,
        token_url: &str,
        client_id: &str,
        callback_port: u16,
        open: BrowserOpener,
    ) -> Self {
        let http = reqwest::Client::builder()
            .timeout(TOKEN_TIMEOUT)
            .redirect(reqwest::redirect::Policy::none())
            .build()
            .expect("nexus oauth client");
        Self {
            http,
            authorize_url: authorize_url.to_string(),
            token_url: token_url.to_string(),
            client_id: client_id.to_string(),
            callback_port,
            timeout: SIGN_IN_TIMEOUT,
            open,
        }
    }

    pub fn with_timeout(mut self, timeout: Duration) -> Self {
        self.timeout = timeout;
        self
    }

    async fn authorize(&self) -> Result<Tokens, OAuthError> {
        let listener = tokio::net::TcpListener::bind(("127.0.0.1", self.callback_port))
            .await
            .map_err(|error| OAuthError::Callback(error.to_string()))?;
        let port = listener
            .local_addr()
            .map_err(|error| OAuthError::Callback(error.to_string()))?
            .port();
        let redirect_uri = format!("http://127.0.0.1:{port}{CALLBACK_PATH}");
        let verifier = URL_SAFE_NO_PAD.encode(rand::random::<[u8; 32]>());
        let challenge = URL_SAFE_NO_PAD.encode(Sha256::digest(verifier.as_bytes()));
        let state = uuid::Uuid::new_v4().simple().to_string();

        let (sender, mut receiver) = mpsc::channel::<Callback>(1);
        let router = axum::Router::new()
            .route(CALLBACK_PATH, axum::routing::get(callback))
            .with_state(Arc::new(CallbackState {
                expected: state.clone(),
                sender,
            }));
        let server = tokio::spawn(async move {
            let _ = axum::serve(listener, router).await;
        });
        let _stop = AbortOnDrop(server);

        let link = reqwest::Url::parse_with_params(
            &self.authorize_url,
            [
                ("client_id", self.client_id.as_str()),
                ("response_type", "code"),
                ("scope", ""),
                ("redirect_uri", redirect_uri.as_str()),
                ("state", state.as_str()),
                ("code_challenge_method", "S256"),
                ("code_challenge", challenge.as_str()),
            ],
        )
        .map_err(|error| OAuthError::Browser(error.to_string()))?;
        (self.open)(link.as_str()).map_err(OAuthError::Browser)?;

        let code = match receiver.recv().await {
            Some(Callback::Code(code)) => code,
            Some(Callback::Denied(reason)) => return Err(OAuthError::Refused(reason)),
            None => return Err(OAuthError::Callback("the listener stopped".to_string())),
        };
        self.token_request(
            &[
                ("grant_type", "authorization_code"),
                ("redirect_uri", redirect_uri.as_str()),
                ("client_id", self.client_id.as_str()),
                ("code", code.as_str()),
                ("code_verifier", verifier.as_str()),
            ],
            OAuthError::Refused,
        )
        .await
    }

    async fn token_request(
        &self,
        form: &[(&str, &str)],
        rejected: impl FnOnce(String) -> OAuthError,
    ) -> Result<Tokens, OAuthError> {
        let response = self
            .http
            .post(&self.token_url)
            .form(form)
            .send()
            .await
            .map_err(network)?;
        let status = response.status();
        let body = response.bytes().await.map_err(network)?;
        // Throttling and timeouts are 4xx too, but say nothing about the grant.
        let grant_rejected = matches!(
            status,
            reqwest::StatusCode::BAD_REQUEST
                | reqwest::StatusCode::UNAUTHORIZED
                | reqwest::StatusCode::FORBIDDEN
        );
        if grant_rejected {
            let reason = serde_json::from_slice::<TokenError>(&body)
                .ok()
                .and_then(|error| error.error_description.or(error.error))
                .unwrap_or_else(|| status.to_string());
            return Err(rejected(reason));
        }
        if !status.is_success() {
            return Err(OAuthError::Network(format!(
                "the token endpoint answered {status}"
            )));
        }
        let reply: TokenReply = serde_json::from_slice(&body)
            .map_err(|error| OAuthError::Network(format!("unexpected token reply: {error}")))?;
        Ok(Tokens {
            access: AccessToken::new(reply.access_token),
            refresh: reply.refresh_token.as_deref().and_then(RefreshToken::parse),
            expires_in: reply.expires_in.unwrap_or(DEFAULT_LIFETIME_SECS),
        })
    }
}

#[async_trait]
impl NexusOAuth for PkceOAuth {
    async fn sign_in(&self, cancel: &CancellationToken) -> Result<Tokens, OAuthError> {
        tokio::select! {
            _ = cancel.cancelled() => Err(OAuthError::Cancelled),
            outcome = tokio::time::timeout(self.timeout, self.authorize()) => {
                outcome.unwrap_or(Err(OAuthError::TimedOut))
            }
        }
    }

    async fn refresh(&self, token: &RefreshToken) -> Result<Tokens, OAuthError> {
        self.token_request(
            &[
                ("grant_type", "refresh_token"),
                ("client_id", self.client_id.as_str()),
                ("refresh_token", token.expose()),
            ],
            |_| OAuthError::Expired,
        )
        .await
    }
}

fn network(error: reqwest::Error) -> OAuthError {
    OAuthError::Network(error.without_url().to_string())
}

struct AbortOnDrop(tokio::task::JoinHandle<()>);

impl Drop for AbortOnDrop {
    fn drop(&mut self) {
        self.0.abort();
    }
}

enum Callback {
    Code(String),
    Denied(String),
}

struct CallbackState {
    expected: String,
    sender: mpsc::Sender<Callback>,
}

/// A request without this sign-in's `state` did not come from the page this
/// app opened, so it is turned away and the wait goes on.
async fn callback(
    State(state): State<Arc<CallbackState>>,
    Query(query): Query<HashMap<String, String>>,
) -> Result<Html<&'static str>, axum::http::StatusCode> {
    if query.get("state") != Some(&state.expected) {
        return Err(axum::http::StatusCode::BAD_REQUEST);
    }
    let outcome = match (query.get("code"), query.get("error")) {
        (Some(code), _) if !code.is_empty() => Callback::Code(code.clone()),
        (_, Some(error)) => {
            Callback::Denied(query.get("error_description").unwrap_or(error).to_string())
        }
        _ => return Err(axum::http::StatusCode::BAD_REQUEST),
    };
    let _ = state.sender.try_send(outcome);
    Ok(Html(DONE_PAGE))
}

#[derive(serde::Deserialize)]
struct TokenReply {
    access_token: String,
    #[serde(default)]
    refresh_token: Option<String>,
    #[serde(default)]
    expires_in: Option<u64>,
}

#[derive(serde::Deserialize)]
struct TokenError {
    #[serde(default)]
    error: Option<String>,
    #[serde(default)]
    error_description: Option<String>,
}

/// The one sign-in that may be waiting on the browser at a time.
#[derive(Default)]
pub struct SignInSlot {
    current: Mutex<Option<CancellationToken>>,
}

impl SignInSlot {
    /// Cancels whichever sign-in was still waiting.
    pub fn begin(&self) -> CancellationToken {
        let token = CancellationToken::new();
        if let Some(previous) = self.lock().replace(token.clone()) {
            previous.cancel();
        }
        token
    }

    pub fn cancel(&self) -> bool {
        match self.lock().take() {
            Some(token) => {
                token.cancel();
                true
            }
            None => false,
        }
    }

    /// Closes the window for cancelling `token`'s sign-in and says whether it
    /// may go ahead: false once it was cancelled or replaced, and then its
    /// successor's slot is left alone. `cancel` cancels under the same lock,
    /// so a cancel is either seen here or finds nothing left to cancel.
    pub fn finish(&self, token: &CancellationToken) -> bool {
        let mut current = self.lock();
        if token.is_cancelled() {
            return false;
        }
        current.take();
        true
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, Option<CancellationToken>> {
        self.current.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::http::StatusCode;
    use axum::Form;
    use axum::Json;
    use serde_json::json;

    type Forms = Arc<Mutex<Vec<HashMap<String, String>>>>;

    async fn spawn_token_endpoint(forms: Forms) -> String {
        let router = axum::Router::new()
            .route(
                "/oauth/token",
                axum::routing::post(
                    |State(forms): State<Forms>, Form(form): Form<HashMap<String, String>>| async move {
                        forms.lock().unwrap().push(form.clone());
                        let rejected = form.get("code").map(String::as_str) == Some("bad-code")
                            || form.get("refresh_token").map(String::as_str) == Some("revoked");
                        if form.get("refresh_token").map(String::as_str) == Some("throttled") {
                            return (
                                StatusCode::TOO_MANY_REQUESTS,
                                Json(json!({ "error": "slow_down" })),
                            );
                        }
                        if rejected {
                            return (
                                StatusCode::BAD_REQUEST,
                                Json(json!({ "error": "invalid_grant",
                                             "error_description": "the grant is invalid" })),
                            );
                        }
                        (
                            StatusCode::OK,
                            Json(json!({ "access_token": "access-1", "refresh_token": "refresh-1",
                                         "expires_in": 21600, "token_type": "Bearer" })),
                        )
                    },
                ),
            )
            .with_state(forms);
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        tokio::spawn(async move { axum::serve(listener, router).await.unwrap() });
        format!("http://{addr}/oauth/token")
    }

    /// Stands in for the browser: follows each opened link's redirect URI with
    /// whatever `answer` makes of the authorize parameters.
    fn browser(
        answer: impl Fn(&HashMap<String, String>) -> Vec<(String, String)> + Send + Sync + 'static,
    ) -> (BrowserOpener, Arc<Mutex<Vec<HashMap<String, String>>>>) {
        let opened: Arc<Mutex<Vec<HashMap<String, String>>>> = Default::default();
        let seen = opened.clone();
        let open: BrowserOpener = Arc::new(move |link| {
            let url = reqwest::Url::parse(link).map_err(|error| error.to_string())?;
            let params: HashMap<String, String> = url.query_pairs().into_owned().collect();
            seen.lock().unwrap().push(params.clone());
            let back = reqwest::Url::parse_with_params(&params["redirect_uri"], answer(&params))
                .map_err(|error| error.to_string())?;
            tokio::spawn(async move {
                let _ = reqwest::get(back).await;
            });
            Ok(())
        });
        (open, opened)
    }

    fn approve(code: &'static str) -> impl Fn(&HashMap<String, String>) -> Vec<(String, String)> {
        move |params| {
            vec![
                ("code".to_string(), code.to_string()),
                ("state".to_string(), params["state"].clone()),
            ]
        }
    }

    fn oauth(token_url: &str, open: BrowserOpener) -> PkceOAuth {
        PkceOAuth::new(
            "https://users.example.invalid/oauth/authorize",
            token_url,
            "test-client",
            0,
            open,
        )
        .with_timeout(Duration::from_secs(5))
    }

    #[tokio::test]
    async fn the_code_that_comes_back_is_exchanged_with_its_verifier() {
        let forms: Forms = Default::default();
        let token_url = spawn_token_endpoint(forms.clone()).await;
        let (open, opened) = browser(approve("code-1"));

        let tokens = oauth(&token_url, open)
            .sign_in(&CancellationToken::new())
            .await
            .unwrap();

        assert_eq!(tokens.access.expose(), "access-1");
        assert_eq!(tokens.refresh.unwrap().expose(), "refresh-1");
        assert_eq!(tokens.expires_in, 21600);

        let authorize = opened.lock().unwrap()[0].clone();
        assert_eq!(authorize["client_id"], "test-client");
        assert_eq!(authorize["response_type"], "code");
        assert_eq!(authorize["code_challenge_method"], "S256");
        assert!(authorize["redirect_uri"].starts_with("http://127.0.0.1:"));
        assert!(authorize["redirect_uri"].ends_with("/callback"));

        let form = forms.lock().unwrap()[0].clone();
        assert_eq!(form["grant_type"], "authorization_code");
        assert_eq!(form["code"], "code-1");
        assert_eq!(form["client_id"], "test-client");
        assert_eq!(form["redirect_uri"], authorize["redirect_uri"]);
        assert!(!form.contains_key("client_secret"));
        assert!(form["code_verifier"].len() >= 43);
        assert_eq!(
            URL_SAFE_NO_PAD.encode(Sha256::digest(form["code_verifier"].as_bytes())),
            authorize["code_challenge"]
        );
    }

    #[tokio::test]
    async fn a_callback_with_the_wrong_state_is_ignored() {
        let forms: Forms = Default::default();
        let token_url = spawn_token_endpoint(forms.clone()).await;
        let (open, _) = browser(|_| {
            vec![
                ("code".to_string(), "stolen".to_string()),
                ("state".to_string(), "not-ours".to_string()),
            ]
        });

        let error = oauth(&token_url, open)
            .with_timeout(Duration::from_millis(300))
            .sign_in(&CancellationToken::new())
            .await
            .unwrap_err();

        assert_eq!(error, OAuthError::TimedOut);
        assert!(forms.lock().unwrap().is_empty(), "nothing was exchanged");
    }

    #[tokio::test]
    async fn a_denied_or_rejected_sign_in_is_refused() {
        let forms: Forms = Default::default();
        let token_url = spawn_token_endpoint(forms.clone()).await;

        let (open, _) = browser(|params| {
            vec![
                ("error".to_string(), "access_denied".to_string()),
                ("state".to_string(), params["state"].clone()),
            ]
        });
        let denied = oauth(&token_url, open)
            .sign_in(&CancellationToken::new())
            .await
            .unwrap_err();
        assert_eq!(denied, OAuthError::Refused("access_denied".to_string()));

        let (open, _) = browser(approve("bad-code"));
        let rejected = oauth(&token_url, open)
            .sign_in(&CancellationToken::new())
            .await
            .unwrap_err();
        assert_eq!(
            rejected,
            OAuthError::Refused("the grant is invalid".to_string())
        );
        assert_eq!(rejected.code(), "sign_in_refused");

        let no_browser: BrowserOpener = Arc::new(|_| Err("no browser".to_string()));
        let failed = oauth(&token_url, no_browser)
            .sign_in(&CancellationToken::new())
            .await
            .unwrap_err();
        assert_eq!(failed.code(), "browser_failed");
    }

    #[tokio::test]
    async fn a_waiting_sign_in_can_be_cancelled_and_frees_its_port() {
        let (open, opened) = browser(|_| Vec::new());
        let waiting = Arc::new(oauth("http://127.0.0.1:9/oauth/token", open));
        let cancel = CancellationToken::new();
        let task = {
            let waiting = waiting.clone();
            let cancel = cancel.clone();
            tokio::spawn(async move { waiting.sign_in(&cancel).await })
        };
        while opened.lock().unwrap().is_empty() {
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
        let redirect = opened.lock().unwrap()[0]["redirect_uri"].clone();
        cancel.cancel();
        assert_eq!(task.await.unwrap().unwrap_err(), OAuthError::Cancelled);

        let port = reqwest::Url::parse(&redirect).unwrap().port().unwrap();
        let mut freed = false;
        for _ in 0..50 {
            if tokio::net::TcpListener::bind(("127.0.0.1", port))
                .await
                .is_ok()
            {
                freed = true;
                break;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
        assert!(freed, "the callback listener is gone");
    }

    #[tokio::test]
    async fn a_refresh_returns_new_tokens_or_reports_the_grant_gone() {
        let forms: Forms = Default::default();
        let token_url = spawn_token_endpoint(forms.clone()).await;
        let (open, _) = browser(|_| Vec::new());
        let oauth = oauth(&token_url, open);

        let tokens = oauth
            .refresh(&RefreshToken::parse("refresh-0").unwrap())
            .await
            .unwrap();
        assert_eq!(tokens.access.expose(), "access-1");
        let form = forms.lock().unwrap()[0].clone();
        assert_eq!(form["grant_type"], "refresh_token");
        assert_eq!(form["refresh_token"], "refresh-0");
        assert_eq!(form["client_id"], "test-client");

        let expired = oauth
            .refresh(&RefreshToken::parse("revoked").unwrap())
            .await
            .unwrap_err();
        assert_eq!(expired, OAuthError::Expired);
        assert_eq!(expired.code(), "sign_in_expired");

        let throttled = oauth
            .refresh(&RefreshToken::parse("throttled").unwrap())
            .await
            .unwrap_err();
        assert_eq!(throttled.code(), "network", "{throttled}");

        let (open, _) = browser(|_| Vec::new());
        let unreachable =
            PkceOAuth::new("https://x.invalid/a", "http://127.0.0.1:9/t", "c", 0, open)
                .refresh(&RefreshToken::parse("refresh-0").unwrap())
                .await
                .unwrap_err();
        assert_eq!(unreachable.code(), "network");
    }

    #[test]
    fn debug_output_never_shows_a_token() {
        let tokens = Tokens {
            access: AccessToken::new("access-secret"),
            refresh: RefreshToken::parse("refresh-secret"),
            expires_in: 60,
        };
        let shown = format!("{tokens:?}");
        assert!(!shown.contains("access-secret") && !shown.contains("refresh-secret"));
    }

    #[test]
    fn a_newer_sign_in_cancels_the_one_before_it() {
        let slot = SignInSlot::default();
        assert!(!slot.cancel());
        let first = slot.begin();
        let second = slot.begin();
        assert!(first.is_cancelled());
        assert!(!second.is_cancelled());

        assert!(!slot.finish(&first), "a replaced sign-in may not go ahead");
        assert!(
            slot.cancel(),
            "the replaced sign-in left its successor alone"
        );
        assert!(second.is_cancelled());
        assert!(!slot.finish(&second), "nor may a cancelled one");

        let third = slot.begin();
        assert!(slot.finish(&third));
        assert!(
            !slot.cancel(),
            "nothing is left to cancel once it went ahead"
        );
        assert!(!third.is_cancelled());
    }
}
