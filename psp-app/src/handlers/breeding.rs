//! Breeding calculator handlers. Pure read-only computations over the cached
//! `BreedingDB` (sourced from `game_data`); no `Session` access. Both transports
//! (native WS + wasm) serve the same handlers.
//!
//! Save Mode and Selection Mode are unified on the backend: both send a `pals`
//! list tagged by `origin` ("owned" / "selected"); the solver treats them
//! identically. This follows PSP's parsing model — owned pals arrive from the
//! frontend's already-parsed `appState.selectedPlayer.pals` (which carries
//! `passive_skills`, unlike `PalSummary`).

// `std::time::Instant::now()` panics on `wasm32-unknown-unknown` (the target has
// no system clock — see `library/std/src/sys/time/unsupported.rs`). The
// `elapsed_ms` field below is diagnostic only, so on wasm we skip timing rather
// than pull in a clock crate. The import + both call sites are cfg-gated
// together below.
#[cfg(not(target_arch = "wasm32"))]
use std::time::Instant;

use serde::Deserialize;
use serde_json::{json, Value};

use psp_core::breeding::{
    self, direct_child, direct_parents, direct_partners, solve, BreedingSpec, CompositeSource,
    Gender, OwnedPalInput, OwnedSource, SelectedPalInput, SelectedSource, SourceAdapter,
    WildSource,
};

use crate::dispatcher::HandlerCtx;
use crate::handler_error::HandlerError;
use crate::messages::MessageType;

/// Emits `{error}` under `mt` and returns `Ok`, so a missing/malformed breeding
/// data set surfaces as a structured failure the frontend (which correlates by
/// type) can render — not a generic error frame.
macro_rules! db_or_soft_error {
    ($ctx:expr, $mt:expr) => {
        match $ctx.app.breeding_db() {
            Ok(db) => db,
            Err(e) => {
                $ctx.emitter.emit($mt, &json!({ "error": e.to_string() }));
                return Ok(());
            }
        }
    };
}

// ---------------------------------------------------------------------
// GET get_breeding_pals — breedable-pal picker list
// ---------------------------------------------------------------------
pub async fn handle_get_breeding_pals(ctx: &mut HandlerCtx<'_>) -> Result<(), HandlerError> {
    let mt = MessageType::GetBreedingPals;
    let db = db_or_soft_error!(ctx, mt);
    let tribes = db.breedable_tribes();
    let pals: Vec<Value> = tribes
        .iter()
        .map(|tribe| {
            json!({
                "tribe": tribe,
                "display_name": db.display_name(tribe),
                "icon": db.icon_path(tribe),
                "combi_rank": db.combi_rank(tribe),
                "rarity": db.rarity(tribe),
                "gender_prob": db.gender_probability(tribe),
            })
        })
        .collect();
    ctx.emitter
        .emit(mt, &json!({ "pals": pals, "total": tribes.len() }));
    Ok(())
}

// ---------------------------------------------------------------------
// POST breeding_direct_child — A + B → child
// ---------------------------------------------------------------------
#[derive(Debug, Deserialize)]
pub struct DirectChildData {
    pub parent_a: String,
    pub parent_b: String,
}

pub async fn handle_breeding_direct_child(
    data: DirectChildData,
    ctx: &mut HandlerCtx<'_>,
) -> Result<(), HandlerError> {
    let mt = MessageType::BreedingDirectChild;
    let db = db_or_soft_error!(ctx, mt);
    let results = direct_child(db, &data.parent_a, &data.parent_b);
    // `result` stays the single headline answer for back-compat; `results`
    // carries every outcome, which is >1 only for the gender-gated combos.
    ctx.emitter.emit(
        mt,
        &json!({ "result": results.first(), "results": results }),
    );
    Ok(())
}

// ---------------------------------------------------------------------
// POST breeding_direct_partners — A + target → candidate B list
// ---------------------------------------------------------------------
#[derive(Debug, Deserialize)]
pub struct DirectPartnersData {
    pub parent_a: String,
    pub target_child: String,
}

pub async fn handle_breeding_direct_partners(
    data: DirectPartnersData,
    ctx: &mut HandlerCtx<'_>,
) -> Result<(), HandlerError> {
    let mt = MessageType::BreedingDirectPartners;
    let db = db_or_soft_error!(ctx, mt);
    let partners = direct_partners(db, &data.parent_a, &data.target_child);
    ctx.emitter.emit(mt, &json!({ "partners": partners }));
    Ok(())
}

