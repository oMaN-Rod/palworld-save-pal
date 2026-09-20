//! Turns a decoded PST payload (see [`super::envelope`]) into a native `BaseBlueprint`.
//!
//! PST's JSON dialect is walked through uesave's `compat::gvas_json`, the same reader
//! that understands uesave's own GVAS-as-JSON form. Two `RawData` shapes come back with
//! an empty type tag because the property that would have named them is a sibling the
//! dialect reader cannot see from inside the payload it is decoding; this module is the
//! caller that re-attaches both, from the sibling it *can* see at this level.

use serde_json::Value;

use super::reconcile;
use super::super::validate::{Finding, Severity};
use super::super::{
    scrub, transform, BaseBlueprint, BlueprintHeader, BlueprintStructure, CaptureOptions,
    SCHEMA_VERSION,
};
use crate::error::CoreError;
use crate::props;
use crate::ue::games::palworld::{PalMapModel, PalTransform};
use crate::ue::{
    CustomFormatData, Header, MapEntry, PackageVersion, PalStruct, Palworld, Properties,
    Property, PropertyKey, StructValue,
};
use uesave::compat::gvas_json::properties_from_json;

#[derive(Debug)]
pub struct PstImport {
    pub blueprint: BaseBlueprint,
    pub findings: Vec<Finding>,
}

pub fn import(bytes: &[u8], name: &str) -> Result<PstImport, CoreError> {
    let payload = super::envelope::decode(bytes)?;
    check_not_outdated(&payload)?;

    let mut findings = Vec::new();

    let (base_camp_properties, anchor, footprint_radius) = read_base_camp(&payload)?;
    let structures = read_structures(&payload, &anchor, &mut findings);

    let item_containers = read_map_entries(
        &payload,
        "item_containers",
        "worldSaveData.ItemContainerSaveData",
        "pst.item_container_dropped",
        &mut findings,
    );
    let character_containers = read_map_entries(
        &payload,
        "char_containers",
        "worldSaveData.CharacterContainerSaveData",
        "pst.character_container_dropped",
        &mut findings,
    );
    let characters = read_map_entries(
        &payload,
        "characters",
        "worldSaveData.CharacterSaveParameterMap",
        "pst.character_dropped",
        &mut findings,
    );
    let works = read_works(&payload, &mut findings);
    let dynamic_items = read_struct_values(
        &payload,
        "dynamic_items",
        "worldSaveData.DynamicItemSaveData",
        "pst.dynamic_item_dropped",
        &mut findings,
    );

    findings.push(Finding {
        severity: Severity::Warning,
        code: "pst.base_camp_level_dropped".to_string(),
        message: "PalStudio's blueprint has nowhere to store the guild's base level; it was \
                  dropped."
            .to_string(),
    });

    let mut blueprint = BaseBlueprint {
        header: BlueprintHeader {
            schema_version: SCHEMA_VERSION,
            game_data_version: String::new(),
            uesave_struct_version: env!("CARGO_PKG_VERSION").to_string(),
            manifest: CaptureOptions::full(),
            name: name.to_string(),
            source_world: String::new(),
            source_base: String::new(),
            created_at: 0,
            structure_count: structures.len() as u32,
            footprint_radius,
            anchor_height_above_terrain: 0.0,
        },
        source_header: palworld_header(),
        base_camp: Some(base_camp_properties),
        structures,
        item_containers,
        character_containers,
        characters,
        works,
        dynamic_items,
    };

    reconcile::reconcile(&mut blueprint, &mut findings);
    scrub::scrub_blueprint(&mut blueprint);

    Ok(PstImport { blueprint, findings })
}

/// Mirrors PST's own `is_old_blueprint`: an export missing either field predates the
/// shape this importer understands.
fn check_not_outdated(payload: &Value) -> Result<(), CoreError> {
    if payload.get("dynamic_items").is_none() || payload.get("base_camp_level").is_none() {
        return Err(CoreError::Parse(
            "this base export is missing dynamic_items or base_camp_level and predates the \
             current PST export format; re-export it from PalworldSaveTools and try again"
                .to_string(),
        ));
    }
    Ok(())
}

