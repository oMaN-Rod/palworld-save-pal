//! Placement validation: given a blueprint and a proposed anchor, decide whether placing
//! it is allowed and why not. Placement mutates nothing until `has_blocking` is false.

use std::collections::HashSet;

use uuid::Uuid;

use super::{transform, BaseBlueprint};
use crate::domain::world;
use crate::domain::world_option;
use crate::gamedata::GameData;
use crate::props;
use crate::session::SaveSession;
use crate::ue::games::palworld::PalTransform;
use crate::ue::{Double, PalStruct, Property, PropertyKey, StructValue, Vector};

/// Anchor within this many cm of an existing base camp triggers `base_too_close`.
/// Inherited from PalworldSaveTools with no cited source -- see `check_base_too_close`.
const BASE_TOO_CLOSE_CM: f64 = 5000.0;

/// A placed structure within this many cm of an existing structure of another base triggers `structure_overlap`.
const STRUCTURE_OVERLAP_CM: f64 = 100.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Severity {
    Blocking,
    Warning,
}

#[derive(Debug, Clone)]
pub struct Finding {
    pub severity: Severity,
    pub code: String,
    pub message: String,
}

#[derive(Debug, Clone, Copy)]
pub enum PlacementMode {
    NewBase { guild_id: Uuid },
    MergeInto { base_id: Uuid },
}

#[derive(Debug, Clone, Copy)]
pub struct Anchor {
    pub x: f64,
    pub y: f64,
    pub z: f64,
    pub yaw_radians: f64,
}

impl Anchor {
    fn to_transform(self) -> PalTransform {
        PalTransform {
            rotation: transform::yaw_quat(self.yaw_radians),
            translation: Vector { x: Double(self.x), y: Double(self.y), z: Double(self.z) },
            scale: Vector { x: Double(1.0), y: Double(1.0), z: Double(1.0) },
        }
    }
}

pub fn has_blocking(findings: &[Finding]) -> bool {
    findings.iter().any(|finding| finding.severity == Severity::Blocking)
}

pub fn check(
    session: &SaveSession,
    game_data: &GameData,
    blueprint: &BaseBlueprint,
    anchor: &Anchor,
    mode: &PlacementMode,
) -> Vec<Finding> {
    let mut findings = Vec::new();
    let anchor_transform = anchor.to_transform();
    let existing_bases = existing_bases(session);
    let world_structures = world_structures(session);
    let blueprint_structure_count = blueprint.structures.len() as i64;
    let mut unreadable_limits: Vec<&str> = Vec::new();

    let (area_range, total_structure_count) = match mode {
        PlacementMode::NewBase { guild_id } => {
            if !check_guild_base_limit(session, *guild_id, &existing_bases, &mut findings) {
                unreadable_limits.push("BaseCampMaxNumInGuild");
            }
            if !check_world_base_limit(session, existing_bases.len() as i64, &mut findings) {
                unreadable_limits.push("BaseCampMaxNum");
            }
            check_base_too_close(&anchor_transform, &existing_bases, &mut findings);
            (blueprint.header.footprint_radius, blueprint_structure_count)
        }
        PlacementMode::MergeInto { base_id } => {
            let target_area_range = existing_bases
                .iter()
                .find(|base| base.id == *base_id)
                .map(|base| base.area_range)
                .unwrap_or(blueprint.header.footprint_radius);
            let existing_structure_count =
                world_structures.iter().filter(|placed| placed.base_id == *base_id).count() as i64;
            (target_area_range, existing_structure_count + blueprint_structure_count)
        }
    };

    if !check_building_limit(session, total_structure_count, &mut findings) {
        unreadable_limits.push("MaxBuildingLimitNum");
    }
    check_limits_unknown(&unreadable_limits, &mut findings);
    check_outside_area_range(blueprint, area_range, &mut findings);
    check_unknown_structure_type(game_data, blueprint, &mut findings);
    check_structure_overlap(&world_structures, blueprint, &anchor_transform, mode, &mut findings);

    findings
}

struct ExistingBase {
    id: Uuid,
    guild_id: Uuid,
    transform: PalTransform,
    area_range: f64,
}

/// One placed structure, reduced to the two things validation asks about. Read in a single
/// pass so the overlap check never walks the property tree again: the walk costs two
/// `PropertyKey` allocations per object, once per blueprint structure per map object.
struct WorldStructure {
    base_id: Uuid,
    x: f64,
    y: f64,
    z: f64,
}

