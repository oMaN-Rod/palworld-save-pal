mod common;

use psp_core::domain::blueprint::place::{self, PlacementRequest};
use psp_core::domain::blueprint::validate::{Anchor, PlacementMode};
use psp_core::domain::blueprint::{capture, gvas, remap, BaseBlueprint, CaptureOptions};
use psp_core::domain::world;
use psp_core::palbin;
use psp_core::props;
use psp_core::session::SaveSession;
use psp_core::ue::games::palworld::{PalConnector, PalMapConcreteModelModuleData};
use psp_core::ue::{MapEntry, PalStruct, Properties, Property, PropertyKey, StructValue};
use std::collections::{BTreeSet, HashSet};
use uuid::Uuid;

fn fixture_blueprint() -> BaseBlueprint {
    let session = common::load_fixture_session("v1_relics");
    let base_id = common::fixture_base_id(&session);
    capture::capture(&session, base_id, CaptureOptions::full(), "Home").expect("capture")
}

fn set_of(ids: impl IntoIterator<Item = Uuid>) -> HashSet<Uuid> {
    ids.into_iter().filter(|id| !id.is_nil()).collect()
}

/// Every id the blueprint DEFINES, grouped so a test can assert each group is
/// non-empty (never vacuously disjoint) and fully regenerated.
fn definition_sets(blueprint: &BaseBlueprint) -> Vec<(&'static str, HashSet<Uuid>)> {
    vec![
        (
            "structure models",
            set_of(capture::structure_instance_ids(blueprint)),
        ),
        (
            "structure concrete models",
            set_of(capture::structure_concrete_instance_ids(blueprint)),
        ),
        (
            "works",
            set_of(blueprint.works.iter().filter_map(capture::work_base_id)),
        ),
        (
            "work assigns",
            set_of(blueprint.works.iter().flat_map(work_assign_ids)),
        ),
        (
            "item containers",
            set_of(
                blueprint
                    .item_containers
                    .iter()
                    .filter_map(capture::container_entry_id),
            ),
        ),
        (
            "character containers",
            set_of(
                blueprint
                    .character_containers
                    .iter()
                    .filter_map(capture::container_entry_id),
            ),
        ),
        (
            "characters",
            set_of(
                blueprint
                    .characters
                    .iter()
                    .filter_map(world::entry_instance_id),
            ),
        ),
        (
            "dynamic items",
            set_of(
                blueprint
                    .dynamic_items
                    .iter()
                    .filter_map(capture::dynamic_item_local_id),
            ),
        ),
    ]
}

fn connector(properties: &Properties) -> Option<&PalConnector> {
    let model = properties
        .0
        .get(&PropertyKey::from("Model"))
        .and_then(props::struct_props)?;
    let connector = model
        .0
        .get(&PropertyKey::from("Connector"))
        .and_then(props::struct_props)?;
    match connector.0.get(&PropertyKey::from("RawData"))? {
        Property::Struct(StructValue::Game(PalStruct::Connector(raw))) => Some(raw),
        _ => None,
    }
}

/// Every 16-byte window in every connector's opaque tail that equals one of
/// `ids`, in the raw Palworld guid byte encoding.
fn connector_tail_hits(blueprint: &BaseBlueprint, ids: &HashSet<Uuid>) -> usize {
    blueprint
        .structures
        .iter()
        .filter_map(|s| connector(&s.properties))
        .flat_map(|c| c.unknown_bytes.windows(16))
        .filter(|window| {
            let raw: [u8; 16] = (*window).try_into().expect("16-byte window");
            ids.contains(&palbin::guid_bytes_to_uuid(raw))
        })
        .count()
}

fn connector_any_place_targets(blueprint: &BaseBlueprint) -> Vec<Uuid> {
    blueprint
        .structures
        .iter()
        .filter_map(|s| connector(&s.properties))
        .flat_map(|c| &c.connect.any_place)
        .map(|item| props::guid_to_uuid(&item.connect_to_model_instance_id))
        .collect()
}

fn work_assign_ids(work: &StructValue) -> Vec<Uuid> {
    work_assigns(work).into_iter().map(|(id, _)| id).collect()
}

fn work_assign_individual_ids(work: &StructValue) -> Vec<Uuid> {
    work_assigns(work)
        .into_iter()
        .map(|(_, individual)| individual)
        .collect()
}

fn work_assigns(work: &StructValue) -> Vec<(Uuid, Uuid)> {
    let mut out = Vec::new();
    let StructValue::Struct(work_props) = work else {
        return out;
    };
    let Some(Property::Map(entries)) = work_props.0.get(&PropertyKey::from("WorkAssignMap")) else {
        return out;
    };
    for entry in entries {
        let Some(assign_props) = props::struct_props(&entry.value) else {
            continue;
        };
        if let Some(Property::Struct(StructValue::Game(PalStruct::WorkAssign(raw)))) =
            assign_props.0.get(&PropertyKey::from("RawData"))
        {
            out.push((
                props::guid_to_uuid(&raw.id),
                props::guid_to_uuid(&raw.assigned_individual_id.instance_id),
            ));
        }
    }
    out
}

/// `(owner_map_object_model_id, owner_map_object_concrete_model_id,
/// transform.map_object_instance_id)` for every captured work.
fn work_owner_ids(work: &StructValue) -> Vec<(&'static str, Uuid)> {
    let mut out = Vec::new();
    let StructValue::Struct(work_props) = work else {
        return out;
    };
    let Some(Property::Struct(StructValue::Game(PalStruct::Work(raw)))) =
        work_props.0.get(&PropertyKey::from("RawData"))
    else {
        return out;
    };
    if let Some(base) = &raw.base_data {
        out.push((
            "owner_map_object_model_id",
            props::guid_to_uuid(&base.owner_map_object_model_id),
        ));
        out.push((
            "owner_map_object_concrete_model_id",
            props::guid_to_uuid(&base.owner_map_object_concrete_model_id),
        ));
    }
    if let Some(id) = raw
        .transform
        .as_ref()
        .and_then(|t| t.map_object_instance_id.as_ref())
    {
        out.push(("transform.map_object_instance_id", props::guid_to_uuid(id)));
    }
    out
}

