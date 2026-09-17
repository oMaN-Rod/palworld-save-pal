use std::path::Path;
use std::sync::Arc;

use axum::middleware;
use axum::Router;
use tower_http::compression::CompressionLayer;
use tower_http::services::ServeDir;

use crate::network::{network_gate, NetworkRuntime};
use crate::static_files::{cache_control_headers, spa_fallback_redirect};
use crate::AppState;

pub fn build_router(state: Arc<AppState>, ui_dir: &Path, network: Arc<NetworkRuntime>) -> Router {
    // Directory index resolution happens in spa_fallback_redirect, so
    // ServeDir is only ever asked for paths already confirmed to be files.
    let serve_ui = ServeDir::new(ui_dir).append_index_html_on_directories(false);
    let ui_dir: Arc<Path> = Arc::from(ui_dir);

    let router = Router::new()
        .route("/ws/{client_id}", axum::routing::get(crate::ws::ws_upgrade))
        .nest("/api/convert", crate::api_convert::routes())
        .nest(
            "/api/network",
            crate::network::routes().merge(crate::service_control::routes()),
        )
        .route(
            "/network-unlock",
            axum::routing::get(crate::network::unlock_page),
        )
        .fallback_service(serve_ui)
        .layer(axum::Extension(Arc::clone(&network)))
        .layer(middleware::from_fn_with_state(
            ui_dir,
            spa_fallback_redirect,
        ))
        .layer(middleware::from_fn(cache_control_headers));
    // Desktop serves over loopback, where gzip only burns CPU; the Docker/web
    // deployment is the one that pays per byte.
    let router = if state.config.desktop_mode {
        router
    } else {
        router.layer(CompressionLayer::new())
    };
    // PalStudio's own network policy is the OUTERMOST layer: every request,
    // including the websocket upgrade, is judged before anything else runs.
    router
        .layer(middleware::from_fn_with_state(network, network_gate))
        .with_state(state)
}
