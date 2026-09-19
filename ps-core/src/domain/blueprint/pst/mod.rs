//! Reads PalworldSaveTools base blueprints into PalStudio's native form.
//!
//! PST is being retired; this is a one-way migration path. All GVAS parsing lives in
//! `uesave`; this module owns only the container format and the assembly around it.

pub mod envelope;
