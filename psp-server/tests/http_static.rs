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

// The real UI build has directories like edit/ holding per-page files
// (pal.html, guild.html, ...) but no edit/index.html. "/edit" must redirect
// straight to "/?path=/edit" in ONE hop; an intermediate add-a-trailing-slash
// redirect would corrupt the target into "/?path=/edit/".
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

fn body_leaks_secret(body: &[u8], secret: &[u8]) -> bool {
    !secret.is_empty() && body.windows(secret.len()).any(|window| window == secret)
}

// Encoded traversal forms, as a real client would send them. Each one ends up
// rejected inside ServeDir and then handled by the SPA fallback as an ordinary
// unmatched route (a 307, not a 404) -- so the status is not the interesting
// assertion. The invariant is that secret.txt's bytes never reach the body.
#[tokio::test]
async fn path_traversal_percent_and_backslash_forms_are_not_served() {
    let temp_dir = tempfile::tempdir().unwrap();
    let router = test_router(&temp_dir).await;
    let secret = b"top secret contents";
    std::fs::write(temp_dir.path().join("secret.txt"), secret).unwrap();

    let cases = [
        // Fully percent-encoded "../".
        ("/%2e%2e%2fsecret.txt", "/?path=/../secret.txt"),
        // Mixed form: only the slash is percent-encoded.
        ("/..%2fsecret.txt", "/?path=/../secret.txt"),
        // Doubly-encoded. Decoding is a single pass, so "%25" becomes '%' and
        // the trailing "2e"/"2f" stay literal: the decoded path is one
        // oddly-named file with no ".." component at all -- hence the
        // pass-through redirect target.
        (
            "/%252e%252e%252fsecret.txt",
            "/?path=/%252e%252e%252fsecret.txt",
        ),
        // Raw Windows backslash (the URI parser allows it unescaped). On this
        // host std::path treats '\' as a separator, so the ParentDir component
        // is seen and rejected.
        ("/..\\secret.txt", "/?path=/..%5Csecret.txt"),
        ("/..%5csecret.txt", "/?path=/..%5Csecret.txt"),
    ];

    for (request_path, expected_location) in cases {
        let response = router
            .clone()
            .oneshot(Request::get(request_path).body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_ne!(
            response.status(),
            StatusCode::OK,
            "request {request_path:?} must not be served directly"
        );
        let location = response
            .headers()
            .get("location")
            .map(|value| value.to_str().unwrap().to_owned());
        assert_eq!(
            location.as_deref(),
            Some(expected_location),
            "request {request_path:?} redirect target"
        );
        let body = response.into_body().collect().await.unwrap().to_bytes();
        assert!(
            !body_leaks_secret(&body, secret),
            "request {request_path:?} leaked the secret file's contents"
        );
    }
}

// Targets spa_fallback_redirect's own directory/index.html pre-check rather
// than ServeDir's independent defense: without the pre-check's
// has_parent_dir_segment guard, it would stat "ui_dir/../secret_dir/index.html"
// -- which the OS resolves, unlike Rust's lexical Path::components -- and serve
// a file outside ui_dir.
#[tokio::test]
async fn path_traversal_to_a_directory_with_index_html_is_not_served() {
    let temp_dir = tempfile::tempdir().unwrap();
    let router = test_router(&temp_dir).await;
    let secret = b"top secret index contents";
    let secret_dir = temp_dir.path().join("secret_dir");
    std::fs::create_dir_all(&secret_dir).unwrap();
    std::fs::write(secret_dir.join("index.html"), secret).unwrap();

    let response = router
        .oneshot(Request::get("/../secret_dir").body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_ne!(response.status(), StatusCode::OK);
    let body = response.into_body().collect().await.unwrap().to_bytes();
    assert!(
        !body_leaks_secret(&body, secret),
        "path traversal to a directory with its own index.html leaked its contents"
    );
}

// A literal '?' can never reach `request.uri().path()` (the URI parser splits
// the query there), so the only way one lands in the DECODED path is
// percent-encoded -- what a client linking to a save named "what?.sav" sends.
// The redirect must re-escape it rather than emit a second query separator.
#[tokio::test]
async fn redirect_escapes_percent_encoded_question_mark() {
    let temp_dir = tempfile::tempdir().unwrap();
    let router = test_router(&temp_dir).await;
    let response = router
        .oneshot(Request::get("/pals/foo%3Fbar").body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::TEMPORARY_REDIRECT);
    let location = response
        .headers()
        .get("location")
        .unwrap()
        .to_str()
        .unwrap();
    assert_eq!(location, "/?path=/pals/foo%3Fbar");
}

// Same reasoning as the '?' case: '#' cannot appear raw in
// request.uri().path() either (the URI parser treats it as the start of a
// fragment), so it must arrive percent-encoded to land in the decoded path.
#[tokio::test]
async fn redirect_escapes_percent_encoded_hash() {
    let temp_dir = tempfile::tempdir().unwrap();
    let router = test_router(&temp_dir).await;
    let response = router
        .oneshot(Request::get("/pals/foo%23bar").body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::TEMPORARY_REDIRECT);
    let location = response
        .headers()
        .get("location")
        .unwrap()
        .to_str()
        .unwrap();
    assert_eq!(location, "/?path=/pals/foo%23bar");
}

// "%E3%83%91%E3%83%AB" is UTF-8 for "パル" (U+30D1 U+30EB). Every non-ASCII
// byte is outside the unreserved set, so the decoded string must round-trip to
// the identical percent-encoded bytes, in uppercase hex.
#[tokio::test]
async fn redirect_escapes_non_ascii_utf8_bytes() {
    let temp_dir = tempfile::tempdir().unwrap();
    let router = test_router(&temp_dir).await;
    let response = router
        .oneshot(
            Request::get("/pals/%E3%83%91%E3%83%AB")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::TEMPORARY_REDIRECT);
    let location = response
        .headers()
        .get("location")
        .unwrap()
        .to_str()
        .unwrap();
    assert_eq!(location, "/?path=/pals/%E3%83%91%E3%83%AB");
}

// '~', '.', '-', '_' are unreserved and must pass through UNESCAPED. The
// escaping tests above can only catch under-escaping; this one catches a
// QUOTE_SET that over-escapes.
#[tokio::test]
async fn redirect_leaves_unreserved_characters_unescaped() {
    let temp_dir = tempfile::tempdir().unwrap();
    let router = test_router(&temp_dir).await;
    let response = router
        .oneshot(Request::get("/pals/a~b.c-d_e").body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::TEMPORARY_REDIRECT);
    let location = response
        .headers()
        .get("location")
        .unwrap()
        .to_str()
        .unwrap();
    assert_eq!(location, "/?path=/pals/a~b.c-d_e");
}
