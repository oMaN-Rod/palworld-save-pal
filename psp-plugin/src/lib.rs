//! Nothing in this crate may panic: it links into `psp-web`, where `panic =
//! abort` turns a panic into a dead module with no error frame.

pub mod context;
pub mod host;
pub mod manifest;
pub mod modules;
pub mod runtime;
pub mod sandbox;
pub mod status;
pub mod syntax;

pub use host::api_def::{ApiDefinition, api_definition};
pub use host::api_meta::lua_meta;
pub use host::fields::{Access, FieldSpec};
pub use host::fields::pal::PAL_FIELDS;
pub use host::fields::player::PLAYER_FIELDS;
