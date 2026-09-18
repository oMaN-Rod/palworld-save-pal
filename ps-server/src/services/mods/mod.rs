//! The impure half of the mod engine: archive extraction, the library on disk,
//! and the install pipeline. Every routing and layout decision is made by
//! `ps_core::mods` and called from here.

pub mod adopt;
pub mod conflicts;
pub mod deploy;
pub mod detect;
pub mod digest;
pub mod extract;
pub mod frameworks;
pub mod ini_text;
pub mod install;
pub mod iostore;
pub mod launch;
pub mod layout;
pub mod library;
pub mod paths;
pub mod running;
pub mod scan;
pub mod settings;
pub mod share;
pub mod uploads;
pub mod verifier;
pub mod verify;

pub use paths::LibraryPaths;
