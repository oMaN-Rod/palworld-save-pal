//! A host-supplied Oodle codec, for targets that cannot link one themselves.
//!
//! `uesave/oodle` wraps ooz-rs, eleven C++ files against libstdc++, which
//! wasm32-unknown-unknown has no toolchain for -- so the web build compiles
//! psp-core without the `oodle` feature and every `PlM` read or write inside it
//! fails with "Compression support not enabled". The browser has a codec all
//! the same: the worker's vendored `ooz.wasm`, already used to unwrap `.sav`
//! files before they reach us. This module is where it is lent to the core.
//!
//! Registration is per-thread, matching how the web worker owns one wasm
//! instance on one thread; a native process that links the real codec never
//! installs a bridge and is unaffected.

use std::cell::RefCell;

type Compress = Box<dyn Fn(&[u8]) -> Result<Vec<u8>, String>>;
type Decompress = Box<dyn Fn(&[u8], usize) -> Result<Vec<u8>, String>>;

thread_local! {
    static BRIDGE: RefCell<Option<(Compress, Decompress)>> = const { RefCell::new(None) };
}

/// Lends this thread an Oodle codec. `decompress` is handed the compressed
/// payload and the uncompressed length the container's header declares.
pub fn set_bridge(
    compress: impl Fn(&[u8]) -> Result<Vec<u8>, String> + 'static,
    decompress: impl Fn(&[u8], usize) -> Result<Vec<u8>, String> + 'static,
) {
    BRIDGE.with(|bridge| {
        *bridge.borrow_mut() = Some((Box::new(compress), Box::new(decompress)));
    });
}

pub fn is_installed() -> bool {
    BRIDGE.with(|bridge| bridge.borrow().is_some())
}

pub(crate) fn compress(data: &[u8]) -> Result<Vec<u8>, crate::error::CoreError> {
    with_bridge(|(compress, _)| compress(data))
}

pub(crate) fn decompress(
    payload: &[u8],
    uncompressed_len: usize,
) -> Result<Vec<u8>, crate::error::CoreError> {
    with_bridge(|(_, decompress)| decompress(payload, uncompressed_len))
}

fn with_bridge(
    call: impl FnOnce(&(Compress, Decompress)) -> Result<Vec<u8>, String>,
) -> Result<Vec<u8>, crate::error::CoreError> {
    BRIDGE.with(|bridge| match bridge.borrow().as_ref() {
        Some(codec) => call(codec).map_err(crate::error::CoreError::Other),
        None => Err(crate::error::CoreError::Other(
            "no Oodle codec is available on this thread".to_string(),
        )),
    })
}
