use std::path::Path;
use std::sync::Arc;

use axum::middleware;
use axum::Router;
use tower_http::services::ServeDir;

use crate::static_files::spa_fallback_redirect;
use crate::AppState;

pub fn build_router(state: Arc<AppState>, ui_dir: &Path) -> Router {
    // Directory index resolution happens in spa_fallback_redirect, so
    // ServeDir is only ever asked for paths already confirmed to be files.
    // `ui_dir` must be the directory the server was configured with; it feeds
    // both the ServeDir below and the SPA index precheck in the middleware.
    let serve_ui = ServeDir::new(ui_dir).append_index_html_on_directories(false);
    let ui_dir: Arc<Path> = Arc::from(ui_dir);

    Router::new()
        .route("/ws/{client_id}", axum::routing::get(crate::ws::ws_upgrade))
        .nest("/api/convert", crate::api_convert::routes())
        .fallback_service(serve_ui)
        .layer(middleware::from_fn_with_state(
            ui_dir,
            spa_fallback_redirect,
        ))
        .with_state(state)
}
