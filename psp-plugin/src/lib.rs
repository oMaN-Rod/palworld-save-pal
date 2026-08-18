//! The safe layer over `psp-lua-sys`: sandbox, capabilities, host API, run loop.
//!
//! Nothing in this crate may panic. It links into `psp-web`, where `panic =
//! abort` turns a panic into a dead module with no error frame, so every
//! fallible path returns a status instead.

pub mod state;
