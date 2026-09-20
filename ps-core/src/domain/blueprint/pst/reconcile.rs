//! Makes a leniently-converted blueprint safe to place.
//!
//! Dropping an unreadable piece is cheap; leaving a structure that dereferences the
//! dropped piece is not, because that failure surfaces as a corrupt save at placement
//! time rather than here. Runs once, after all dropping is finished.

use std::collections::HashSet;

use uuid::Uuid;

use super::super::{capture, validate::{Finding, Severity}, BaseBlueprint};
use crate::props;
use crate::ue::{FGuid, PalStruct, Property, PropertyKey, StructValue};

/// At most this many examples are named in an aggregated finding.
const MAX_EXEMPLARS: usize = 5;

fn aggregated(code: &str, severity: Severity, count: usize, exemplars: &[String]) -> Finding {
    let shown = exemplars.iter().take(MAX_EXEMPLARS).cloned().collect::<Vec<_>>().join(", ");
    Finding {
        severity,
        code: code.to_string(),
        message: if exemplars.len() > MAX_EXEMPLARS {
            format!("{count} affected, including {shown}")
        } else {
            format!("{count} affected: {shown}")
        },
    }
}

pub fn reconcile(blueprint: &mut BaseBlueprint, findings: &mut Vec<Finding>) {
    drop_structures_missing_containers(blueprint, findings);
    clear_slots_missing_dynamic_items(blueprint, findings);
    remove_dangling_connector_links(blueprint, findings);

    if blueprint.structures.is_empty() {
        findings.push(Finding {
            severity: Severity::Blocking,
            code: "pst.no_structures".to_string(),
            message: "no structures survived import".to_string(),
        });
    }
}

/// A structure's `ItemContainer`/`CharacterContainer` modules name a `target_container_id`
/// that must resolve to a real container entry -- one dropped during assembly leaves the
/// structure pointing at nothing, which is unsafe to place.
fn drop_structures_missing_containers(blueprint: &mut BaseBlueprint, findings: &mut Vec<Finding>) {
    let item_ids: HashSet<Uuid> =
        blueprint.item_containers.iter().filter_map(capture::container_entry_id).collect();
    let character_ids: HashSet<Uuid> =
        blueprint.character_containers.iter().filter_map(capture::container_entry_id).collect();

    let mut dropped = Vec::new();
    blueprint.structures.retain(|structure| {
        let (item_targets, character_targets) = capture::module_target_container_ids(&structure.properties);
        let resolves = item_targets.iter().all(|id| item_ids.contains(id))
            && character_targets.iter().all(|id| character_ids.contains(id));
        if !resolves {
            dropped.push(structure.map_object_id.clone());
        }
        resolves
    });

    if !dropped.is_empty() {
        findings.push(aggregated(
            "pst.structure_dropped_missing_container",
            Severity::Warning,
            dropped.len(),
            &dropped,
        ));
    }
}

/// An item container slot's occupant names a `dynamic_id` that must resolve to a
/// surviving `DynamicItemSaveData` entry -- one dropped during assembly leaves the slot
/// pointing at nothing, so the slot is emptied rather than the whole container dropped.
fn clear_slots_missing_dynamic_items(blueprint: &mut BaseBlueprint, findings: &mut Vec<Finding>) {
    let surviving: HashSet<Uuid> =
        blueprint.dynamic_items.iter().filter_map(capture::dynamic_item_local_id).collect();

    let mut cleared_ids = Vec::new();
    for entry in &mut blueprint.item_containers {
        let dangling: HashSet<Uuid> = capture::container_slot_dynamic_item_ids(entry)
            .into_iter()
            .filter(|id| !surviving.contains(id))
            .collect();
        if dangling.is_empty() {
            continue;
        }

        let container_id = capture::container_entry_id(entry);
        let Some(value_props) = props::struct_props_mut(&mut entry.value) else { continue };
        let Some(slots) = props::get_mut(value_props, &["Slots"]).and_then(props::struct_values_mut)
        else {
            continue;
        };
        for slot in slots {
            let StructValue::Struct(slot_props) = slot else { continue };
            let Some(Property::Struct(StructValue::Game(PalStruct::ItemContainerSlots(raw)))) =
                slot_props.0.get_mut(&PropertyKey::from("RawData"))
            else {
                continue;
            };
            let id = props::guid_to_uuid(&raw.item.dynamic_id.local_id_in_created_world);
            if !dangling.contains(&id) {
                continue;
            }
            raw.count = 0;
            raw.item.static_id.clear();
            raw.item.dynamic_id.created_world_id = FGuid::nil();
            raw.item.dynamic_id.local_id_in_created_world = FGuid::nil();
            if let Some(container_id) = container_id {
                cleared_ids.push(container_id.to_string());
            }
        }
    }

    if !cleared_ids.is_empty() {
        findings.push(aggregated(
            "pst.slot_cleared_missing_dynamic_item",
            Severity::Warning,
            cleared_ids.len(),
            &cleared_ids,
        ));
    }
}