// ---------------------------------------------------------------------
// POST breeding_direct_parents — target → all parent pairs
// ---------------------------------------------------------------------
#[derive(Debug, Deserialize)]
pub struct DirectParentsData {
    pub target_child: String,
}

pub async fn handle_breeding_direct_parents(
    data: DirectParentsData,
    ctx: &mut HandlerCtx<'_>,
) -> Result<(), HandlerError> {
    let mt = MessageType::BreedingDirectParents;
    let db = db_or_soft_error!(ctx, mt);
    let parents = direct_parents(db, &data.target_child);
    ctx.emitter.emit(mt, &json!({ "parents": parents }));
    Ok(())
}

// ---------------------------------------------------------------------
// POST breeding_chain — chain solver (Selection + Save Mode)
// ---------------------------------------------------------------------
#[derive(Debug, Deserialize)]
pub struct ChainRequest {
    pub target_pal: String,
    #[serde(default)]
    pub required_passives: Vec<String>,
    pub target_gender: Option<String>,
    #[serde(default = "default_max_generations")]
    pub max_generations: u32,
    #[serde(default = "default_max_results")]
    pub max_results: u32,
    #[serde(default)]
    pub pals: Vec<PalInput>,
    #[serde(default)]
    pub include_wild: bool,
}

fn default_max_generations() -> u32 {
    5
}

fn default_max_results() -> u32 {
    5
}

/// A single input pal. `origin` distinguishes owned (save) vs selected
/// (theoretical) for display badges; the solver splits them into the
/// corresponding source adapters. Unrecognized origins default to "owned".
#[derive(Debug, Deserialize)]
pub struct PalInput {
    pub character_id: String,
    pub gender: Option<String>,
    #[serde(default)]
    pub passive_skills: Vec<String>,
    #[serde(default)]
    pub origin: String,
    pub instance_id: Option<Value>,
    pub nickname: Option<Value>,
    pub level: Option<Value>,
    pub owner_uid: Option<Value>,
}

pub async fn handle_breeding_chain(
    data: ChainRequest,
    ctx: &mut HandlerCtx<'_>,
) -> Result<(), HandlerError> {
    let mt = MessageType::BreedingChain;
    let db = db_or_soft_error!(ctx, mt);
    #[cfg(not(target_arch = "wasm32"))]
    let start = Instant::now();

    let mut owned: Vec<OwnedPalInput> = Vec::new();
    let mut selected: Vec<SelectedPalInput> = Vec::new();
    let mut warnings: Vec<String> = Vec::new();
    for pal in &data.pals {
        if pal.origin == "selected" {
            let species = breeding::normalize_species(Some(&pal.character_id));
            if species.is_empty() {
                continue;
            }
            if !db.is_breedable(&species) {
                warnings.push(format!("{species:?} is not in the breeding table; skipped"));
                continue;
            }
            selected.push(SelectedPalInput {
                species: pal.character_id.clone(),
                gender: pal.gender.clone(),
                passives: pal.passive_skills.clone(),
            });
        } else {
            owned.push(OwnedPalInput {
                character_id: pal.character_id.clone(),
                gender: pal.gender.clone(),
                passive_skills: pal.passive_skills.clone(),
                instance_id: pal.instance_id.clone(),
                nickname: pal.nickname.clone(),
                level: pal.level.clone(),
                owner_uid: pal.owner_uid.clone(),
            });
        }
    }

    let mut adapters: Vec<Box<dyn SourceAdapter>> = Vec::new();
    if !owned.is_empty() {
        adapters.push(Box::new(OwnedSource::new(owned)));
    }
    if !selected.is_empty() {
        adapters.push(Box::new(SelectedSource::new(selected)));
    }
    if data.include_wild {
        adapters.push(Box::new(WildSource::new()));
    }
    let composite = CompositeSource::new(adapters);

    let target_gender = data.target_gender.as_deref().map(|g| Gender::coerce(Some(g)));
    let spec = BreedingSpec {
        target_pal: data.target_pal.clone(),
        required_passives: data.required_passives.clone(),
        target_gender,
        max_generations: data.max_generations,
        max_results: data.max_results,
    };

    let chains = solve(db, &composite, &spec);
    #[cfg(not(target_arch = "wasm32"))]
    let elapsed_ms = serde_json::Value::from(start.elapsed().as_millis() as u64);
    #[cfg(target_arch = "wasm32")]
    let elapsed_ms = serde_json::Value::Null;

    ctx.emitter.emit(
        mt,
        &json!({
            "chains": chains,
            "total": chains.len(),
            "elapsed_ms": elapsed_ms,
            "warnings": warnings,
        }),
    );
    Ok(())
}
