//! Signal — a live Palworld world feed.
//!
//! Signal polls a data source (a dedicated server's REST API, the local game's
//! `-output-gamedata` bridge file, or a synthetic fixture), normalizes the rows
//! into a stable actor model, and republishes them on a local, token-guarded
//! HTTP API (`/v1/hello`, `/v1/live`, `/v1/server`, `/v1/minimap`).
pub mod api;
pub mod discovery;
pub mod manager;
pub mod model;
pub mod normalize;
pub mod poller;
pub mod rest;
pub mod store;
pub mod token;

/// What the `/v1/hello` endpoint reports as its product name.
pub const PRODUCT_NAME: &str = "PalStudio Signal";

/// Protocol compatibility level of the wire format. Feed clients treat
/// this as an opaque build number.
pub const WIRE_BUILD: i64 = 1;

/// Palworld's Steam appid, used for the Proton prefix path during
/// game-data discovery.
pub const PALWORLD_APPID: u64 = 1623730;
