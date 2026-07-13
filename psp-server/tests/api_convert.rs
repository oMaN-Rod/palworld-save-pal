use std::path::PathBuf;
use std::sync::Arc;

use axum::body::{Body, Bytes};
use axum::http::{Request, StatusCode};
use http_body_util::BodyExt;
use tower::ServiceExt;

use psp_server::router::build_router;
use psp_server::{AppState, ServerConfig};

/// A plain-GVAS (non-Palworld) save, vendored so this test needs no external
/// checkout.
fn sample_save_bytes() -> Vec<u8> {
    let path =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../tests/fixtures/drg-save-test.sav");
    std::fs::read(path).expect("vendored tests/fixtures/drg-save-test.sav")
}

async fn test_router(temp_dir: &tempfile::TempDir) -> axum::Router {
    let config = ServerConfig {
        host: "127.0.0.1".parse().unwrap(),
        port: 0,
        ui_dir: temp_dir.path().join("ui"),
        data_dir: PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../data"),
        db_path: temp_dir.path().join("test.db"),
        desktop_mode: false,
    };
    let db = psp_db::open(&config.db_path).await.unwrap();
    let game_data =
        Arc::new(psp_core::gamedata::GameData::load(&config.data_dir.join("json")).unwrap());
    let (live_connections, _live_connections_rx) = tokio::sync::watch::channel(0usize);
    let server_services = Arc::new(psp_server::services::ServerServices::with_docker(Arc::new(
        psp_server::services::docker::mock::MockDocker::default(),
    )));
    build_router(Arc::new(AppState {
        config,
        game_data,
        db,
        dialogs: Arc::new(psp_server::desktop_dialogs::NullDialogProvider),
        live_connections,
        server_services,
        sessions: std::sync::Mutex::new(psp_server::SessionStore::default()),
    }))
}

fn multipart_file_request(uri: &str, file_bytes: &[u8]) -> Request<Body> {
    let boundary = "PSP_TEST_BOUNDARY";
    let mut body = Vec::new();
    body.extend_from_slice(format!("--{boundary}\r\n").as_bytes());
    body.extend_from_slice(
        b"Content-Disposition: form-data; name=\"file\"; filename=\"test.sav\"\r\n\
          Content-Type: application/octet-stream\r\n\r\n",
    );
    body.extend_from_slice(file_bytes);
    body.extend_from_slice(format!("\r\n--{boundary}--\r\n").as_bytes());
    Request::post(uri)
        .header(
            "content-type",
            format!("multipart/form-data; boundary={boundary}"),
        )
        .body(Body::from(body))
        .unwrap()
}

async fn body_bytes(response: axum::response::Response) -> Bytes {
    response.into_body().collect().await.unwrap().to_bytes()
}

#[tokio::test]
async fn sav_to_json_to_sav_round_trips() {
    let temp_dir = tempfile::tempdir().unwrap();
    let router = test_router(&temp_dir).await;

    let response = router
        .clone()
        .oneshot(multipart_file_request(
            "/api/convert/sav-to-json",
            &sample_save_bytes(),
        ))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(
        response
            .headers()
            .get("content-type")
            .unwrap()
            .to_str()
            .unwrap(),
        "application/json"
    );
    let first_json = body_bytes(response).await;

    let response = router
        .clone()
        .oneshot(
            Request::post("/api/convert/json-to-sav")
                .header("content-type", "application/json")
                .body(Body::from(first_json.clone()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(
        response
            .headers()
            .get("content-disposition")
            .unwrap()
            .to_str()
            .unwrap(),
        "attachment; filename=converted.sav"
    );
    let compressed_save = body_bytes(response).await;
    // .sav layout: [uncompressed_len u32][compressed_len u32][magic 3B][save_type 1B],
    // so the "PlM" magic sits at bytes 8..11 and the save type at byte 11.
    assert_eq!(
        &compressed_save[8..11],
        b"PlM",
        "expected Oodle-compressed output"
    );
    assert_eq!(
        compressed_save[11], 0x31,
        "expected save_type byte 0x31 (SaveType.PLM)"
    );

    // The PlM sav must convert back to identical JSON.
    let response = router
        .oneshot(multipart_file_request(
            "/api/convert/sav-to-json",
            &compressed_save,
        ))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let second_json = body_bytes(response).await;
    let first: serde_json::Value = serde_json::from_slice(&first_json).unwrap();
    let second: serde_json::Value = serde_json::from_slice(&second_json).unwrap();
    pretty_assertions::assert_eq!(first, second);
}

#[tokio::test]
async fn sav_to_json_without_file_field_is_422() {
    let temp_dir = tempfile::tempdir().unwrap();
    let router = test_router(&temp_dir).await;
    let boundary = "PSP_TEST_BOUNDARY";
    let body = format!(
        "--{boundary}\r\nContent-Disposition: form-data; name=\"other\"\r\n\r\nx\r\n--{boundary}--\r\n"
    );
    let response = router
        .oneshot(
            Request::post("/api/convert/sav-to-json")
                .header(
                    "content-type",
                    format!("multipart/form-data; boundary={boundary}"),
                )
                .body(Body::from(body))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::UNPROCESSABLE_ENTITY);
}

#[tokio::test]
async fn sav_to_json_with_corrupt_bytes_is_500() {
    let temp_dir = tempfile::tempdir().unwrap();
    let router = test_router(&temp_dir).await;
    let response = router
        .oneshot(multipart_file_request(
            "/api/convert/sav-to-json",
            b"not a save file at all, way too short and wrong magic",
        ))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::INTERNAL_SERVER_ERROR);
}

#[tokio::test]
async fn json_to_sav_with_garbage_body_is_500() {
    let temp_dir = tempfile::tempdir().unwrap();
    let router = test_router(&temp_dir).await;
    let response = router
        .oneshot(
            Request::post("/api/convert/json-to-sav")
                .body(Body::from("not json at all"))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::INTERNAL_SERVER_ERROR);
}