/// The anchor transform every structure is made relative to, and the base's footprint
/// radius. A missing or mis-shaped base camp leaves nothing to anchor structures to, so
/// it is refused rather than imported without one.
fn read_base_camp(payload: &Value) -> Result<(Properties, PalTransform, f64), CoreError> {
    let value = payload
        .get("base_camp")
        .and_then(|base_camp| base_camp.get("value"))
        .ok_or_else(|| CoreError::Parse("payload is missing base_camp.value".to_string()))?;
    let properties = properties_from_json::<Palworld>(value, "worldSaveData.BaseCampSaveData")
        .map_err(|e| CoreError::Parse(format!("base_camp did not decode: {e}")))?;

    let Some(Property::Struct(StructValue::Game(PalStruct::BaseCamp(raw)))) =
        properties.0.get(&PropertyKey::from("RawData"))
    else {
        return Err(CoreError::Parse(
            "base_camp.RawData is not a typed BaseCamp".to_string(),
        ));
    };
    let anchor = raw.transform.clone();
    let footprint_radius = raw.area_range as f64;

    Ok((properties, anchor, footprint_radius))
}

fn read_structures(
    payload: &Value,
    anchor: &PalTransform,
    findings: &mut Vec<Finding>,
) -> Vec<BlueprintStructure> {
    let mut structures = Vec::new();
    let Some(map_objects) = payload.get("map_objects").and_then(Value::as_array) else {
        return structures;
    };
    for (index, value) in map_objects.iter().enumerate() {
        match read_structure(value, anchor) {
            Ok(structure) => structures.push(structure),
            Err(e) => findings.push(Finding {
                severity: Severity::Warning,
                code: "pst.structure_dropped".to_string(),
                message: format!("map_objects[{index}] did not decode and was dropped: {e}"),
            }),
        }
    }
    structures
}

fn read_structure(value: &Value, anchor: &PalTransform) -> Result<BlueprintStructure, CoreError> {
    let mut properties = properties_from_json::<Palworld>(value, "worldSaveData.MapObjectSaveData")
        .map_err(|e| CoreError::Parse(e.to_string()))?;

    let map_object_id = properties
        .0
        .get(&PropertyKey::from("MapObjectId"))
        .and_then(props::as_str)
        .ok_or_else(|| CoreError::Parse("map object has no MapObjectId".to_string()))?
        .to_string();

    let relative_transform = {
        let model = structure_model(&properties)
            .ok_or_else(|| CoreError::Parse("map object has no typed Model.RawData".to_string()))?;
        transform::to_relative(anchor, &model.initial_transform_cache)
    };

    // The sibling naming a concrete-model module's type is the ModuleMap entry's own
    // key; the dialect reader never sees it while decoding the entry's RawData.
    reattach_module_types(&mut properties);

    Ok(BlueprintStructure { map_object_id, relative_transform, properties })
}

fn structure_model(properties: &Properties) -> Option<&PalMapModel> {
    let model = properties.0.get(&PropertyKey::from("Model")).and_then(props::struct_props)?;
    match model.0.get(&PropertyKey::from("RawData"))? {
        Property::Struct(StructValue::Game(PalStruct::MapModel(model))) => Some(model),
        _ => None,
    }
}

fn reattach_module_types(properties: &mut Properties) {
    let Some(concrete) = properties
        .0
        .get_mut(&PropertyKey::from("ConcreteModel"))
        .and_then(props::struct_props_mut)
    else {
        return;
    };
    let Some(entries) =
        concrete.0.get_mut(&PropertyKey::from("ModuleMap")).and_then(props::map_entries_mut)
    else {
        return;
    };
    for entry in entries {
        let Some(module_type) = props::as_enum(&entry.key).map(str::to_string) else { continue };
        let Some(value_props) = props::struct_props_mut(&mut entry.value) else { continue };
        if let Some(Property::Struct(StructValue::Game(PalStruct::MapConcreteModelModule(raw)))) =
            value_props.0.get_mut(&PropertyKey::from("RawData"))
        {
            raw.module_type = module_type;
        }
    }
}

