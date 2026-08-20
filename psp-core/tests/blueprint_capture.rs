mod common;

use psp_core::domain::world;

#[test]
fn work_values_present_returns_entries() {
    let session = common::load_fixture_session("v1_relics");
    let works = world::work_values(&session.level).expect("WorkSaveData accessor must not error");
    assert!(
        works.is_some(),
        "the v1_relics fixture has 15 bases, so WorkSaveData must be present"
    );
}

use psp_core::domain::blueprint::{capture, scrub, CaptureOptions};
use psp_core::domain::guild;
use uuid::Uuid;

#[test]
fn capture_collects_the_bases_structures() {
    let session = common::load_fixture_session("v1_relics");
    let base_id = common::fixture_base_id(&session);

    let blueprint = capture::capture(&session, base_id, CaptureOptions::blueprint(), "Home")
        .expect("capture must succeed for a base that exists");

    assert!(!blueprint.structures.is_empty(), "a real base must capture at least one structure");
    assert_eq!(
        blueprint.header.structure_count as usize,
        blueprint.structures.len(),
        "the header count must match the captured structures"
    );
}

#[test]
fn captured_transforms_are_relative_to_the_anchor() {
    let session = common::load_fixture_session("v1_relics");
    let base_id = common::fixture_base_id(&session);

    let blueprint =
        capture::capture(&session, base_id, CaptureOptions::blueprint(), "Home").expect("capture");

    let max_offset = blueprint
        .structures
        .iter()
        .map(|s| s.relative_transform.translation.x.0.abs())
        .fold(0.0_f64, f64::max);
    assert!(
        max_offset < 100_000.0,
        "relative offsets must be base-scale, not world coordinates: got {max_offset}"
    );
}

#[test]
fn capture_of_an_unknown_base_is_an_error() {
    let session = common::load_fixture_session("v1_relics");

    let result =
        capture::capture(&session, uuid::Uuid::nil(), CaptureOptions::blueprint(), "Nope");

    assert!(result.is_err(), "capturing a base that does not exist must error");
}

#[test]
fn the_blueprint_preset_drops_container_contents() {
    let session = common::load_fixture_session("v1_relics");
    let base_id = common::fixture_base_id(&session);

    let blueprint =
        capture::capture(&session, base_id, CaptureOptions::blueprint(), "Home").expect("capture");

    // Containers a structure references still travel (an absent one crashes the
    // game), but carry no items.
    for entry in &blueprint.item_containers {
        assert!(
            capture::container_slot_dynamic_item_ids(entry).is_empty(),
            "the blueprint preset must drop container contents (containers travel empty)"
        );
    }
    assert!(
        blueprint.dynamic_items.is_empty(),
        "no dynamic items ship when the blueprint preset drops container contents"
    );
    assert!(
        !blueprint.header.manifest.container_contents,
        "the manifest must record that contents were not captured"
    );
}

#[test]
fn scrubbing_zeroes_the_build_player_uid() {
    let session = common::load_fixture_session("v1_relics");
    let base_id = common::fixture_base_id(&session);
    let mut unscrubbed =
        capture::capture_unscrubbed(&session, base_id, CaptureOptions::full(), "Home")
            .expect("capture");

    let owner = capture::first_build_player_uid(&unscrubbed)
        .expect("the fixture base must have a built structure with an owner");
    assert!(!owner.is_nil(), "precondition: the fixture owner uid must be non-nil");

    scrub::scrub_blueprint(&mut unscrubbed);

    let after = capture::first_build_player_uid(&unscrubbed)
        .expect("the structure must still be present after scrubbing");
    assert!(after.is_nil(), "no build_player_uid may survive scrubbing");
}

#[test]
fn capture_scrubs_without_being_asked() {
    let session = common::load_fixture_session("v1_relics");
    let base_id = common::fixture_base_id(&session);

    let blueprint =
        capture::capture(&session, base_id, CaptureOptions::full(), "Home").expect("capture");

    let owners = capture::structure_build_player_uids(&blueprint);
    assert!(!owners.is_empty(), "the fixture base must have structures with a build_player_uid");
    assert!(
        owners.iter().all(|uid| uid.is_nil()),
        "capture must scrub every structure's build_player_uid, never returning an unscrubbed blueprint: {owners:?}"
    );
}