fn module_targets(properties: &Properties) -> Vec<(&'static str, Uuid)> {
    let mut out = Vec::new();
    let Some(concrete) = properties
        .0
        .get(&PropertyKey::from("ConcreteModel"))
        .and_then(props::struct_props)
    else {
        return out;
    };
    let Some(entries) = concrete
        .0
        .get(&PropertyKey::from("ModuleMap"))
        .and_then(props::map_entries)
    else {
        return out;
    };
    for module in entries {
        let Some(module_props) = props::struct_props(&module.value) else {
            continue;
        };
        let Some(Property::Struct(StructValue::Game(PalStruct::MapConcreteModelModule(raw)))) =
            module_props.0.get(&PropertyKey::from("RawData"))
        else {
            continue;
        };
        match &raw.data {
            PalMapConcreteModelModuleData::ItemContainer {
                target_container_id,
                ..
            } => {
                out.push(("item containers", props::guid_to_uuid(target_container_id)));
            }
            PalMapConcreteModelModuleData::CharacterContainer {
                target_container_id,
                ..
            } => {
                out.push((
                    "character containers",
                    props::guid_to_uuid(target_container_id),
                ));
            }
            PalMapConcreteModelModuleData::Workee { target_work_id, .. } => {
                out.push(("works", props::guid_to_uuid(target_work_id)));
            }
            _ => {}
        }
    }
    out
}

fn character_container_slot_ids(entry: &MapEntry) -> Vec<Uuid> {
    container_slot_ids(entry, |slot_props| {
        match slot_props.0.get(&PropertyKey::from("RawData")) {
            Some(Property::Struct(StructValue::Game(PalStruct::CharacterContainer(raw)))) => {
                Some(props::guid_to_uuid(&raw.instance_id))
            }
            _ => None,
        }
    })
}

fn item_container_slot_ids(entry: &MapEntry) -> Vec<Uuid> {
    container_slot_ids(entry, |slot_props| {
        match slot_props.0.get(&PropertyKey::from("RawData")) {
            Some(Property::Struct(StructValue::Game(PalStruct::ItemContainerSlots(raw)))) => Some(
                props::guid_to_uuid(&raw.item.dynamic_id.local_id_in_created_world),
            ),
            _ => None,
        }
    })
}

fn container_slot_ids(
    entry: &MapEntry,
    mut field: impl FnMut(&Properties) -> Option<Uuid>,
) -> Vec<Uuid> {
    let mut out = Vec::new();
    let Some(value_props) = props::struct_props(&entry.value) else {
        return out;
    };
    let Some(slots) = props::get(value_props, &["Slots"]).and_then(props::struct_values) else {
        return out;
    };
    for slot in slots {
        let StructValue::Struct(slot_props) = slot else {
            continue;
        };
        if let Some(id) = field(slot_props) {
            out.push(id);
        }
    }
    out
}

/// A captured pal's `SaveParameter.SlotId.ContainerId.ID` back-pointer.
fn pal_container_id(entry: &MapEntry) -> Option<Uuid> {
    let save_parameter = world::entry_save_parameter(entry)?;
    props::get(save_parameter, &["SlotID", "ContainerId", "ID"])
        .or_else(|| props::get(save_parameter, &["SlotId", "ContainerId", "ID"]))
        .and_then(props::as_uuid)
}

fn base_camp_owner_id(blueprint: &BaseBlueprint) -> Option<Uuid> {
    match blueprint
        .base_camp
        .as_ref()?
        .0
        .get(&PropertyKey::from("RawData"))?
    {
        Property::Struct(StructValue::Game(PalStruct::BaseCamp(raw))) => {
            Some(props::guid_to_uuid(&raw.owner_map_object_instance_id))
        }
        _ => None,
    }
}

fn work_collection(blueprint: &BaseBlueprint) -> palbin::WorkCollection {
    let base_camp = blueprint
        .base_camp
        .as_ref()
        .expect("the fixture base has a BaseCampSaveData");
    let bytes = props::get(base_camp, &["WorkCollection", "RawData"])
        .and_then(props::as_byte_array)
        .expect("the fixture base camp carries a WorkCollection blob");
    palbin::read_work_collection(bytes).expect("WorkCollection decodes")
}

#[test]
fn remapping_gives_every_structure_a_fresh_instance_id() {
    let mut blueprint = fixture_blueprint();

    let before: HashSet<Uuid> = capture::structure_instance_ids(&blueprint)
        .into_iter()
        .collect();
    remap::remap_blueprint(&mut blueprint).expect("remap");
    let after: HashSet<Uuid> = capture::structure_instance_ids(&blueprint)
        .into_iter()
        .collect();

    assert!(
        !before.is_empty(),
        "the fixture base must have structures for this to mean anything"
    );
    assert_eq!(before.len(), after.len(), "no structure may be lost");
    assert!(
        before.is_disjoint(&after),
        "every instance id must be regenerated, unlike PST which reuses them"
    );
}

#[test]
fn remapping_twice_yields_two_disjoint_id_sets() {
    let blueprint = fixture_blueprint();

    let mut first = blueprint.clone();
    let mut second = blueprint.clone();
    remap::remap_blueprint(&mut first).expect("remap");
    remap::remap_blueprint(&mut second).expect("remap");

    let a: HashSet<Uuid> = capture::structure_instance_ids(&first)
        .into_iter()
        .collect();
    let b: HashSet<Uuid> = capture::structure_instance_ids(&second)
        .into_iter()
        .collect();
    assert!(
        !a.is_empty(),
        "the fixture base must have structures for this to mean anything"
    );
    assert!(
        a.is_disjoint(&b),
        "the same blueprint placed twice must not collide with itself"
    );
}

#[test]
fn remapping_keeps_the_model_to_concrete_reference_consistent() {
    let mut blueprint = fixture_blueprint();
    remap::remap_blueprint(&mut blueprint).expect("remap");

    for structure in &blueprint.structures {
        assert!(
            capture::model_concrete_reference_resolves(structure),
            "Model.concrete_model_instance_id must still point at ConcreteModel.instance_id"
        );
    }
    // Most structures have a nil concrete model, so the loop above passes on
    // them whatever the remap does; only the linked ones test anything.
    let linked = set_of(capture::structure_concrete_instance_ids(&blueprint)).len();
    assert!(
        linked > 0,
        "no structure carries a ConcreteModel, so this asserts nothing"
    );
}

#[test]
fn every_definition_set_is_non_empty_and_fully_regenerated() {
    let mut blueprint = fixture_blueprint();
    let before = definition_sets(&blueprint);
    remap::remap_blueprint(&mut blueprint).expect("remap");
    let after = definition_sets(&blueprint);

    for ((name, before), (_, after)) in before.into_iter().zip(after) {
        assert!(
            !before.is_empty(),
            "{name}: the fixture must define ids here, or this is vacuous"
        );
        assert_eq!(
            before.len(),
            after.len(),
            "{name}: no definition may be lost or merged"
        );
        assert!(
            before.is_disjoint(&after),
            "{name}: every defined id must be regenerated"
        );
    }
}

