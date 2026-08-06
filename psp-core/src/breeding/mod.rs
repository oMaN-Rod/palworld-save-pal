//! Palworld breeding calculator engine.
//!
//! Pure data + logic, zero deps on save-decode code. Consumes plain JSON
//! (loaded via [`crate::gamedata::GameData`]) and produces breeding answers.
//!
//! Faithful port of `PalSavTools/src/palworld_aio/breeding/`.
//!
//! Public surface mirrors the Python package:
//! - [`BreedingDB`] — indexes + lookups
//! - [`direct_child`], [`direct_partners`], [`direct_parents`] — Direct Mode
//! - [`solve`] — chain solver (Selection + Save Mode)
//! - [`OwnedSource`], [`SelectedSource`], [`WildSource`], [`CompositeSource`]
//! - [`min_steps`], [`can_reach`] — reachability

pub mod data;
pub mod direct;
pub mod graph;
pub mod model;
pub mod solver;
pub mod sources;

pub use data::BreedingDB;
pub use direct::{direct_child, direct_parents, direct_partners};
pub use graph::{can_reach, min_steps};
pub use model::{
    BreedingSpec, BreedingStep, Chain, ChainSource, ComboType, DirectResult, Gender, GenderProb,
    Origin, PalRef,
};
pub use solver::solve;
pub use sources::{
    normalize_species, CompositeSource, OwnedPalInput, OwnedSource, SelectedPalInput,
    SelectedSource, SourceAdapter, WildSource,
};

/// Errors raised while building or using the breeding database.
#[derive(Debug, thiserror::Error)]
pub enum BreedingError {
    #[error("missing breeding data file in GameData: {0}")]
    MissingData(String),
    #[error("breeding data parse error: {0}")]
    Parse(#[from] serde_json::Error),
}
