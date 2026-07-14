//! Palworld dedicated-server REST API client. The server exposes plain HTTP at
//! `http://{host}:{port}/v1/api/{endpoint}` and authenticates with HTTP basic
//! auth whose user is always the literal "admin" and whose password is the
//! server's AdminPassword.
use serde_json::Value;

use super::ServiceError;

#[derive(Clone)]
pub struct PalworldApiClient {
    http: reqwest::Client,
}

impl Default for PalworldApiClient {
    fn default() -> Self {
        Self::new()
    }
}

impl PalworldApiClient {
    pub fn new() -> Self {
        let http = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(10))
            .build()
            .expect("building reqwest client cannot fail with static config");
        Self { http }
    }

    /// Some endpoints answer with bare text ("OK") rather than JSON, so the body
    /// is only parsed as JSON when the content type says so. A non-2xx response is
    /// still `Ok`, carrying its `status_code` for the caller to inspect; `Err` is
    /// reserved for transport failures.
    pub async fn rest_api_call(
        &self,
        host: &str,
        port: u16,
        admin_password: &str,
        endpoint: &str,
        method: &str,
        payload: Option<&Value>,
    ) -> Result<Value, ServiceError> {
        let url = format!("http://{host}:{port}/v1/api/{endpoint}");
        let empty_object = Value::Object(serde_json::Map::new());
        let request = match method.to_ascii_uppercase().as_str() {
            "GET" => self.http.get(&url),
            "POST" => self.http.post(&url).json(payload.unwrap_or(&empty_object)),
            other => {
                let parsed_method = reqwest::Method::from_bytes(other.as_bytes())
                    .map_err(|error| ServiceError::Http(error.to_string()))?;
                let mut builder = self.http.request(parsed_method, &url);
                if let Some(body) = payload {
                    builder = builder.json(body);
                }
                builder
            }
        };
        let response = request
            .basic_auth("admin", Some(admin_password))
            .send()
            .await
            .map_err(|error| ServiceError::Http(error.to_string()))?;
        let status_code = response.status().as_u16();
        let is_json = response
            .headers()
            .get(reqwest::header::CONTENT_TYPE)
            .and_then(|value| value.to_str().ok())
            .map(|value| value.starts_with("application/json"))
            .unwrap_or(false);
        let data = if is_json {
            response
                .json::<Value>()
                .await
                .map_err(|error| ServiceError::Http(error.to_string()))?
        } else {
            Value::String(
                response
                    .text()
                    .await
                    .map_err(|error| ServiceError::Http(error.to_string()))?,
            )
        };
        Ok(serde_json::json!({ "status_code": status_code, "data": data }))
    }

    pub async fn get_player_count(&self, host: &str, port: u16, admin_password: &str) -> u64 {
        match self
            .rest_api_call(host, port, admin_password, "players", "GET", None)
            .await
        {
            Ok(result) => result
                .get("data")
                .and_then(|data| data.get("players"))
                .and_then(|players| players.as_array())
                .map(|players| players.len() as u64)
                .unwrap_or(0),
            Err(_) => 0,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::extract::State;
    use axum::http::{HeaderMap, StatusCode};
    use axum::routing::{get, post};
    use axum::{Json, Router};
    use std::sync::{Arc, Mutex};

    #[derive(Default, Clone)]
    struct Captured {
        auth_header: Arc<Mutex<Option<String>>>,
        body: Arc<Mutex<Option<serde_json::Value>>>,
    }

    async fn spawn_stub(captured: Captured) -> u16 {
        let players = {
            let captured = captured.clone();
            move |headers: HeaderMap| {
                let captured = captured.clone();
                async move {
                    *captured.auth_header.lock().unwrap() = headers
                        .get("authorization")
                        .map(|value| value.to_str().unwrap().to_string());
                    Json(serde_json::json!({"players": [{"name": "a"}, {"name": "b"}]}))
                }
            }
        };
        let announce = {
            let captured = captured.clone();
            move |State(_): State<()>, Json(body): Json<serde_json::Value>| {
                let captured = captured.clone();
                async move {
                    *captured.body.lock().unwrap() = Some(body);
                    "OK"
                }
            }
        };
        let server_error = || async {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({"error": "boom"})),
            )
        };
        let router = Router::new()
            .route("/v1/api/players", get(players))
            .route("/v1/api/announce", post(announce))
            .route("/v1/api/error", get(server_error))
            .with_state(());
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = listener.local_addr().unwrap().port();
        tokio::spawn(async move { axum::serve(listener, router).await.unwrap() });
        port
    }

    #[tokio::test]
    async fn get_call_sends_basic_auth_and_wraps_json_response() {
        let captured = Captured::default();
        let port = spawn_stub(captured.clone()).await;
        let client = PalworldApiClient::new();
        let result = client
            .rest_api_call("127.0.0.1", port, "secret", "players", "GET", None)
            .await
            .unwrap();
        assert_eq!(result["status_code"], 200);
        assert_eq!(result["data"]["players"].as_array().unwrap().len(), 2);
        // Basic base64("admin:secret") == "Basic YWRtaW46c2VjcmV0"
        assert_eq!(
            captured.auth_header.lock().unwrap().as_deref(),
            Some("Basic YWRtaW46c2VjcmV0")
        );
    }

    #[tokio::test]
    async fn post_call_defaults_missing_payload_to_empty_object_and_returns_text() {
        let captured = Captured::default();
        let port = spawn_stub(captured.clone()).await;
        let client = PalworldApiClient::new();
        let result = client
            .rest_api_call("127.0.0.1", port, "secret", "announce", "POST", None)
            .await
            .unwrap();
        assert_eq!(result["status_code"], 200);
        assert_eq!(result["data"], serde_json::json!("OK"));
        assert_eq!(
            captured.body.lock().unwrap().clone().unwrap(),
            serde_json::json!({})
        );
    }

    #[tokio::test]
    async fn get_call_passes_non_2xx_status_through_as_data_instead_of_erroring() {
        let captured = Captured::default();
        let port = spawn_stub(captured).await;
        let client = PalworldApiClient::new();
        let result = client
            .rest_api_call("127.0.0.1", port, "secret", "error", "GET", None)
            .await
            .unwrap();
        assert_eq!(result["status_code"], 500);
        assert_eq!(result["data"], serde_json::json!({"error": "boom"}));
    }

    #[tokio::test]
    async fn get_player_count_counts_players_and_swallows_errors() {
        let captured = Captured::default();
        let port = spawn_stub(captured).await;
        let client = PalworldApiClient::new();
        assert_eq!(
            client.get_player_count("127.0.0.1", port, "secret").await,
            2
        );
        // Nothing listens on port 1 — must return 0, not error.
        assert_eq!(client.get_player_count("127.0.0.1", 1, "secret").await, 0);
    }
}