/// A structure's `Connector.RawData.connect.any_place` names other structures by their
/// `Model.RawData.instance_id`; one dropped during assembly leaves a link pointing at
/// nothing, so the link is removed rather than the owning structure. PST's own importer
/// runs an equivalent pass (`_remap_connector_links` in `base_manager.py`).
fn remove_dangling_connector_links(blueprint: &mut BaseBlueprint, findings: &mut Vec<Finding>) {
    let surviving: HashSet<Uuid> = capture::structure_instance_ids(blueprint).into_iter().collect();

    let mut removed = 0usize;
    let mut exemplars = Vec::new();
    for structure in &mut blueprint.structures {
        let dropped = if let Some(connector) = capture::map_object_connector_mut(&mut structure.properties)
        {
            let before = connector.connect.any_place.len();
            connector
                .connect
                .any_place
                .retain(|item| surviving.contains(&props::guid_to_uuid(&item.connect_to_model_instance_id)));
            before - connector.connect.any_place.len()
        } else {
            0
        };

        if dropped > 0 {
            removed += dropped;
            if exemplars.len() < MAX_EXEMPLARS {
                exemplars.push(structure.map_object_id.clone());
            }
        }
    }

    if removed > 0 {
        findings.push(aggregated(
            "pst.connector_link_removed",
            Severity::Warning,
            removed,
            &exemplars,
        ));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ue::Properties;

    fn fixture(name: &str) -> Vec<u8> {
        let dir =
            std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../tests/fixtures/pst");
        let gz_path = dir.join(format!("{name}.gz"));
        if gz_path.exists() {
            let compressed = std::fs::read(&gz_path)
                .unwrap_or_else(|e| panic!("read fixture {}: {e}", gz_path.display()));
            let mut decoded = Vec::new();
            std::io::Read::read_to_end(
                &mut flate2::read::GzDecoder::new(compressed.as_slice()),
                &mut decoded,
            )
            .unwrap_or_else(|e| panic!("gunzip fixture {}: {e}", gz_path.display()));
            decoded
        } else {
            std::fs::read(dir.join(name)).unwrap_or_else(|e| panic!("read fixture {name}: {e}"))
        }
    }

    fn model_instance_id(properties: &Properties) -> Option<Uuid> {
        let model = properties.0.get(&PropertyKey::from("Model")).and_then(props::struct_props)?;
        match model.0.get(&PropertyKey::from("RawData"))? {
            Property::Struct(StructValue::Game(PalStruct::MapModel(raw))) => {
                Some(props::guid_to_uuid(&raw.instance_id))
            }
            _ => None,
        }
    }

    /// A link removed by `remove_dangling_connector_links` must actually be gone from
    /// the owning structure's `any_place` list, not merely reported -- placement would
    /// otherwise dereference a neighbor that this same pass just deleted.
    #[test]
    fn a_connector_link_to_a_removed_structure_is_dropped_with_a_warning() {
        let mut blueprint = super::super::import(&fixture("v1_relics_base.json"), "Home")
            .expect("imports")
            .blueprint;

        let mut owner_and_target = None;
        for structure in &mut blueprint.structures {
            if let Some(connector) = capture::map_object_connector_mut(&mut structure.properties) {
                if let Some(item) = connector.connect.any_place.first() {
                    owner_and_target = Some((
                        structure.map_object_id.clone(),
                        props::guid_to_uuid(&item.connect_to_model_instance_id),
                    ));
                    break;
                }
            }
        }
        let (owner_map_object_id, target_id) =
            owner_and_target.expect("fixture has at least one connector link");

        let target_index = blueprint
            .structures
            .iter()
            .position(|s| model_instance_id(&s.properties) == Some(target_id))
            .expect("the linked structure exists in the fixture");
        blueprint.structures.remove(target_index);

        let mut findings = Vec::new();
        reconcile(&mut blueprint, &mut findings);

        findings
            .iter()
            .find(|f| f.code == "pst.connector_link_removed")
            .expect("the removal is reported");

        let owner = blueprint
            .structures
            .iter_mut()
            .find(|s| s.map_object_id == owner_map_object_id)
            .expect("the owning structure survives; only its neighbor was removed");
        let connector = capture::map_object_connector_mut(&mut owner.properties)
            .expect("the owner still carries a connector");
        assert!(
            connector
                .connect
                .any_place
                .iter()
                .all(|item| props::guid_to_uuid(&item.connect_to_model_instance_id) != target_id),
            "a link to a removed structure must not survive reconciliation"
        );
    }
}