fn existing_bases(session: &SaveSession) -> Vec<ExistingBase> {
    let Ok(Some(entries)) = world::base_camp_map(&session.level) else {
        return Vec::new();
    };
    entries
        .iter()
        .filter_map(|entry| {
            let id = props::as_uuid(&entry.key)?;
            let value_props = props::struct_props(&entry.value)?;
            match value_props.0.get(&PropertyKey::from("RawData")) {
                Some(Property::Struct(StructValue::Game(PalStruct::BaseCamp(raw)))) => Some(ExistingBase {
                    id,
                    guild_id: props::guid_to_uuid(&raw.group_id_belong_to),
                    transform: raw.transform.clone(),
                    area_range: raw.area_range as f64,
                }),
                _ => None,
            }
        })
        .collect()
}

fn world_structures(session: &SaveSession) -> Vec<WorldStructure> {
    let Ok(Some(map_objects)) = world::map_object_values(&session.level) else {
        return Vec::new();
    };
    let model_key = PropertyKey::from("Model");
    let raw_data_key = PropertyKey::from("RawData");
    map_objects
        .iter()
        .filter_map(|value| {
            let StructValue::Struct(object_props) = value else { return None };
            let model = object_props.0.get(&model_key).and_then(props::struct_props)?;
            let Property::Struct(StructValue::Game(PalStruct::MapModel(raw))) =
                model.0.get(&raw_data_key)?
            else {
                return None;
            };
            let translation = &raw.initial_transform_cache.translation;
            Some(WorldStructure {
                base_id: props::guid_to_uuid(&raw.base_camp_id_belong_to),
                x: translation.x.0,
                y: translation.y.0,
                z: translation.z.0,
            })
        })
        .collect()
}

fn distance(a: &Vector, b: &Vector) -> f64 {
    let dx = a.x.0 - b.x.0;
    let dy = a.y.0 - b.y.0;
    let dz = a.z.0 - b.z.0;
    (dx * dx + dy * dy + dz * dz).sqrt()
}

fn length(v: &Vector) -> f64 {
    (v.x.0 * v.x.0 + v.y.0 * v.y.0 + v.z.0 * v.z.0).sqrt()
}

/// Each `check_*` limit function returns whether the setting it needs could be
/// read at all; `false` means the answer is unknown, not "within limits".
fn check_guild_base_limit(
    session: &SaveSession,
    guild_id: Uuid,
    existing_bases: &[ExistingBase],
    findings: &mut Vec<Finding>,
) -> bool {
    let Some(limit) = world_option_int(session, "BaseCampMaxNumInGuild") else { return false };
    let count = existing_bases.iter().filter(|base| base.guild_id == guild_id).count() as i64;
    if count >= limit {
        findings.push(Finding {
            severity: Severity::Blocking,
            code: "guild_base_limit".to_string(),
            message: format!(
                "guild already has {count} base(s), at or above the BaseCampMaxNumInGuild limit of {limit}"
            ),
        });
    }
    true
}

fn check_world_base_limit(session: &SaveSession, count: i64, findings: &mut Vec<Finding>) -> bool {
    let Some(limit) = world_option_int(session, "BaseCampMaxNum") else { return false };
    if count >= limit {
        findings.push(Finding {
            severity: Severity::Blocking,
            code: "world_base_limit".to_string(),
            message: format!(
                "world already has {count} base(s), at or above the BaseCampMaxNum limit of {limit}"
            ),
        });
    }
    true
}

fn check_building_limit(
    session: &SaveSession,
    structure_count: i64,
    findings: &mut Vec<Finding>,
) -> bool {
    let Some(limit) = world_option_int(session, "MaxBuildingLimitNum") else { return false };
    if limit != 0 && structure_count > limit {
        findings.push(Finding {
            severity: Severity::Blocking,
            code: "building_limit".to_string(),
            message: format!(
                "placement would leave {structure_count} structures, over the MaxBuildingLimitNum limit of {limit}"
            ),
        });
    }
    true
}

/// A save whose `WorldOption.sav` is missing or unparseable (the loader degrades to `None`)
/// leaves every limit unevaluated. Say so, or `has_blocking` silently reports "nothing
/// wrong" for a placement that was never checked.
fn check_limits_unknown(unreadable: &[&str], findings: &mut Vec<Finding>) {
    if unreadable.is_empty() {
        return;
    }
    findings.push(Finding {
        severity: Severity::Warning,
        code: "limits_unknown".to_string(),
        message: format!(
            "world limits could not be read ({}), so base and building limits were not checked",
            unreadable.join(", ")
        ),
    });
}