#[test]
fn capture_full_leaves_no_player_uid_in_any_concrete_model() {
    let session = common::load_fixture_session("v1_relics");
    let base_id = common::fixture_base_id(&session);

    let blueprint =
        capture::capture(&session, base_id, CaptureOptions::full(), "Home").expect("capture");

    let mut checked_any_uid = false;
    for structure in &blueprint.structures {
        let uids = capture::structure_concrete_player_uids(&structure.properties);
        checked_any_uid |= !uids.is_empty();
        assert!(
            uids.iter().all(|uid| uid.is_nil()),
            "structure {} leaked a player uid in its ConcreteModel: {uids:?}",
            structure.map_object_id
        );
    }
    if !checked_any_uid {
        eprintln!(
            "note: the v1_relics fixture base has no signboard/pal-egg/chest/password-locked \
             structure, so this test is a regression guard, not proof of a live fix"
        );
    }
}

#[test]
fn capture_scrubs_all_captured_character_player_uids() {
    let session = common::load_fixture_session("v1_relics");
    let base_id = common::fixture_base_id(&session);

    let blueprint =
        capture::capture(&session, base_id, CaptureOptions::full(), "Home").expect("capture");

    assert!(
        !blueprint.characters.is_empty(),
        "the v1_relics fixture base must capture worker pal characters"
    );
    for entry in &blueprint.characters {
        let (key_uid, owner_uid, old_owner_uids) = capture::character_entry_player_uids(entry);
        assert!(key_uid.is_nil(), "character key PlayerUId must be scrubbed, got {key_uid}");
        assert!(owner_uid.is_nil(), "OwnerPlayerUId must be scrubbed, got {owner_uid}");
        assert!(
            old_owner_uids.iter().all(|uid| uid.is_nil()),
            "OldOwnerPlayerUIds must be empty or all nil, got {old_owner_uids:?}"
        );
    }
}

#[test]
fn scrubbing_leaves_structure_geometry_untouched() {
    let session = common::load_fixture_session("v1_relics");
    let base_id = common::fixture_base_id(&session);
    let mut blueprint =
        capture::capture_unscrubbed(&session, base_id, CaptureOptions::full(), "Home")
            .expect("capture");

    let before: Vec<f64> = blueprint
        .structures
        .iter()
        .map(|s| s.relative_transform.translation.x.0)
        .collect();
    scrub::scrub_blueprint(&mut blueprint);
    let after: Vec<f64> = blueprint
        .structures
        .iter()
        .map(|s| s.relative_transform.translation.x.0)
        .collect();

    assert_eq!(before, after, "scrubbing must not move structures");
}

#[test]
fn the_full_preset_captures_container_contents() {
    let session = common::load_fixture_session("v1_relics");
    let base_id = common::fixture_base_id(&session);

    let blueprint =
        capture::capture(&session, base_id, CaptureOptions::full(), "Home").expect("capture");

    assert!(
        !blueprint.item_containers.is_empty(),
        "the full preset must carry the base's item containers"
    );
    assert!(
        blueprint.header.manifest.container_contents,
        "the manifest must record that contents were captured"
    );
}

#[test]
fn item_containers_are_not_duplicated() {
    let session = common::load_fixture_session("v1_relics");
    let base_id = common::fixture_base_id(&session);

    let blueprint =
        capture::capture(&session, base_id, CaptureOptions::full(), "Home").expect("capture");

    let mut ids: Vec<String> = blueprint
        .item_containers
        .iter()
        .map(|e| format!("{:?}", e.key))
        .collect();
    let total = ids.len();
    ids.sort();
    ids.dedup();
    assert_eq!(ids.len(), total, "each item container must appear exactly once");
}

