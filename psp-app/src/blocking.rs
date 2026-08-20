//! Offloads blocking work (save file IO, uesave parsing, zip/base64 codecs)
//! from async handlers onto tokio's blocking pool.
//!
//! wasm32 has no blocking pool (single-threaded runtime), so the same call
//! sites run inline there — handlers stay shared between native and webapp.

/// Runs a blocking closure on the blocking pool natively. On wasm32 there is
/// no blocking pool (single-threaded runtime) — run inline instead.
///
/// A panic inside the closure resumes on the awaiting task, where the
/// dispatcher's `catch_unwind` turns it into an `error` frame exactly like an
/// inline panic would.
#[cfg(not(target_arch = "wasm32"))]
pub async fn run_blocking<T, F>(f: F) -> T
where
    T: Send + 'static,
    F: FnOnce() -> T + Send + 'static,
{
    tokio::task::spawn_blocking(f)
        .await
        .unwrap_or_else(|join_error| std::panic::resume_unwind(join_error.into_panic()))
}

#[cfg(target_arch = "wasm32")]
pub async fn run_blocking<T, F>(f: F) -> T
where
    F: FnOnce() -> T,
{
    f()
}
