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

fn body_leaks_secret(body: &[u8], secret: &[u8]) -> bool {
    !secret.is_empty() && body.windows(secret.len()).any(|window| window == secret)
}

// Percent-encoded, mixed-encoding, doubly-encoded, and Windows-backslash
// variants of the same attack, as an actual client would send them (a
// literal ".." request is easy to filter for; these are the forms that
// slip past naive checks). Every one of these ends up 404-ing inside
// ServeDir (tower-http-0.6.11 rejects any decoded Component::ParentDir in
// `build_and_validate_path`, src/services/fs/serve_dir/mod.rs), and
// spa_fallback_redirect then treats that 404 exactly like any other
// unmatched SPA route: a 307 to `/?path=<re-encoded decoded path>`. The
// exact status therefore isn't the interesting assertion (it's a redirect,
// not a 404, for every case here) -- what matters, and what is asserted
// for every case, is that secret.txt's bytes never appear in the body.
#[tokio::test]
async fn path_traversal_percent_and_backslash_forms_are_not_served() {
    let temp_dir = tempfile::tempdir().unwrap();
    let router = test_router(&temp_dir).await;
    let secret = b"top secret contents";
    std::fs::write(temp_dir.path().join("secret.txt"), secret).unwrap();

    let cases = [
        // %2e%2e%2f decodes in a single pass to "../" -- identical to a
        // literal ".." request once decoded.
        ("/%2e%2e%2fsecret.txt", "/?path=/../secret.txt"),
        // Mixed form: only the slash is percent-encoded.
        ("/..%2fsecret.txt", "/?path=/../secret.txt"),
        // Doubly-encoded: a single percent-decode pass turns "%25" into
        // '%' but leaves the following "2e"/"2f" as literal text (decoding
        // is one pass, not recursive), so the decoded path is
        // "/%2e%2e%2fsecret.txt" -- a request for one oddly-named file,
        // with no ".." component and no extra "/". It is safe not because
        // anything recognized a traversal attempt, but because a single
        // decode of this input never produces one.
        (
            "/%252e%252e%252fsecret.txt",
            "/?path=/%252e%252e%252fsecret.txt",
        ),
        // Windows-style backslash, sent raw. The `http` crate's URI parser
        // accepts a literal backslash unescaped in a path (byte 0x5C falls
        // in its allowed range). On this host, std::path::Path treats '\\'
        // as a separator, so both spa_fallback_redirect's own
        // has_parent_dir_segment check and ServeDir's std::path-based
        // component check see a ParentDir component here and reject it.
        ("/..\\secret.txt", "/?path=/..%5Csecret.txt"),
        // Same attack, percent-encoded. Decodes to the identical
        // "..\\secret.txt" and is rejected the same way.
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

// Targets spa_fallback_redirect's own directory/index.html pre-check
// specifically (as opposed to ServeDir's independent defense above): a
// directory *outside* ui_dir that itself contains an index.html, reached
// via "..". If the pre-check's has_parent_dir_segment guard were missing,
// the pre-check would stat "ui_dir/../secret_dir/index.html" (which does
// resolve, since the OS -- unlike Rust's lexical Path::components --
// collapses ".." during the actual stat syscall) and rewrite the request
// URI to serve it. Confirmed this can't happen: see the report's
// "layered defenses" section for what neutering the guard actually showed.
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

// axum's `request.uri().path()` is the raw, still percent-encoded path; a
// literal '?' can never appear in it (the http crate's URI parser splits
// path from query at the first unescaped '?'). The only way to get a '?'
// into the *decoded* path is to send it percent-encoded, which is exactly
// what a client linking to a save named e.g. "what?.sav" would do.
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
    // decoded path "/pals/foo?bar"; Python's quote(path, safe="/") escapes
    // '?' (not in unreserved) to %3F, so this round-trips unchanged.
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
    // decoded path "/pals/foo#bar"; '#' is escaped to %23 by quote(), same
    // as here.
    assert_eq!(location, "/?path=/pals/foo%23bar");
}

// Non-ASCII input, sent the way a real client would: percent-encoded UTF-8
// bytes. "%E3%83%91%E3%83%AB" is the UTF-8 encoding of "パル" (U+30D1
// U+30EB). Python's quote() always percent-encodes bytes outside its
// unreserved set, which includes every non-ASCII byte, so the decoded
// string round-trips to the identical percent-encoded bytes, uppercase hex.
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

// '~', '.', '-', '_' are all in Python's unreserved set (quote()'s default
// safe="/" plus the always-unreserved set) and must pass through
// unescaped. A test that only checks that reserved characters get escaped
// can't catch a QUOTE_SET that over-escapes these.
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