#[test]
fn dynamic_items_are_limited_to_those_the_containers_reference() {
    let session = common::load_fixture_session("v1_relics");
    let base_id = common::fixture_base_id(&session);

    let blueprint =
        capture::capture(&session, base_id, CaptureOptions::full(), "Home").expect("capture");

    let all_dynamic = psp_core::domain::world::dynamic_item_values(&session.level)
        .expect("dynamic items")
        .len();
    assert!(
        !blueprint.dynamic_items.is_empty(),
        "the fixture base must reference at least one dynamic item"
    );
    assert!(
        blueprint.dynamic_items.len() < all_dynamic,
        "a wholesale copy of every dynamic item in the save would not filter: captured {} of {}",
        blueprint.dynamic_items.len(),
        all_dynamic
    );

    let referenced_ids: std::collections::HashSet<Uuid> = blueprint
        .item_containers
        .iter()
        .flat_map(capture::container_slot_dynamic_item_ids)
        .collect();
    for item in &blueprint.dynamic_items {
        let id = capture::dynamic_item_local_id(item).expect("captured dynamic item must decode");
        assert!(
            referenced_ids.contains(&id),
            "captured dynamic item {id} is not referenced by any captured container slot"
        );
    }
}

#[test]
fn asking_for_workers_does_not_capture_caged_pals() {
    let session = common::load_fixture_session("v1_relics");
    let base_id = common::fixture_base_id(&session);

    let workers_only = CaptureOptions { worker_pals: true, ..CaptureOptions::blueprint() };
    let blueprint = capture::capture(&session, base_id, workers_only, "Home").expect("capture");

    // `manifest` is a verbatim copy of `options`, so asserting on it alone would pass by
    // construction; inspect the actually captured character containers instead.
    let base_entry = psp_core::domain::world::base_camp_map(&session.level)
        .expect("base camp map")
        .and_then(|entries| entries.iter().find(|entry| psp_core::props::as_uuid(&entry.key) == Some(base_id)))
        .expect("fixture base camp entry must exist");
    let (_guild_id, worker_container_id) = guild::base_guild_and_container(base_entry)
        .expect("fixture base must resolve a worker container");

    let captured_container_ids: Vec<Uuid> = blueprint
        .character_containers
        .iter()
        .filter_map(capture::container_entry_id)
        .collect();

    // Housed containers a structure references also travel, but emptied -- no
    // caged pals come along.
    assert!(
        captured_container_ids.contains(&worker_container_id),
        "workers_only capture must contain the base's worker container"
    );
    for entry in &blueprint.character_containers {
        if capture::container_entry_id(entry) == Some(worker_container_id) {
            continue;
        }
        assert!(
            capture::character_container_slot_instance_ids(entry).is_empty(),
            "a housed container captured for referential integrity must carry no caged pals"
        );
    }
}

/// The base camp's `WorkerDirector` is the only thing naming the base's worker
/// container, so it must travel at every layer or a placed base ends up with none.
#[test]
fn a_layer_without_worker_pals_captures_the_container_but_none_of_its_pals() {
    let session = common::load_fixture_session("v1_relics");
    let base_id = common::fixture_base_id(&session);

    let base_entry = psp_core::domain::world::base_camp_map(&session.level)
        .expect("base camp map")
        .and_then(|entries| {
            entries.iter().find(|entry| psp_core::props::as_uuid(&entry.key) == Some(base_id))
        })
        .expect("fixture base camp entry must exist");
    let (_guild_id, worker_container_id) = guild::base_guild_and_container(base_entry)
        .expect("fixture base must resolve a worker container");
    let source_entry = psp_core::domain::world::character_container_map(&session.level)
        .expect("character containers")
        .iter()
        .find(|entry| capture::container_entry_id(entry) == Some(worker_container_id))
        .expect("the worker container must be in the save");
    let (source_slots, source_pals) = common::container_slot_census(source_entry);
    assert!(
        source_slots > 0 && source_pals > 0,
        "the fixture's worker container must have slots and pals in them, or an emptied copy \
         is indistinguishable from the original: {source_pals} of {source_slots}"
    );

    for (layer, options, expected_pals) in [
        ("blueprint", CaptureOptions::blueprint(), 0),
        ("configured", CaptureOptions::configured(), 0),
        ("full", CaptureOptions::full(), source_pals),
    ] {
        let blueprint = capture::capture(&session, base_id, options, "Home").expect("capture");
        let worker = blueprint
            .character_containers
            .iter()
            .find(|entry| capture::container_entry_id(entry) == Some(worker_container_id))
            .unwrap_or_else(|| panic!("{layer}: the base's worker container must be captured"));
        assert_eq!(
            common::container_slot_census(worker),
            (source_slots, expected_pals),
            "{layer}: the captured worker container must keep the base's capacity and hold only \
             the pals this layer asked for"
        );
        assert_eq!(
            blueprint.characters.is_empty(),
            expected_pals == 0,
            "{layer}: an emptied worker container must come with no captured pals, and a filled \
             one with some"
        );
    }
}

