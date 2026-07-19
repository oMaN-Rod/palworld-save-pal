//! POST /api/convert/* — .sav <-> JSON conversion. Reading accepts GVAS/PlM;
//! writing emits PlM/Oodle.

use std::io::Cursor;
use std::sync::Arc;

use axum::body::Bytes;
use axum::extract::{DefaultBodyLimit, Multipart};
use axum::http::{header, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::post;
use axum::Router;
use psp_core::ue::games::palworld::palworld_types;
use psp_core::ue::{Palworld, Save, SaveReader};

use crate::AppState;

/// Same ceiling as the WS channel — real Level.sav files are 100s of MB.
const MAX_CONVERT_BODY_BYTES: usize = 1 << 30;

pub fn routes() -> Router<Arc<AppState>> {
    Router::new()
        .route("/sav-to-json", post(sav_to_json))
        .route("/json-to-sav", post(json_to_sav))
        .layer(DefaultBodyLimit::max(MAX_CONVERT_BODY_BYTES))
}

async fn sav_to_json(mut multipart: Multipart) -> Response {
    let mut file_bytes: Option<Bytes> = None;
    while let Ok(Some(field)) = multipart.next_field().await {
        if field.name() == Some("file") {
            match field.bytes().await {
                Ok(bytes) => file_bytes = Some(bytes),
                Err(read_error) => {
                    return (StatusCode::BAD_REQUEST, read_error.to_string()).into_response()
                }
            }
            break;
        }
    }
    let Some(file_bytes) = file_bytes else {
        return (
            StatusCode::UNPROCESSABLE_ENTITY,
            "missing multipart field 'file'",
        )
            .into_response();
    };

    // Blocking parse of a potentially huge save — keep it off the async workers.
    let conversion = tokio::task::spawn_blocking(move || -> Result<String, String> {
        let save = SaveReader::new()
            .game::<Palworld>()
            .types(palworld_types())
            .read(Cursor::new(file_bytes.as_ref()))
            .map_err(|parse_error| parse_error.to_string())?;
        serde_json::to_string(&save).map_err(|serialize_error| serialize_error.to_string())
    })
    .await;

    match conversion {
        Ok(Ok(json_text)) => (
            StatusCode::OK,
            [(header::CONTENT_TYPE, "application/json")],
            json_text,
        )
            .into_response(),
        Ok(Err(conversion_error)) => {
            tracing::error!(%conversion_error, "sav-to-json failed");
            (StatusCode::INTERNAL_SERVER_ERROR, conversion_error).into_response()
        }
        Err(join_error) => {
            (StatusCode::INTERNAL_SERVER_ERROR, join_error.to_string()).into_response()
        }
    }
}

async fn json_to_sav(body: Bytes) -> Response {
    let conversion = tokio::task::spawn_blocking(move || -> Result<Vec<u8>, String> {
        let save: Save =
            serde_json::from_slice(&body).map_err(|parse_error| parse_error.to_string())?;
        let mut sav_bytes = Vec::new();
        save.write_plm(&mut sav_bytes)
            .map_err(|write_error| write_error.to_string())?;
        Ok(sav_bytes)
    })
    .await;

    match conversion {
        Ok(Ok(sav_bytes)) => (
            StatusCode::OK,
            [
                (header::CONTENT_TYPE, "application/octet-stream"),
                (
                    header::CONTENT_DISPOSITION,
                    "attachment; filename=converted.sav",
                ),
            ],
            sav_bytes,
        )
            .into_response(),
        Ok(Err(conversion_error)) => {
            tracing::error!(%conversion_error, "json-to-sav failed");
            (StatusCode::INTERNAL_SERVER_ERROR, conversion_error).into_response()
        }
        Err(join_error) => {
            (StatusCode::INTERNAL_SERVER_ERROR, join_error.to_string()).into_response()
        }
    }
}