#[test]
fn every_reference_resolves_into_the_post_remap_definition_set() {
    let mut blueprint = fixture_blueprint();
    remap::remap_blueprint(&mut blueprint).expect("remap");

    let sets = definition_sets(&blueprint);
    let structures = lookup(&sets, "structure models");
    let concrete = lookup(&sets, "structure concrete models");
    let works = lookup(&sets, "works");
    let item_containers = lookup(&sets, "item containers");
    let character_containers = lookup(&sets, "character containers");
    let characters = lookup(&sets, "characters");
    let dynamic_items = lookup(&sets, "dynamic items");

    // Only a non-nil reference tests anything: nil means "outside the
    // blueprint", which is always allowed.
    let mut checked = 0;
    let mut check = |label: &str, id: Uuid, targets: &HashSet<Uuid>| {
        if id.is_nil() {
            return;
        }
        assert!(
            targets.contains(&id),
            "{label}: {id} resolves to nothing in the blueprint"
        );
        checked += 1;
    };

    for structure in &blueprint.structures {
        for (kind, id) in module_targets(&structure.properties) {
            let targets = match kind {
                "item containers" => item_containers,
                "character containers" => character_containers,
                _ => works,
            };
            check(&format!("module target ({kind})"), id, targets);
        }
    }
    for id in connector_any_place_targets(&blueprint) {
        check("connector any_place", id, structures);
    }
    for entry in &blueprint.character_containers {
        for id in character_container_slot_ids(entry) {
            check("character container slot", id, characters);
        }
    }
    for entry in &blueprint.item_containers {
        for id in item_container_slot_ids(entry) {
            check("item container slot", id, dynamic_items);
        }
    }
    for entry in &blueprint.characters {
        if let Some(id) = pal_container_id(entry) {
            check("pal SlotId.ContainerId.ID", id, character_containers);
        }
    }
    for work in &blueprint.works {
        for (field, id) in work_owner_ids(work) {
            let targets = if field == "owner_map_object_concrete_model_id" {
                concrete
            } else {
                structures
            };
            check(field, id, targets);
        }
        for id in work_assign_individual_ids(work) {
            check("work assign assigned_individual_id", id, characters);
        }
    }
    if let Some(id) = base_camp_owner_id(&blueprint) {
        check("base camp owner_map_object_instance_id", id, structures);
    }

    assert!(
        checked > 400,
        "expected the fixture's whole live reference surface, resolved only {checked}"
    );
}