#[test]
fn base_identity_is_dropped_when_not_requested() {
    let session = common::load_fixture_session("v1_relics");
    let base_id = common::fixture_base_id(&session);

    let blueprint =
        capture::capture(&session, base_id, CaptureOptions::blueprint(), "Home").expect("capture");

    assert!(
        blueprint.header.source_base.is_empty(),
        "the source base name must be withheld unless base_identity was requested"
    );
}

/// The world name identifies its author's world as squarely as the base name. Pinned
/// at all three presets, since a leak the default preset alone avoids is still a leak.
#[test]
fn the_source_world_is_withheld_unless_base_identity_is_requested() {
    let session = common::load_fixture_session("v1_relics");
    let base_id = common::fixture_base_id(&session);

    let world_name = session.world_name.clone();
    assert!(!world_name.is_empty(), "setup: the fixture save must carry a world name");

    for (layer, options) in [
        ("blueprint", CaptureOptions::blueprint()),
        ("configured", CaptureOptions::configured()),
        ("full", CaptureOptions::full()),
    ] {
        let blueprint = capture::capture(&session, base_id, options, "Home").expect("capture");
        assert_eq!(
            blueprint.header.source_world,
            if options.base_identity { world_name.clone() } else { String::new() },
            "{layer}: the source world name must travel only with the base identity"
        );
    }
}

#[test]
fn structure_container_refs_resolve_when_contents_and_pals_off() {
    use std::collections::HashSet;

    let session = common::load_fixture_session("v1_relics");
    let base_id = common::fixture_base_id(&session);
    // `blueprint()` captures production_config only, so container_contents AND
    // housed_pals are both OFF -- dangling container references here crash Palworld
    // on `IsWorkable`.
    let bp = capture::capture(&session, base_id, CaptureOptions::blueprint(), "Home").expect("capture");

    let item_containers: HashSet<Uuid> =
        bp.item_containers.iter().filter_map(capture::container_entry_id).collect();
    let character_containers: HashSet<Uuid> =
        bp.character_containers.iter().filter_map(capture::container_entry_id).collect();

    let mut refs = 0usize;
    for structure in &bp.structures {
        let (item_ids, character_ids) = capture::module_target_container_ids(&structure.properties);
        for id in item_ids.into_iter().filter(|id| *id != Uuid::nil()) {
            refs += 1;
            assert!(
                item_containers.contains(&id),
                "structure {} references item container {id} absent from the blueprint",
                structure.map_object_id
            );
        }
        for id in character_ids.into_iter().filter(|id| *id != Uuid::nil()) {
            refs += 1;
            assert!(
                character_containers.contains(&id),
                "structure {} references character container {id} absent from the blueprint",
                structure.map_object_id
            );
        }
    }
    assert!(refs > 0, "fixture base must reference at least one container for this test to be meaningful");

    // container_contents off: every captured item container is emptied.
    for entry in &bp.item_containers {
        assert!(
            capture::container_slot_dynamic_item_ids(entry).is_empty(),
            "container_contents off must leave item containers empty"
        );
    }
}
