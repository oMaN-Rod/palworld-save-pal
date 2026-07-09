//! SPA fallback middleware for the built UI: serves a matching file or
//! directory index, otherwise redirects unmatched paths to the SPA root
//! with the original path preserved as a query parameter.
use std::path::Component;
use std::sync::Arc;

use axum::extract::{Request, State};
use axum::http::{StatusCode, Uri};
use axum::middleware::Next;
use axum::response::{IntoResponse, Redirect, Response};
use percent_encoding::{percent_decode_str, utf8_percent_encode, AsciiSet, NON_ALPHANUMERIC};

use crate::AppState;

/// Characters left unescaped when percent-encoding the redirect's path
/// query param: alphanumerics plus `_ . - ~ /`.
const QUOTE_SET: &AsciiSet = &NON_ALPHANUMERIC
    .remove(b'/')
    .remove(b'_')
    .remove(b'.')
    .remove(b'-')
    .remove(b'~');

pub async fn spa_fallback_redirect(
    State(state): State<Arc<AppState>>,
    mut request: Request,
    next: Next,
) -> Response {
    let raw_path = request.uri().path().to_owned();

    if raw_path.starts_with("/ws") || raw_path.starts_with("/api") {
        return next.run(request).await;
    }

    let decoded_path = percent_decode_str(&raw_path)
        .decode_utf8_lossy()
        .into_owned();

    // A directory with an index.html is served regardless of a trailing
    // slash on the request. ServeDir only does this when the URL already
    // ends in "/" (otherwise it redirects to add one), so directories are
    // resolved here and handed to ServeDir as a concrete file path.
    if !has_parent_dir_segment(&decoded_path) {
        let candidate = state
            .config
            .ui_dir
            .join(decoded_path.trim_start_matches('/'));
        if candidate.join("index.html").is_file() {
            *request.uri_mut() = append_index_html(request.uri());
        }
    }

    let response = next.run(request).await;

    if response.status() == StatusCode::NOT_FOUND && raw_path != "/" {
        let encoded_path = utf8_percent_encode(&decoded_path, QUOTE_SET).to_string();
        return Redirect::temporary(&format!("/?path={encoded_path}")).into_response();
    }
    response
}

fn has_parent_dir_segment(path: &str) -> bool {
    std::path::Path::new(path)
        .components()
        .any(|component| matches!(component, Component::ParentDir))
}

fn append_index_html(uri: &Uri) -> Uri {
    let path = uri.path();
    let new_path = if path.ends_with('/') {
        format!("{path}index.html")
    } else {
        format!("{path}/index.html")
    };
    let path_and_query = match uri.query() {
        Some(query) => format!("{new_path}?{query}"),
        None => new_path,
    };
    let mut parts = uri.clone().into_parts();
    parts.path_and_query = Some(path_and_query.parse().expect("valid path"));
    Uri::from_parts(parts).expect("valid uri")
}