fn lookup<'a>(sets: &'a [(&'static str, HashSet<Uuid>)], name: &str) -> &'a HashSet<Uuid> {
    &sets
        .iter()
        .find(|(key, _)| *key == name)
        .expect("known definition set")
        .1
}

#[test]
fn character_container_slots_still_name_their_pals() {
    let mut blueprint = fixture_blueprint();
    remap::remap_blueprint(&mut blueprint).expect("remap");

    let characters = set_of(
        blueprint
            .characters
            .iter()
            .filter_map(world::entry_instance_id),
    );
    let slots = set_of(
        blueprint
            .character_containers
            .iter()
            .flat_map(character_container_slot_ids),
    );

    assert!(
        !characters.is_empty(),
        "the fixture base must have housed or worker pals"
    );
    assert_eq!(
        slots.len(),
        characters.len(),
        "every occupied slot must still name exactly one captured pal"
    );
    assert!(
        slots.is_subset(&characters),
        "a slot must not point at a pal outside the blueprint"
    );
}

#[test]
fn work_assignments_still_name_their_workers() {
    let mut blueprint = fixture_blueprint();
    let assigned_before = set_of(blueprint.works.iter().flat_map(work_assign_individual_ids));
    remap::remap_blueprint(&mut blueprint).expect("remap");

    let characters = set_of(
        blueprint
            .characters
            .iter()
            .filter_map(world::entry_instance_id),
    );
    let assigned_after = set_of(blueprint.works.iter().flat_map(work_assign_individual_ids));

    assert!(
        !assigned_before.is_empty(),
        "the fixture must have assigned workers"
    );
    assert_eq!(
        assigned_after.len(),
        assigned_before.len(),
        "no assignment may be dropped to nil"
    );
    assert!(
        assigned_after.is_subset(&characters),
        "an assignment must name a captured pal"
    );
}

#[test]
fn no_connector_tail_survives_holding_a_pre_remap_id() {
    let mut blueprint = fixture_blueprint();
    let before = set_of(capture::structure_instance_ids(&blueprint));
    let before_hits = connector_tail_hits(&blueprint, &before);

    remap::remap_blueprint(&mut blueprint).expect("remap");

    let after = set_of(capture::structure_instance_ids(&blueprint));
    assert!(
        before_hits > 0,
        "the fixture's connector tails must hold structure ids or this asserts nothing"
    );
    assert_eq!(
        connector_tail_hits(&blueprint, &before),
        0,
        "no connector tail may still name a pre-remap structure -- \
         a placed copy would cross-link to the source base"
    );
    assert_eq!(
        connector_tail_hits(&blueprint, &after),
        before_hits,
        "every tail reference must have been rewritten, not erased"
    );
}

#[test]
fn the_work_collection_lists_exactly_the_remapped_works() {
    let mut blueprint = fixture_blueprint();
    let before = work_collection(&blueprint);
    let before_works: Vec<Uuid> = blueprint
        .works
        .iter()
        .filter_map(capture::work_base_id)
        .collect();
    remap::remap_blueprint(&mut blueprint).expect("remap");
    let after = work_collection(&blueprint);
    let after_works: Vec<Uuid> = blueprint
        .works
        .iter()
        .filter_map(capture::work_base_id)
        .collect();

    assert!(!before_works.is_empty(), "the fixture base must have works");
    assert!(
        before.work_ids.len() > before_works.len(),
        "the fixture's WorkCollection must carry ids the capture drops, or the rebuild is untested"
    );
    assert_eq!(
        after.work_ids, after_works,
        "the collection must list the post-remap works, in order"
    );
    assert_ne!(
        after.own_id, before.own_id,
        "the collection's own id is a definition too"
    );
    assert!(
        set_of(after.work_ids.clone()).is_disjoint(&set_of(before.work_ids)),
        "no pre-remap work id may survive in the collection"
    );
}

/// The `WorkCollection` rebuild shortens an `ArrayProperty<Byte>`, and the
/// connector substitution rewrites bytes inside another. Both have to survive
/// the encoders that turn a blueprint into a `.psp` file.
#[test]
fn a_remapped_blueprint_still_round_trips_through_both_encodings() {
    let mut blueprint = fixture_blueprint();
    remap::remap_blueprint(&mut blueprint).expect("remap");
    let expected = work_collection(&blueprint);

    let from_psp = gvas::from_psp_bytes(&gvas::to_psp_bytes(&blueprint).expect("psp encode"))
        .expect("psp decode");
    let from_json =
        gvas::from_json(&gvas::to_json(&blueprint).expect("json encode")).expect("json decode");

    assert_eq!(from_psp.structures.len(), blueprint.structures.len());
    assert_eq!(
        work_collection(&from_json),
        expected,
        "the rebuilt WorkCollection must survive json"
    );
    assert_eq!(
        connector_tail_hits(
            &from_json,
            &set_of(capture::structure_instance_ids(&from_json))
        ),
        connector_tail_hits(
            &blueprint,
            &set_of(capture::structure_instance_ids(&blueprint))
        ),
        "the substituted connector tails must survive json"
    );
}

/// Adds a byte to one of the base camp's opaque blobs. `WorkerDirector` is a
/// fixed 118-byte layout and `WorkCollection` is checked to its last byte, so
/// one byte too many is enough to make either refuse to decode -- which is what
/// a Palworld update that moved a field would look like from here.
fn lengthen_base_camp_blob(base_camp: &mut Properties, field: &str) {
    let raw_data = props::get_mut(base_camp, &[field, "RawData"])
        .unwrap_or_else(|| panic!("the fixture base camp must carry a {field} blob"));
    let bytes = props::as_byte_array_mut(raw_data)
        .unwrap_or_else(|| panic!("{field} RawData must be a byte array"));
    bytes.push(0);
}

/// Both of `remap`'s opaque-blob rewrites used to shrug a decode failure off and
/// carry the blob over untouched: the placed base's workers would go on naming
/// the SOURCE save's container, and the base would go on calling its works by
/// the source save's ids, with nothing reported. The blobs are read by offset,
/// so one Palworld update is all it would take.
#[test]
fn a_base_camp_blob_that_does_not_decode_refuses_the_remap() {
    let blueprint = fixture_blueprint();

    // The control: the same blueprint, blobs intact, remaps cleanly -- so a
    // refusal below is the corruption and not the blueprint.
    assert!(
        remap::remap_blueprint(&mut blueprint.clone()).is_ok(),
        "setup: an intact blueprint must remap, or the refusals below prove nothing"
    );

    for field in ["WorkerDirector", "WorkCollection"] {
        let mut corrupt = blueprint.clone();
        lengthen_base_camp_blob(
            corrupt
                .base_camp
                .as_mut()
                .expect("the fixture base has a base camp"),
            field,
        );

        let error = remap::remap_blueprint(&mut corrupt).expect_err(&format!(
            "{field}: a blob that does not decode must refuse the remap"
        ));
        assert!(
            error.to_string().contains(field),
            "{field}: the refusal must name the blob that failed: {error}"
        );
    }
}

#[test]
fn a_nil_id_stays_nil() {
    let mut remap = remap::IdRemap::default();
    assert!(
        remap.new_for(Uuid::nil()).is_nil(),
        "a zero guid must not be given an identity"
    );
}

// ---- end-to-end identity invariants ----

/// How many structures the `v1_relics` fixture's richest base carries.
const FIXTURE_BASE_STRUCTURES: usize = 543;

/// Every capture layer. A UID leak that only the default preset avoids is still
/// a leak, so nothing here is allowed to test `full` alone.
fn capture_layers() -> [(&'static str, CaptureOptions); 3] {
    [
        ("blueprint", CaptureOptions::blueprint()),
        ("configured", CaptureOptions::configured()),
        ("full", CaptureOptions::full()),
    ]
}

fn new_base_request(anchor: Anchor, guild_id: Uuid, owner: Uuid) -> PlacementRequest {
    PlacementRequest {
        anchor,
        mode: PlacementMode::NewBase { guild_id },
        owner_player_uid: owner,
        override_warnings: true,
    }
}

fn anchor_far_from_everything() -> Anchor {
    Anchor {
        x: 400_000.0,
        y: 400_000.0,
        z: 1000.0,
        yaw_radians: 0.0,
    }
}

/// Every byte encoding one UID can appear in inside a blueprint's payload: the
/// on-disk `FGuid` layout (four little-endian `u32`s, which shuffles the uuid's
/// byte order), the unshuffled uuid bytes, and the hyphenated ASCII form.
/// Scanning for the uuid's own byte order alone would miss every guid the game
/// actually writes.
fn uid_needles(uid: Uuid) -> Vec<(&'static str, Vec<u8>)> {
    vec![
        ("on-disk guid bytes", palbin::guid_bytes(uid).to_vec()),
        ("uuid bytes", uid.as_bytes().to_vec()),
        ("ascii", uid.to_string().into_bytes()),
    ]
}

fn contains(haystack: &[u8], needle: &[u8]) -> bool {
    haystack
        .windows(needle.len())
        .any(|window| window == needle)
}

/// `uid_needles` is only defence in depth if its three forms are three
/// different byte strings that `contains` can each actually find. Only the
/// on-disk form ever matches a payload the game wrote, so a form that had
/// silently become a copy of another -- or one `contains` could never match --
/// would sit in the leak scans looking like coverage they do not provide.
#[test]
fn every_uid_needle_form_is_a_distinct_findable_needle() {
    let uid = Uuid::from_u128(0x36d1_1392_45df_a3d2_8f2a_448c_0b55_0e18);
    let forms = uid_needles(uid);
    assert_eq!(
        forms.len(),
        3,
        "every encoding a uid can appear in must be scanned for"
    );

    for (form, needle) in &forms {
        assert!(
            !needle.is_empty(),
            "{form}: an empty needle cannot be searched for"
        );
        let mut haystack = vec![0xABu8; 7];
        haystack.extend_from_slice(needle);
        haystack.extend_from_slice(&[0xCDu8; 7]);
        assert!(
            contains(&haystack, needle),
            "{form}: a planted needle must be found"
        );
        for (other, other_needle) in &forms {
            if other == form {
                continue;
            }
            assert!(
                !contains(&haystack, other_needle),
                "{form}: {other} matches the same bytes, so the two forms are one needle"
            );
        }
    }
}

/// A `.psp` file's payload, decompressed. `to_psp_bytes` writes the body with
/// `write_plm`, so the file is Oodle-compressed and a byte scan over it as
/// written can only ever fail by coincidence -- it would pass with every UID in
/// the save still inside. Decompressing first is what makes the scan mean
/// anything.
fn psp_payload(blueprint: &BaseBlueprint) -> Vec<u8> {
    let file = gvas::to_psp_bytes(blueprint).expect("psp encode");
    let body = &file[gvas::PSP_MAGIC.len() + 4..];
    let payload = psp_core::ue::compression::decompress_save(&mut std::io::Cursor::new(body))
        .expect("a psp body must decompress");
    assert!(
        payload.len() > file.len(),
        "setup: the psp body must actually be compressed, {} raw vs {} in the file",
        payload.len(),
        file.len()
    );
    payload
}

#[test]
fn no_player_uid_from_the_source_save_survives_capture() {
    let session = common::load_fixture_session("v1_relics");
    let base_id = common::fixture_base_id(&session);
    let uids = common::all_player_uids(&session);

    assert!(
        uids.len() >= 10,
        "setup: the fixture must know its ten players, got {}",
        uids.len()
    );

    for (layer, options) in capture_layers() {
        // The control. Without it a clean scan would prove only that these
        // needles are absent from any blueprint whatsoever, whether or not the
        // scrub pass does anything.
        let leaky = capture::capture_unscrubbed(&session, base_id, options, "Home")
            .expect("unscrubbed capture");
        let leaky_json = gvas::to_json(&leaky).expect("serialize");
        assert!(
            uids.iter().any(|uid| leaky_json.contains(&uid.to_string())),
            "{layer}: an unscrubbed capture must carry a source uid, or this scan proves nothing"
        );

        let blueprint = capture::capture(&session, base_id, options, "Home").expect("capture");
        assert_eq!(
            blueprint.structures.len(),
            FIXTURE_BASE_STRUCTURES,
            "{layer}: setup: the capture must carry the whole base"
        );

        let json = gvas::to_json(&blueprint).expect("serialize");
        for uid in &uids {
            assert!(
                !json.contains(&uid.to_string()),
                "{layer}: player uid {uid} leaked into the blueprint json"
            );
        }
    }
}

/// The belt-and-braces check: a UID hiding in an unmodelled `unknown_bytes`
/// tail never reaches the JSON as a string, but it is still in the file.
#[test]
fn no_player_uid_survives_into_the_psp_bytes() {
    let session = common::load_fixture_session("v1_relics");
    let base_id = common::fixture_base_id(&session);
    let uids = common::all_player_uids(&session);

    assert!(
        uids.len() >= 10,
        "setup: the fixture must know its ten players, got {}",
        uids.len()
    );

    for (layer, options) in capture_layers() {
        // The control. Without it a clean scan would prove only that these
        // needles are absent from any blueprint whatsoever, whether or not the
        // scrub pass does anything.
        let leaky = capture::capture_unscrubbed(&session, base_id, options, "Home")
            .expect("unscrubbed capture");
        let leaky_payload = psp_payload(&leaky);
        let hit_forms: BTreeSet<&'static str> = uids
            .iter()
            .flat_map(|uid| uid_needles(*uid))
            .filter(|(_, needle)| contains(&leaky_payload, needle))
            .map(|(form, _)| form)
            .collect();
        // Asserted exactly, not `any`: the game writes every guid as an on-disk
        // `FGuid`, so that is the one form a real leak takes, and an `any` here
        // would go on passing on some other form if it stopped matching. The
        // other two forms stay in the scan for a leak the game would never
        // write -- a guid an encoder emitted in uuid order, or one spelled out
        // as a string -- and `every_uid_needle_form_is_a_distinct_findable_needle`
        // is what shows they are live needles rather than dead weight.
        assert_eq!(
            hit_forms.into_iter().collect::<Vec<_>>(),
            vec!["on-disk guid bytes"],
            "{layer}: an unscrubbed capture must leak source uids as on-disk guid bytes, \
             and only as those, or this scan proves nothing"
        );

        let blueprint = capture::capture(&session, base_id, options, "Home").expect("capture");
        assert_eq!(
            blueprint.structures.len(),
            FIXTURE_BASE_STRUCTURES,
            "{layer}: setup: the capture must carry the whole base"
        );

        let payload = psp_payload(&blueprint);
        for uid in &uids {
            for (form, needle) in uid_needles(*uid) {
                assert!(
                    !contains(&payload, &needle),
                    "{layer}: player uid {uid} leaked into the psp bytes as {form}"
                );
            }
        }
    }
}

/// Guild identity is not player identity, and `common::all_player_uids`
/// structurally cannot see it: a guild id is a `GroupSaveDataMap` key, never a
/// `PlayerUId`, so the two uid scans above would stay green with the source
/// save's guild stamped on every captured pal. It identifies the sharer just as
/// squarely, so it gets its own scan, at every layer and in both encodings.
#[test]
fn no_guild_id_from_the_source_save_survives_capture() {
    let session = common::load_fixture_session("v1_relics");
    let base_id = common::fixture_base_id(&session);
    let group_ids = common::all_group_ids(&session);
    let owning_guild = common::fixture_guild_id(&session);

    assert!(
        group_ids.len() >= 2,
        "setup: the fixture must know several groups, got {}",
        group_ids.len()
    );
    assert!(
        group_ids.contains(&owning_guild),
        "setup: the captured base's own guild must be among the ids scanned for"
    );

    for (layer, options) in capture_layers() {
        // The control, in both encodings. Without it a clean scan would prove
        // only that a guild id is absent from any blueprint whatsoever.
        let leaky = capture::capture_unscrubbed(&session, base_id, options, "Home")
            .expect("unscrubbed capture");
        assert!(
            gvas::to_json(&leaky)
                .expect("serialize")
                .contains(&owning_guild.to_string()),
            "{layer}: an unscrubbed capture must carry the source guild id, \
             or the json scan proves nothing"
        );
        assert!(
            contains(&psp_payload(&leaky), &palbin::guid_bytes(owning_guild)),
            "{layer}: an unscrubbed capture must carry the source guild id, \
             or the byte scan proves nothing"
        );

        let blueprint = capture::capture(&session, base_id, options, "Home").expect("capture");
        assert_eq!(
            blueprint.structures.len(),
            FIXTURE_BASE_STRUCTURES,
            "{layer}: setup: the capture must carry the whole base"
        );

        let json = gvas::to_json(&blueprint).expect("serialize");
        let payload = psp_payload(&blueprint);
        for id in &group_ids {
            assert!(
                !json.contains(&id.to_string()),
                "{layer}: guild id {id} leaked into the blueprint json"
            );
            for (form, needle) in uid_needles(*id) {
                assert!(
                    !contains(&payload, &needle),
                    "{layer}: guild id {id} leaked into the psp bytes as {form}"
                );
            }
        }
    }
}

/// Where the guild id sat on a captured pal: the typed `PalCharacterData`
/// sibling of the `SaveParameter` bag, which the property-bag scrub never
/// touches. Pinned structurally as well as by byte scan, so a future capture
/// that carried pals from a save whose guild id happened to be absent from
/// `GroupSaveDataMap` could not quietly reintroduce it.
#[test]
fn no_captured_pal_still_names_its_guild() {
    let session = common::load_fixture_session("v1_relics");
    let base_id = common::fixture_base_id(&session);

    let leaky = capture::capture_unscrubbed(&session, base_id, CaptureOptions::full(), "Home")
        .expect("unscrubbed capture");
    let leaked = character_group_ids(&leaky);
    assert!(
        leaked.iter().any(|id| !id.is_nil()),
        "setup: an unscrubbed full capture must carry pals stamped with their guild, \
         got {leaked:?}"
    );

    let blueprint =
        capture::capture(&session, base_id, CaptureOptions::full(), "Home").expect("capture");
    let scrubbed = character_group_ids(&blueprint);
    assert_eq!(
        scrubbed.len(),
        leaked.len(),
        "scrubbing must not drop pals, only their identity"
    );
    assert!(
        scrubbed.iter().all(Uuid::is_nil),
        "a captured pal must not name the source save's guild, got {scrubbed:?}"
    );
}

/// Each captured character's typed `PalCharacterData.group_id`, one per
/// character, duplicates kept so a caller can assert the count too.
fn character_group_ids(blueprint: &BaseBlueprint) -> Vec<Uuid> {
    blueprint
        .characters
        .iter()
        .filter_map(world::entry_character_data)
        .map(|data| props::guid_to_uuid(&data.group_id))
        .collect()
}

// ---- the secret a lock module carries ----

const LOCK_MODULE_TYPE: &str = "EPalMapObjectConcreteModelModuleType::PasswordLock";

/// Distinctive enough that finding it in a payload cannot be coincidence.
const INJECTED_PASSWORD: &str = "PspBlueprintLockSecret";

/// A structure's `Model.RawData.base_camp_id_belong_to`, the field capture
/// selects a base's structures by.
fn structure_base_id(object_props: &Properties) -> Option<Uuid> {
    let model = object_props
        .0
        .get(&PropertyKey::from("Model"))
        .and_then(props::struct_props)?;
    match model.0.get(&PropertyKey::from("RawData"))? {
        Property::Struct(StructValue::Game(PalStruct::MapModel(raw))) => {
            Some(props::guid_to_uuid(&raw.base_camp_id_belong_to))
        }
        _ => None,
    }
}

/// Gives `count` of the base's structures a real `PasswordLock` module. No
/// fixture ships one -- no chest or door in them was ever locked -- so without
/// this every assertion about what a lock leaks passes over an empty set.
///
/// The module is appended to a `ModuleMap` the structure already carries, so it
/// travels through the same property and the same write schema the game's own
/// modules do.
fn inject_password_locks(
    session: &mut SaveSession,
    base_id: Uuid,
    victim: Uuid,
    count: usize,
) -> usize {
    use psp_core::ue::games::palworld::{PalMapConcreteModelModule, PalPlayerLockInfo};

    let mut injected = 0;
    let values = world::map_object_values_mut(&mut session.level)
        .expect("map objects")
        .expect("MapObjectSaveData");
    for value in values {
        if injected == count {
            break;
        }
        let StructValue::Struct(object_props) = value else {
            continue;
        };
        if structure_base_id(object_props) != Some(base_id) {
            continue;
        }
        let Some(concrete) = object_props
            .0
            .get_mut(&PropertyKey::from("ConcreteModel"))
            .and_then(props::struct_props_mut)
        else {
            continue;
        };
        let Some(entries) = concrete
            .0
            .get_mut(&PropertyKey::from("ModuleMap"))
            .and_then(props::map_entries_mut)
        else {
            continue;
        };
        if entries.is_empty() {
            continue;
        }

        let module = PalMapConcreteModelModule {
            module_type: LOCK_MODULE_TYPE.to_string(),
            data: PalMapConcreteModelModuleData::PasswordLock {
                lock_state: 1,
                password: INJECTED_PASSWORD.to_string(),
                player_infos: vec![PalPlayerLockInfo {
                    player_uid: props::uuid_to_guid(victim),
                    try_failed_count: 0,
                    try_success_cache: 0,
                }],
                trailing_bytes: [0; 4],
            },
            custom_version_data: Vec::new(),
        };
        let mut module_props = Properties::default();
        module_props.insert(
            "RawData",
            Property::Struct(StructValue::Game(PalStruct::MapConcreteModelModule(module))),
        );
        entries.push(MapEntry {
            key: Property::Enum(LOCK_MODULE_TYPE.to_string()),
            value: Property::Struct(StructValue::Struct(module_props)),
        });
        injected += 1;
    }
    injected
}

/// One structure's `PasswordLock` modules as `(password, player uids)`, walked
/// through the public property surface rather than through capture's own
/// traversal helper. The fixture already ships lock modules of its own, all
/// with an empty password, so nothing here may assume a lock is an injected one.
fn structure_locks(properties: &Properties) -> Vec<(String, Vec<Uuid>)> {
    let mut out = Vec::new();
    let Some(concrete) = properties
        .0
        .get(&PropertyKey::from("ConcreteModel"))
        .and_then(props::struct_props)
    else {
        return out;
    };
    let Some(entries) = concrete
        .0
        .get(&PropertyKey::from("ModuleMap"))
        .and_then(props::map_entries)
    else {
        return out;
    };
    for module in entries {
        let Some(module_props) = props::struct_props(&module.value) else {
            continue;
        };
        let Some(Property::Struct(StructValue::Game(PalStruct::MapConcreteModelModule(raw)))) =
            module_props.0.get(&PropertyKey::from("RawData"))
        else {
            continue;
        };
        if let PalMapConcreteModelModuleData::PasswordLock {
            password,
            player_infos,
            ..
        } = &raw.data
        {
            out.push((
                password.clone(),
                player_infos
                    .iter()
                    .map(|info| props::guid_to_uuid(&info.player_uid))
                    .collect(),
            ));
        }
    }
    out
}

fn captured_locks(blueprint: &BaseBlueprint) -> Vec<(String, Vec<Uuid>)> {
    blueprint
        .structures
        .iter()
        .flat_map(|s| structure_locks(&s.properties))
        .collect()
}

/// How many captured locks still carry the injected secret.
fn injected_passwords(blueprint: &BaseBlueprint) -> usize {
    captured_locks(blueprint)
        .iter()
        .filter(|(password, _)| password == INJECTED_PASSWORD)
        .count()
}

fn lock_player_uids(blueprint: &BaseBlueprint) -> Vec<Uuid> {
    captured_locks(blueprint)
        .into_iter()
        .flat_map(|(_, uids)| uids)
        .collect()
}

/// How many of the base's structures hold the injected secret in the SAVE --
/// the control that owes nothing to capture, so a scan of what capture produced
/// cannot be clean merely because the injection never landed.
fn source_injected_passwords(session: &SaveSession, base_id: Uuid) -> usize {
    let Ok(Some(values)) = world::map_object_values(&session.level) else {
        return 0;
    };
    values
        .iter()
        .filter_map(|value| match value {
            StructValue::Struct(object_props) => Some(object_props),
            _ => None,
        })
        .filter(|object_props| structure_base_id(object_props) == Some(base_id))
        .flat_map(structure_locks)
        .filter(|(password, _)| password == INJECTED_PASSWORD)
        .count()
}

/// A `PasswordLock`'s `password` is the secret that opens the chest or door it
/// sits on, and a blueprint is a file its author hands to strangers.
/// `configured` and `full` both keep the lock module -- `access_config` is on
/// for both -- so at those two layers nothing but the scrub decides whether the
/// secret travels. Only `full`, which is understood to be a complete snapshot,
/// keeps it.
///
/// Both the typed field and the bytes the file is actually made of are checked:
/// a password is a plain string, so the byte scan is what would catch it
/// surviving somewhere the structural read does not look.
#[test]
fn a_lock_password_leaves_the_save_only_in_a_full_capture() {
    const LOCKS: usize = 3;

    for (layer, options) in capture_layers() {
        let mut session = common::load_fixture_session("v1_relics");
        let base_id = common::fixture_base_id(&session);
        let victim = *common::all_player_uids(&session)
            .first()
            .expect("the fixture must know its players");

        assert_eq!(
            inject_password_locks(&mut session, base_id, victim, LOCKS),
            LOCKS,
            "{layer}: setup: the base must take {LOCKS} password locks"
        );
        assert_eq!(
            source_injected_passwords(&session, base_id),
            LOCKS,
            "{layer}: setup: the save under capture must hold the secret, or a clean \
             blueprint proves only that the injection never landed"
        );

        // The unscrubbed control, which can only speak for a layer that keeps
        // the lock at all: `blueprint` drops the whole thing in
        // `clear_access_config`, before the scrub is reached. The source-side
        // count above is what stands in for it there.
        let leaky = capture::capture_unscrubbed(&session, base_id, options, "Home")
            .expect("unscrubbed capture");
        let control = if options.access_config { LOCKS } else { 0 };
        assert_eq!(
            injected_passwords(&leaky),
            control,
            "{layer}: an unscrubbed capture must carry the password exactly when the layer \
             keeps the lock, or this proves nothing"
        );
        assert_eq!(
            lock_player_uids(&leaky)
                .iter()
                .filter(|uid| **uid == victim)
                .count(),
            control,
            "{layer}: an unscrubbed capture must carry the locked-out player exactly when \
             the layer keeps the lock"
        );
        assert_eq!(
            contains(&psp_payload(&leaky), INJECTED_PASSWORD.as_bytes()),
            options.access_config,
            "{layer}: the unscrubbed capture's bytes must agree with its typed fields"
        );

        let blueprint = capture::capture(&session, base_id, options, "Home").expect("capture");
        let keeps_password = options == CaptureOptions::full();

        assert_eq!(
            injected_passwords(&blueprint),
            if keeps_password { LOCKS } else { 0 },
            "{layer}: only a full capture may keep a lock's password"
        );
        assert_eq!(
            contains(&psp_payload(&blueprint), INJECTED_PASSWORD.as_bytes()),
            keeps_password,
            "{layer}: the blueprint's own bytes must agree with its typed fields"
        );

        let uids = lock_player_uids(&blueprint);
        if options.access_config {
            assert_eq!(
                uids.len(),
                lock_player_uids(&leaky).len(),
                "{layer}: a layer that keeps the lock must zero its player list, not drop it"
            );
            assert!(
                uids.len() >= LOCKS,
                "{layer}: setup: the injected locks must be in this list"
            );
        }
        assert!(
            uids.iter().all(Uuid::is_nil),
            "{layer}: no locked-out player's uid may survive capture, got {uids:?}"
        );
    }
}

/// Every id class a placement appends to the destination, read straight off the
/// session so a test can compare the save's own census before and after.
fn destination_id_sets(session: &SaveSession) -> Vec<(&'static str, Vec<Uuid>)> {
    vec![
        ("map objects", common::all_map_object_instance_ids(session)),
        (
            "concrete models",
            destination_concrete_instance_ids(session),
        ),
        (
            "works",
            world::work_values(&session.level)
                .ok()
                .flatten()
                .map(|values| values.iter().filter_map(capture::work_base_id).collect())
                .unwrap_or_default(),
        ),
        (
            "item containers",
            world::item_container_map(&session.level)
                .map(|entries| {
                    entries
                        .iter()
                        .filter_map(capture::container_entry_id)
                        .collect()
                })
                .unwrap_or_default(),
        ),
        (
            "character containers",
            world::character_container_map(&session.level)
                .map(|entries| {
                    entries
                        .iter()
                        .filter_map(capture::container_entry_id)
                        .collect()
                })
                .unwrap_or_default(),
        ),
        (
            "characters",
            world::character_map(&session.level)
                .map(|entries| {
                    entries
                        .iter()
                        .filter_map(world::entry_instance_id)
                        .collect()
                })
                .unwrap_or_default(),
        ),
        (
            "dynamic items",
            world::dynamic_item_values(&session.level)
                .map(|values| {
                    values
                        .iter()
                        .filter_map(capture::dynamic_item_local_id)
                        .collect()
                })
                .unwrap_or_default(),
        ),
    ]
}

fn destination_concrete_instance_ids(session: &SaveSession) -> Vec<Uuid> {
    let Ok(Some(values)) = world::map_object_values(&session.level) else {
        return Vec::new();
    };
    values
        .iter()
        .filter_map(|value| {
            let StructValue::Struct(object_props) = value else {
                return None;
            };
            let concrete = object_props
                .0
                .get(&PropertyKey::from("ConcreteModel"))
                .and_then(props::struct_props)?;
            match concrete.0.get(&PropertyKey::from("RawData")) {
                Some(Property::Struct(StructValue::Game(PalStruct::MapConcreteModel(raw)))) => {
                    Some(props::guid_to_uuid(&raw.instance_id))
                }
                _ => None,
            }
        })
        .collect()
}

/// What the blueprint itself says it will add to each id class, counted off the
/// blueprint rather than off the placement's own report.
fn blueprint_definition_counts(blueprint: &BaseBlueprint) -> Vec<(&'static str, usize)> {
    vec![
        (
            "map objects",
            capture::structure_instance_ids(blueprint).len(),
        ),
        (
            "concrete models",
            capture::structure_concrete_instance_ids(blueprint).len(),
        ),
        (
            "works",
            blueprint
                .works
                .iter()
                .filter_map(capture::work_base_id)
                .count(),
        ),
        (
            "item containers",
            blueprint
                .item_containers
                .iter()
                .filter_map(capture::container_entry_id)
                .count(),
        ),
        (
            "character containers",
            blueprint
                .character_containers
                .iter()
                .filter_map(capture::container_entry_id)
                .count(),
        ),
        (
            "characters",
            blueprint
                .characters
                .iter()
                .filter_map(world::entry_instance_id)
                .count(),
        ),
        (
            "dynamic items",
            blueprint
                .dynamic_items
                .iter()
                .filter_map(capture::dynamic_item_local_id)
                .count(),
        ),
    ]
}

#[test]
fn placement_introduces_no_guid_that_already_existed() {
    let mut session = common::load_fixture_session("v1_relics");
    let base_id = common::fixture_base_id(&session);
    let blueprint =
        capture::capture(&session, base_id, CaptureOptions::full(), "Home").expect("capture");
    let guild_id = common::fixture_guild_id(&session);
    let owner = common::fixture_player_uid(&session);
    common::set_world_option_int(&mut session, "BaseCampMaxNumInGuild", 99);

    let before: HashSet<Uuid> = common::all_map_object_instance_ids(&session)
        .into_iter()
        .collect();
    assert!(
        before.len() > FIXTURE_BASE_STRUCTURES,
        "setup: the source save must already hold the captured base's own ids, got {}",
        before.len()
    );
    let census_before = destination_id_sets(&session);
    let expected = blueprint_definition_counts(&blueprint);

    let result = place::place(
        &mut session,
        &blueprint,
        &new_base_request(anchor_far_from_everything(), guild_id, owner),
        &common::game_data(),
    )
    .expect("placement");
    let new_base_id = result.base_id.expect("new base id");

    let placed = psp_core::domain::guild::base_structures(&session, new_base_id);
    assert_eq!(
        placed.len(),
        FIXTURE_BASE_STRUCTURES,
        "the placed base must hold every structure the blueprint carried"
    );
    for structure in &placed {
        let id: Uuid = structure
            .instance_id
            .parse()
            .expect("instance id is a uuid");
        assert!(
            !before.contains(&id),
            "placement reused an existing instance id: {id}"
        );
    }

    // The same invariant across every OTHER id class the placement appends to:
    // a collision -- with the destination or within the placement itself --
    // shows up as a set that grew by less than the number of rows added.
    let census_after = destination_id_sets(&session);
    for ((name, before), ((_, after), (_, added))) in census_before
        .into_iter()
        .zip(census_after.into_iter().zip(expected))
    {
        assert!(
            added > 0,
            "{name}: the blueprint must define ids here, or this is vacuous"
        );
        let unique_before: HashSet<Uuid> = before.iter().copied().collect();
        let unique_after: HashSet<Uuid> = after.iter().copied().collect();
        assert_eq!(
            after.len(),
            before.len() + added,
            "{name}: the placement must add exactly what the blueprint defines"
        );
        assert_eq!(
            unique_after.len(),
            unique_before.len() + added,
            "{name}: every id the placement introduces must be one the destination did not hold"
        );
    }
}

/// Ids are only consistent if every reference lands somewhere. The check runs
/// against the WHOLE destination, not just the placed base: a reference that
/// resolves to nothing in the save is a dangling pointer whatever it once meant.
#[test]
fn every_reference_a_placed_base_makes_resolves_in_the_destination() {
    let mut session = common::load_fixture_session("v1_relics");
    let base_id = common::fixture_base_id(&session);
    let blueprint =
        capture::capture(&session, base_id, CaptureOptions::full(), "Home").expect("capture");
    let guild_id = common::fixture_guild_id(&session);
    let owner = common::fixture_player_uid(&session);
    common::set_world_option_int(&mut session, "BaseCampMaxNumInGuild", 99);

    let result = place::place(
        &mut session,
        &blueprint,
        &new_base_request(anchor_far_from_everything(), guild_id, owner),
        &common::game_data(),
    )
    .expect("placement");
    let new_base_id = result.base_id.expect("new base id");

    let sets: Vec<(&'static str, HashSet<Uuid>)> = destination_id_sets(&session)
        .into_iter()
        .map(|(name, ids)| (name, set_of(ids)))
        .collect();
    let structures = lookup(&sets, "map objects");
    let concrete = lookup(&sets, "concrete models");
    let works = lookup(&sets, "works");
    let item_containers = lookup(&sets, "item containers");
    let character_containers = lookup(&sets, "character containers");
    let characters = lookup(&sets, "characters");
    let dynamic_items = lookup(&sets, "dynamic items");

    // Unscrubbed, because the scrub pass is for what LEAVES the save: this
    // reads back what actually landed in it.
    let placed =
        capture::capture_unscrubbed(&session, new_base_id, CaptureOptions::full(), "Placed")
            .expect("recapture");
    assert_eq!(
        placed.structures.len(),
        FIXTURE_BASE_STRUCTURES,
        "setup: the placed base must read back whole, or its references are not being walked"
    );

    let mut checked = 0;
    let mut check = |label: &str, id: Uuid, targets: &HashSet<Uuid>| {
        if id.is_nil() {
            return;
        }
        assert!(
            targets.contains(&id),
            "{label}: {id} resolves to nothing in the destination"
        );
        checked += 1;
    };

    for structure in &placed.structures {
        for (kind, id) in module_targets(&structure.properties) {
            let targets = match kind {
                "item containers" => item_containers,
                "character containers" => character_containers,
                _ => works,
            };
            check(&format!("module target ({kind})"), id, targets);
        }
    }
    for id in connector_any_place_targets(&placed) {
        check("connector any_place", id, structures);
    }
    for entry in &placed.character_containers {
        for id in character_container_slot_ids(entry) {
            check("character container slot", id, characters);
        }
    }
    for entry in &placed.item_containers {
        for id in item_container_slot_ids(entry) {
            check("item container slot", id, dynamic_items);
        }
    }
    for entry in &placed.characters {
        if let Some(id) = pal_container_id(entry) {
            check("pal SlotId.ContainerId.ID", id, character_containers);
        }
    }
    for work in &placed.works {
        for (field, id) in work_owner_ids(work) {
            let targets = if field == "owner_map_object_concrete_model_id" {
                concrete
            } else {
                structures
            };
            check(field, id, targets);
        }
        for id in work_assign_individual_ids(work) {
            check("work assign assigned_individual_id", id, characters);
        }
    }
    if let Some(id) = base_camp_owner_id(&placed) {
        check("base camp owner_map_object_instance_id", id, structures);
    }

    assert!(
        checked > 400,
        "expected the placed base's whole live reference surface, resolved only {checked}"
    );
}
