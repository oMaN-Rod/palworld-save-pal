use std::path::PathBuf;
use std::sync::Arc;

use axum::body::Body;
use axum::http::{Request, StatusCode};
use http_body_util::BodyExt;
use tower::ServiceExt;

use psp_server::router::build_router;
use psp_server::{AppState, ServerConfig};

async fn test_router(temp_dir: &tempfile::TempDir) -> axum::Router {
    // Real repo game data; synthetic UI dir.
    let ui_dir = temp_dir.path().join("ui");
    std::fs::create_dir_all(ui_dir.join("assets")).unwrap();
    std::fs::write(ui_dir.join("index.html"), "<html>psp</html>").unwrap();
    std::fs::write(ui_dir.join("assets/app.js"), "console.log('ok')").unwrap();

    let config = ServerConfig {
        host: "127.0.0.1".parse().unwrap(),
        port: 0,
        ui_dir,
        data_dir: PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../data"),
        db_path: temp_dir.path().join("test.db"),
        desktop_mode: false,
    };
    let db = psp_db::open(&config.db_path).await.unwrap();
    let game_data =
        Arc::new(psp_core::gamedata::GameData::load(&config.data_dir.join("json")).unwrap());
    build_router(Arc::new(AppState {
        config,
        game_data,
        db,
    }))
}

#[tokio::test]
async fn root_serves_index_html() {
    let temp_dir = tempfile::tempdir().unwrap();
    let router = test_router(&temp_dir).await;
    let response = router
        .oneshot(Request::get("/").body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let body = response.into_body().collect().await.unwrap().to_bytes();
    assert_eq!(&body[..], b"<html>psp</html>");
}

#[tokio::test]
async fn static_asset_is_served() {
    let temp_dir = tempfile::tempdir().unwrap();
    let router = test_router(&temp_dir).await;
    let response = router
        .oneshot(Request::get("/assets/app.js").body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let body = response.into_body().collect().await.unwrap().to_bytes();
    assert_eq!(&body[..], b"console.log('ok')");
}

#[tokio::test]
async fn unknown_path_redirects_to_spa_root_with_encoded_path() {
    let temp_dir = tempfile::tempdir().unwrap();
    let router = test_router(&temp_dir).await;
    let response = router
        .oneshot(Request::get("/edit/pal%20box").body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::TEMPORARY_REDIRECT);
    let location = response
        .headers()
        .get("location")
        .unwrap()
        .to_str()
        .unwrap();
    assert_eq!(location, "/?path=/edit/pal%20box");
}

#[tokio::test]
async fn api_and_ws_paths_bypass_the_spa_redirect() {
    let temp_dir = tempfile::tempdir().unwrap();
    let router = test_router(&temp_dir).await;
    let api_response = router
        .clone()
        .oneshot(Request::get("/api/nope").body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(api_response.status(), StatusCode::NOT_FOUND);
    let ws_response = router
        .oneshot(Request::get("/ws").body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(ws_response.status(), StatusCode::NOT_FOUND);
}

// The real ui_build layout has directories like edit/ that hold per-page
// files (pal.html, guild.html, ...) but no edit/index.html of their own.
// A request for "/edit" must redirect straight to "/?path=/edit" in one
// hop, not bounce through an intermediate "add a trailing slash" redirect
// first (which would turn it into "/?path=/edit/").
#[tokio::test]
async fn directory_without_index_html_redirects_in_one_hop_without_adding_a_slash() {
    let temp_dir = tempfile::tempdir().unwrap();
    let router = test_router(&temp_dir).await;
    let edit_dir = temp_dir.path().join("ui/edit");
    std::fs::create_dir_all(&edit_dir).unwrap();
    std::fs::write(edit_dir.join("pal.html"), "<html>pal</html>").unwrap();

    let response = router
        .oneshot(Request::get("/edit").body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::TEMPORARY_REDIRECT);
    let location = response
        .headers()
        .get("location")
        .unwrap()
        .to_str()
        .unwrap();
    assert_eq!(location, "/?path=/edit");
}

#[tokio::test]
async fn directory_with_index_html_is_served_without_a_trailing_slash() {
    let temp_dir = tempfile::tempdir().unwrap();
    let router = test_router(&temp_dir).await;
    let docs_dir = temp_dir.path().join("ui/docs");
    std::fs::create_dir_all(&docs_dir).unwrap();
    std::fs::write(docs_dir.join("index.html"), "<html>docs</html>").unwrap();

    let response = router
        .oneshot(Request::get("/docs").body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let body = response.into_body().collect().await.unwrap().to_bytes();
    assert_eq!(&body[..], b"<html>docs</html>");
}

#[tokio::test]
async fn path_traversal_outside_ui_dir_is_not_served() {
    let temp_dir = tempfile::tempdir().unwrap();
    let router = test_router(&temp_dir).await;
    std::fs::write(temp_dir.path().join("secret.txt"), "top secret").unwrap();

    let response = router
        .oneshot(Request::get("/../secret.txt").body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_ne!(response.status(), StatusCode::OK);
    let body = response.into_body().collect().await.unwrap().to_bytes();
    assert!(
        !body.starts_with(b"top secret"),
        "path traversal leaked the secret file's contents"
    );
}
