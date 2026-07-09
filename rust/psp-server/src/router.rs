use std::sync::Arc;

use axum::middleware;
use axum::Router;
use tower_http::services::ServeDir;

use crate::static_files::spa_fallback_redirect;
use crate::AppState;

pub fn build_router(state: Arc<AppState>) -> Router {
    // Directory index resolution happens in spa_fallback_redirect, so
    // ServeDir is only ever asked for paths already confirmed to be files.
    let serve_ui = ServeDir::new(&state.config.ui_dir).append_index_html_on_directories(false);

    Router::new()
        .route("/ws/{client_id}", axum::routing::get(crate::ws::ws_upgrade))
        .nest("/api/convert", crate::api_convert::routes())
        .fallback_service(serve_ui)
        .layer(middleware::from_fn_with_state(
            state.clone(),
            spa_fallback_redirect,
        ))
        .with_state(state)
}
