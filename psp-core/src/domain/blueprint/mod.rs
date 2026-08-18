//! Capture a base out of a save into a portable blueprint, and place one back.

pub mod capture;
pub mod gvas;
pub mod place;
pub mod remap;
pub mod scrub;
pub mod transform;
pub mod validate;

use serde::{Deserialize, Serialize};

use crate::error::CoreError;
use crate::ue::games::palworld::PalTransform;
use crate::ue::{Header, MapEntry, Properties, StructValue};

pub const SCHEMA_VERSION: u32 = 1;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct CaptureOptions {
    pub production_config: bool,
    pub structure_condition: bool,
    pub container_contents: bool,
    pub worker_pals: bool,
    pub housed_pals: bool,
    pub production_progress: bool,
    pub access_config: bool,
    pub base_identity: bool,
}

impl CaptureOptions {
    pub fn blueprint() -> Self {
        CaptureOptions {
            production_config: true,
            ..CaptureOptions::default()
        }
    }

    pub fn configured() -> Self {
        CaptureOptions {
            production_config: true,
            structure_condition: true,
            access_config: true,
            base_identity: true,
            ..CaptureOptions::default()
        }
    }

    pub fn full() -> Self {
        CaptureOptions {
            production_config: true,
            structure_condition: true,
            container_contents: true,
            worker_pals: true,
            housed_pals: true,
            production_progress: true,
            access_config: true,
            base_identity: true,
        }
    }
}

pub type CaptureManifest = CaptureOptions;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BlueprintHeader {
    pub schema_version: u32,
    pub game_data_version: String,
    pub uesave_struct_version: String,
    pub manifest: CaptureManifest,
    pub name: String,
    pub source_world: String,
    pub source_base: String,
    pub created_at: i64,
    pub structure_count: u32,
    pub footprint_radius: f64,
    pub anchor_height_above_terrain: f64,
}

/// No serde derives: `Properties` is only deserializable inside a schema
/// context, which Task 6 supplies by encoding the blueprint as a `Save`.
#[derive(Debug, Clone)]
pub struct BlueprintStructure {
    pub map_object_id: String,
    pub relative_transform: PalTransform,
    pub properties: Properties,
}

#[derive(Debug, Clone)]
pub struct BaseBlueprint {
    pub header: BlueprintHeader,
    /// The source save's GVAS header, carried verbatim. NOT decoration:
    /// `large_world_coordinates()` (engine major >= 5) decides whether every
    /// `Vector` is f64 or f32, and `property_tag()` (>= 5.4) whether an extra
    /// byte is consumed per property. A defaulted header decodes every
    /// coordinate at the wrong precision, silently.
    pub source_header: Header,
    pub base_camp: Option<Properties>,
    pub structures: Vec<BlueprintStructure>,
    pub item_containers: Vec<MapEntry>,
    pub character_containers: Vec<MapEntry>,
    pub characters: Vec<MapEntry>,
    pub works: Vec<StructValue>,
    pub dynamic_items: Vec<StructValue>,
}

impl BaseBlueprint {
    pub fn check_schema_version(&self) -> Result<(), CoreError> {
        if self.header.schema_version > SCHEMA_VERSION {
            return Err(CoreError::Parse(format!(
                "blueprint schema version {} is newer than supported version {SCHEMA_VERSION}",
                self.header.schema_version
            )));
        }
        Ok(())
    }

    pub fn check_manifest_consistency(&self) -> Result<(), CoreError> {
        if self.header.structure_count == 0 {
            return Ok(());
        }
        let manifest = &self.header.manifest;
        if manifest.container_contents && self.item_containers.is_empty() {
            return Err(CoreError::Parse(
                "manifest claims container contents but none are present".to_string(),
            ));
        }
        if manifest.worker_pals && self.character_containers.is_empty() {
            return Err(CoreError::Parse(
                "manifest claims worker pals but no character container is present".to_string(),
            ));
        }
        Ok(())
    }
}