fn read_map_entries(
    payload: &Value,
    section: &str,
    path: &str,
    code: &str,
    findings: &mut Vec<Finding>,
) -> Vec<MapEntry> {
    let mut entries = Vec::new();
    let Some(items) = payload.get(section).and_then(Value::as_array) else { return entries };
    for (index, item) in items.iter().enumerate() {
        match read_map_entry(item, path) {
            Ok(entry) => entries.push(entry),
            Err(e) => findings.push(Finding {
                severity: Severity::Warning,
                code: code.to_string(),
                message: format!("{section}[{index}] did not decode and was dropped: {e}"),
            }),
        }
    }
    entries
}

fn read_map_entry(item: &Value, path: &str) -> Result<MapEntry, CoreError> {
    let key = item.get("key").ok_or_else(|| CoreError::Parse("entry has no key".to_string()))?;
    let value =
        item.get("value").ok_or_else(|| CoreError::Parse("entry has no value".to_string()))?;
    let key = properties_from_json::<Palworld>(key, path).map_err(|e| CoreError::Parse(e.to_string()))?;
    let value =
        properties_from_json::<Palworld>(value, path).map_err(|e| CoreError::Parse(e.to_string()))?;
    Ok(MapEntry {
        key: Property::Struct(StructValue::Struct(key)),
        value: Property::Struct(StructValue::Struct(value)),
    })
}

fn read_works(payload: &Value, findings: &mut Vec<Finding>) -> Vec<StructValue> {
    let mut works = Vec::new();
    let Some(items) = payload.get("works").and_then(Value::as_array) else { return works };
    for (index, item) in items.iter().enumerate() {
        match read_work(item) {
            Ok(value) => works.push(value),
            Err(e) => findings.push(Finding {
                severity: Severity::Warning,
                code: "pst.work_dropped".to_string(),
                message: format!("works[{index}] did not decode and was dropped: {e}"),
            }),
        }
    }
    works
}

fn read_work(value: &Value) -> Result<StructValue, CoreError> {
    let mut properties = properties_from_json::<Palworld>(value, "worldSaveData.WorkSaveData")
        .map_err(|e| CoreError::Parse(e.to_string()))?;

    // `WorkableType` is a sibling of `RawData`, not inside it, so the dialect reader
    // left `PalWork::work_type` empty; it is right here, so re-attach it.
    let workable_type =
        properties.0.get(&PropertyKey::from("WorkableType")).and_then(props::as_enum).map(str::to_string);
    if let Some(workable_type) = workable_type {
        if let Some(Property::Struct(StructValue::Game(PalStruct::Work(raw)))) =
            properties.0.get_mut(&PropertyKey::from("RawData"))
        {
            raw.work_type = workable_type;
        }
    }

    Ok(StructValue::Struct(properties))
}

fn read_struct_values(
    payload: &Value,
    section: &str,
    path: &str,
    code: &str,
    findings: &mut Vec<Finding>,
) -> Vec<StructValue> {
    let mut values = Vec::new();
    let Some(items) = payload.get(section).and_then(Value::as_array) else { return values };
    for (index, item) in items.iter().enumerate() {
        match properties_from_json::<Palworld>(item, path) {
            Ok(properties) => values.push(StructValue::Struct(properties)),
            Err(e) => findings.push(Finding {
                severity: Severity::Warning,
                code: code.to_string(),
                message: format!("{section}[{index}] did not decode and was dropped: {e}"),
            }),
        }
    }
    values
}

/// A synthesized canonical Palworld header (UE 5.1). PST ships no GVAS header of its
/// own. This governs only how an imported blueprint re-encodes to its own `.psbp` file
/// format; placement always writes through the *target* session's real header, so this
/// synthesized one never reaches a save.
fn palworld_header() -> Header {
    Header {
        magic: u32::from_le_bytes(*b"GVAS"),
        save_game_version: 2,
        package_version: PackageVersion { ue4: 522, ue5: Some(1008) },
        engine_version_major: 5,
        engine_version_minor: 1,
        engine_version_patch: 0,
        engine_version_build: 0,
        engine_version: String::from("5.1.0"),
        custom_version: Some((3, Vec::<CustomFormatData>::new())),
    }
}