/// Warns rather than blocks: the 5000 cm figure has no cited source.
fn check_base_too_close(
    anchor_transform: &PalTransform,
    existing_bases: &[ExistingBase],
    findings: &mut Vec<Finding>,
) {
    for base in existing_bases {
        let separation = distance(&anchor_transform.translation, &base.transform.translation);
        if separation < BASE_TOO_CLOSE_CM {
            findings.push(Finding {
                severity: Severity::Warning,
                code: "base_too_close".to_string(),
                message: format!(
                    "anchor is {separation:.0} cm from existing base {base_id}, under the {BASE_TOO_CLOSE_CM:.0} cm separation",
                    base_id = base.id
                ),
            });
        }
    }
}

fn check_outside_area_range(blueprint: &BaseBlueprint, area_range: f64, findings: &mut Vec<Finding>) {
    for structure in &blueprint.structures {
        let translation = &structure.relative_transform.translation;
        let horizontal = (translation.x.0 * translation.x.0 + translation.y.0 * translation.y.0).sqrt();
        if horizontal > area_range {
            findings.push(Finding {
                severity: Severity::Warning,
                code: "outside_area_range".to_string(),
                message: format!(
                    "structure {} sits {horizontal:.0} cm from the base center, outside the {area_range:.0} cm area range",
                    structure.map_object_id
                ),
            });
        }
    }
}

/// Warning, not Blocking: the catalog is regenerated per game update and a base
/// legitimately holds objects that are not buildings (dropped items, destructible rocks),
/// so an unrecognized id means "we cannot vouch for this one", not "unplaceable".
fn check_unknown_structure_type(
    game_data: &GameData,
    blueprint: &BaseBlueprint,
    findings: &mut Vec<Finding>,
) {
    let Some(buildings) = game_data.get("buildings").and_then(serde_json::Value::as_object) else {
        return;
    };
    // Saves and catalog disagree on casing for the same object
    // (`Stone_Foundation` on disk vs `Stone_foundation` in the catalog), so an
    // exact-case lookup would flag hundreds of ordinary walls and floors.
    let known: HashSet<String> = buildings.keys().map(|key| key.to_lowercase()).collect();
    for structure in &blueprint.structures {
        if !known.contains(&structure.map_object_id.to_lowercase()) {
            findings.push(Finding {
                severity: Severity::Warning,
                code: "unknown_structure_type".to_string(),
                message: format!(
                    "structure {} is not a known building type in the bundled game data",
                    structure.map_object_id
                ),
            });
        }
    }
}

fn check_structure_overlap(
    world_structures: &[WorldStructure],
    blueprint: &BaseBlueprint,
    anchor_transform: &PalTransform,
    mode: &PlacementMode,
    findings: &mut Vec<Finding>,
) {
    // In `MergeInto` mode the target base's own structures are not "another base";
    // excluding them lets an added structure sit next to the existing ones without warning.
    let same_base_id = match mode {
        PlacementMode::MergeInto { base_id } => Some(*base_id),
        PlacementMode::NewBase { .. } => None,
    };

    // Rotation preserves length, so no structure can land farther from the
    // anchor than its own relative offset. Anything outside that reach plus the
    // overlap radius cannot collide with anything in the blueprint.
    let reach = blueprint
        .structures
        .iter()
        .map(|structure| length(&structure.relative_transform.translation))
        .fold(0.0, f64::max)
        + STRUCTURE_OVERLAP_CM;
    let anchor_translation = &anchor_transform.translation;
    let candidates: Vec<&WorldStructure> = world_structures
        .iter()
        .filter(|placed| Some(placed.base_id) != same_base_id)
        .filter(|placed| {
            (placed.x - anchor_translation.x.0).abs() <= reach
                && (placed.y - anchor_translation.y.0).abs() <= reach
                && (placed.z - anchor_translation.z.0).abs() <= reach
        })
        .collect();
    if candidates.is_empty() {
        return;
    }

    for structure in &blueprint.structures {
        let world_transform = transform::to_world(anchor_transform, &structure.relative_transform);
        let placed = &world_transform.translation;
        let overlaps = candidates.iter().any(|candidate| {
            let dx = placed.x.0 - candidate.x;
            let dy = placed.y.0 - candidate.y;
            let dz = placed.z.0 - candidate.z;
            dx * dx + dy * dy + dz * dz < STRUCTURE_OVERLAP_CM * STRUCTURE_OVERLAP_CM
        });
        if overlaps {
            findings.push(Finding {
                severity: Severity::Warning,
                code: "structure_overlap".to_string(),
                message: format!(
                    "structure {} lands within {STRUCTURE_OVERLAP_CM:.0} cm of a structure belonging to another base",
                    structure.map_object_id
                ),
            });
        }
    }
}

fn world_option_int(session: &SaveSession, key: &str) -> Option<i64> {
    let save = session.world_option.as_ref()?;
    world_option::read_settings(save).into_iter().find(|entry| entry.key == key)?.value.as_i64()
}
