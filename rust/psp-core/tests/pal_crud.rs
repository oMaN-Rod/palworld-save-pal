mod common;

use psp_core::domain::{containers, pal, player, world};
use psp_core::dto::pal::PalDto;
use psp_core::error::CoreError;
use psp_core::gamedata::GameData;
use psp_core::progress::null_progress;
use psp_core::session::{LoadedPlayer, SaveKind, SaveSession};
use uesave::{
    Header, MapEntry, PackageVersion, Properties, Property, PropertySchemas, Root, Save,
    StructValue, ValueVec,
};
use uuid::Uuid;

fn game_data() -> GameData {
    let json_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data/json");
    GameData::load(&json_dir).expect("data dir")
}

fn guid_property(id: Uuid) -> Property {
    Property::Struct(StructValue::Guid(psp_core::props::uuid_to_guid(id)))
}

fn minimal_save(properties: Properties) -> Save {
    Save {
        header: Header {
            magic: 0,
            save_game_version: 0,
            package_version: PackageVersion { ue4: 0, ue5: None },
            engine_version_major: 0,
            engine_version_minor: 0,
            engine_version_patch: 0,
            engine_version_build: 0,
            engine_version: String::new(),
            custom_version: None,
        },
        schemas: PropertySchemas::default(),
        root: Root {
            save_game_type: String::new(),
            properties,
        },
        extra: Vec::new(),
    }
}

// ============================================================================
// Real-save coverage (`tests/fixtures/saves/world1`, always runs -- see
// `tests/common/mod.rs`'s own doc comment and `pal_write.rs`'s established
// convention for this workspace). world1 has 2 players and no `_dps.sav`
// files for either (verified: `ls tests/fixtures/saves/world1/Players`), so
// DPS ops below are exercised synthetically instead.
// ============================================================================

fn loaded_session(session: &mut SaveSession, data: &GameData) -> Uuid {
    let player_id = *session
        .player_summaries
        .keys()
        .next()
        .expect("world1 has a player");
    player::get_player_details(session, data, player_id, &null_progress())
        .unwrap()
        .expect("player loads");
    player_id
}

/// The mandated positive cache-invalidation proof (this task's brief:
/// "prove the positive: after a delete, the rebuilt index differs, and the
/// cache was cleared" -- mirrors `world_index.rs`'s
/// `stale_character_index_after_removal_would_resolve_the_wrong_entry`, but
/// exercised through the real `add_player_pal`/`delete_player_pals` CRUD
/// entry points rather than raw map surgery).
#[test]
fn add_and_delete_player_pal_invalidate_caches_and_shift_the_rebuilt_index() {
    let mut session = common::load_fixture_session("world1");
    let data = game_data();
    // Needs a player who already owns at least one pal, to serve as the
    // "earlier entry" deleted below -- not just "whichever player happens
    // to be first in `player_summaries`'s (unordered) iteration order",
    // which may legitimately own none in a given fixture (world1's second
    // player owns zero pals -- verified directly).
    let mut player_id = None;
    for candidate in session.player_summaries.keys().copied().collect::<Vec<_>>() {
        player::get_player_details(&mut session, &data, candidate, &null_progress())
            .unwrap()
            .expect("player loads");
        let details = player::build_player_dto(&session, &data, candidate)
            .unwrap()
            .unwrap();
        if !details.pals.is_empty() {
            player_id = Some(candidate);
            break;
        }
    }
    let Some(player_id) = player_id else {
        eprintln!("world1 has no player owning any pal; nothing to prove");
        return;
    };
    let details = player::build_player_dto(&session, &data, player_id)
        .unwrap()
        .unwrap();
    let pal_box_id = details.pal_box_id.expect("pal box exists");
    let entry_count_before = world::character_map(&session.level).unwrap().len();

    session.caches.character_index = Some(world::build_character_index(&session.level));

    let new_pal = pal::add_player_pal(
        &mut session,
        &data,
        player_id,
        "Sheepball",
        "planned",
        pal_box_id,
        None,
    )
    .unwrap()
    .expect("world1's pal box has room for one more pal");
    assert_eq!(new_pal.character_id, "Sheepball");
    assert_eq!(new_pal.owner_uid, Some(player_id));
    assert_eq!(new_pal.storage_id, pal_box_id);
    assert_eq!(
        world::character_map(&session.level).unwrap().len(),
        entry_count_before + 1
    );
    assert!(
        session.caches.character_index.is_none(),
        "add_player_pal must invalidate caches"
    );

    let index_after_add = world::build_character_index(&session.level);
    let position_after_add = *index_after_add.get(&new_pal.instance_id).unwrap();
    assert_eq!(position_after_add, entry_count_before);

    // Warm the cache again, then delete an EARLIER entry -- this must shift
    // every later position (including the just-added pal's) down by one.
    // Must be an actual PAL this player owns (not `entries[0]`, which is
    // frequently the player's OWN character entry, `is_player == true` --
    // `delete_player_pals`'s ownership guard, this task's Critical fix,
    // correctly rejects that as "not a pal player_id owns", matching
    // Python's `Player.pals` scoping, which never includes the player's own
    // entry either).
    session.caches.character_index = Some(index_after_add);
    let earlier_pal_id = {
        let entries = world::character_map(&session.level).unwrap();
        entries
            .iter()
            .find(|entry| {
                !world::entry_is_player(entry)
                    && world::entry_save_parameter(entry).and_then(|params| {
                        psp_core::props::get(params, &["OwnerPlayerUId"])
                            .and_then(psp_core::props::as_uuid)
                    }) == Some(player_id)
            })
            .and_then(world::entry_instance_id)
            .expect("player_id must already own at least one pal earlier in the map")
    };
    assert_ne!(earlier_pal_id, new_pal.instance_id);

    pal::delete_player_pals(&mut session, player_id, &[earlier_pal_id]).unwrap();
    assert!(
        session.caches.character_index.is_none(),
        "delete_player_pals must invalidate caches"
    );
    assert_eq!(
        world::character_map(&session.level).unwrap().len(),
        entry_count_before
    );

    let index_after_delete = world::build_character_index(&session.level);
    assert!(!index_after_delete.contains_key(&earlier_pal_id));
    let position_after_delete = *index_after_delete
        .get(&new_pal.instance_id)
        .expect("the new pal must still be present after deleting a different pal");
    assert_eq!(
        position_after_delete,
        position_after_add - 1,
        "removing an earlier entry must shift every later position -- a stale \
         (not-invalidated) index would still claim the new pal lives at its \
         pre-delete position, silently editing whatever pal now occupies it"
    );
}

#[test]
fn unloaded_player_errors_with_python_message() {
    let mut session = common::load_fixture_session("world1");
    let data = game_data();
    let ghost = Uuid::new_v4();
    let error = pal::add_player_pal(
        &mut session,
        &data,
        ghost,
        "Sheepball",
        "x",
        Uuid::new_v4(),
        None,
    )
    .unwrap_err();
    assert_eq!(
        error.to_string(),
        format!("Player {ghost} not found in the save file.")
    );
}

#[test]
fn clone_pal_matches_source_stats_when_the_pal_box_has_room() {
    let mut session = common::load_fixture_session("world1");
    let data = game_data();
    let player_id = loaded_session(&mut session, &data);
    let details = player::build_player_dto(&session, &data, player_id)
        .unwrap()
        .unwrap();
    let Some((_, source_pal)) = details.pals.iter().next() else {
        eprintln!("world1's chosen player has no pals; nothing to clone");
        return;
    };
    let source = source_pal.clone();
    let entry_count_before = world::character_map(&session.level).unwrap().len();
    session.caches.character_index = Some(world::build_character_index(&session.level));

    match pal::clone_pal(&mut session, &data, &source).unwrap() {
        Some(cloned) => {
            assert_ne!(cloned.instance_id, source.instance_id);
            assert_eq!(cloned.character_id, source.character_id);
            assert_eq!(cloned.talent_hp, source.talent_hp);
            assert_eq!(cloned.owner_uid, Some(player_id));
            assert_eq!(
                world::character_map(&session.level).unwrap().len(),
                entry_count_before + 1
            );
            assert!(
                session.caches.character_index.is_none(),
                "clone_pal must invalidate caches"
            );
        }
        None => {
            // Pal box genuinely full, or the real fixture's first free slot
            // happened to be 0 (PARITY-BUG-2) -- both acceptable here; the
            // dedicated, deterministic pin of the slot-0 case is below.
        }
    }
}

#[test]
fn move_pal_updates_slot_membership() {
    let mut session = common::load_fixture_session("world1");
    let data = game_data();
    let player_id = loaded_session(&mut session, &data);
    let details = player::build_player_dto(&session, &data, player_id)
        .unwrap()
        .unwrap();
    let party = details.party.clone().unwrap();
    let pal_box = details.pal_box.clone().unwrap();
    let Some(boxed_slot) = pal_box.slots.iter().find(|s| s.pal_id.is_some()) else {
        eprintln!("world1's pal box is empty; nothing to move");
        return;
    };
    let pal_id = boxed_slot.pal_id.unwrap();
    if party.slots.len() as i32 >= party.size {
        eprintln!("world1's party is full; nothing to prove");
        return;
    }
    let moved = pal::move_pal(&mut session, &data, player_id, pal_id, party.id)
        .unwrap()
        .expect("party has space");
    assert_eq!(moved.instance_id, pal_id);
    assert_eq!(
        moved.storage_id, pal_box.id,
        "PARITY-BUG-1: ContainerId never changes on move, only SlotIndex does"
    );
    let after = player::build_player_dto(&session, &data, player_id)
        .unwrap()
        .unwrap();
    assert!(after
        .party
        .unwrap()
        .slots
        .iter()
        .any(|s| s.pal_id == Some(pal_id)));
    assert!(!after
        .pal_box
        .unwrap()
        .slots
        .iter()
        .any(|s| s.pal_id == Some(pal_id)));
}

/// Proof of the check-before-mutate fix relative to the brief's own
/// reference code: a `pal_id` that belongs to a DIFFERENT player must be
/// rejected before any container is touched, not silently succeed-as-"full".
#[test]
fn move_pal_rejects_a_pal_not_owned_by_this_player_without_mutating_any_container() {
    let mut session = common::load_fixture_session("world1");
    let data = game_data();
    let player_ids: Vec<Uuid> = session.player_summaries.keys().copied().collect();
    if player_ids.len() < 2 {
        eprintln!("world1 must have 2 players for this test; skipping");
        return;
    }
    let (player_a, player_b) = (player_ids[0], player_ids[1]);
    player::get_player_details(&mut session, &data, player_a, &null_progress())
        .unwrap()
        .expect("player a loads");
    player::get_player_details(&mut session, &data, player_b, &null_progress())
        .unwrap()
        .expect("player b loads");
    let details_b = player::build_player_dto(&session, &data, player_b)
        .unwrap()
        .unwrap();
    let Some((&foreign_pal_id, _)) = details_b.pals.iter().next() else {
        eprintln!("player b has no pals; nothing to prove");
        return;
    };
    let details_a = player::build_player_dto(&session, &data, player_a)
        .unwrap()
        .unwrap();
    let pal_box_a = details_a.pal_box.clone().unwrap();
    let container_index = world::build_character_container_index(&session.level);
    let &entry_index = container_index.get(&pal_box_a.id).unwrap();
    let slots_before = containers::read_character_container(&session.level, entry_index)
        .unwrap()
        .slots;

    let error =
        pal::move_pal(&mut session, &data, player_a, foreign_pal_id, pal_box_a.id).unwrap_err();
    assert!(matches!(error, CoreError::PalNotFound(id) if id == foreign_pal_id));

    let slots_after = containers::read_character_container(&session.level, entry_index)
        .unwrap()
        .slots;
    assert_eq!(
        slots_before.len(),
        slots_after.len(),
        "a rejected move must not append a phantom slot for the foreign pal id"
    );
    assert!(
        !slots_after.iter().any(|s| s.pal_id == Some(foreign_pal_id)),
        "the foreign pal id must never appear in player A's pal box"
    );
}

/// Critical fix (this task's review): `delete_player_pals` must reject a
/// `pal_id` that belongs to a DIFFERENT player BEFORE mutating anything --
/// mirrors `move_pal_rejects_a_pal_not_owned_by_this_player_without_
/// mutating_any_container` above. Without the ownership guard,
/// `delete_pal_entry`'s unscoped whole-map search would delete player B's
/// pal entirely from the save when called through player A.
#[test]
fn delete_player_pals_rejects_a_pal_not_owned_by_this_player_without_mutating_anything() {
    let mut session = common::load_fixture_session("world1");
    let data = game_data();
    let player_ids: Vec<Uuid> = session.player_summaries.keys().copied().collect();
    if player_ids.len() < 2 {
        eprintln!("world1 must have 2 players for this test; skipping");
        return;
    }
    let (player_a, player_b) = (player_ids[0], player_ids[1]);
    player::get_player_details(&mut session, &data, player_a, &null_progress())
        .unwrap()
        .expect("player a loads");
    player::get_player_details(&mut session, &data, player_b, &null_progress())
        .unwrap()
        .expect("player b loads");
    let details_b = player::build_player_dto(&session, &data, player_b)
        .unwrap()
        .unwrap();
    let Some((&foreign_pal_id, _)) = details_b.pals.iter().next() else {
        eprintln!("player b has no pals; nothing to prove");
        return;
    };
    let entry_count_before = world::character_map(&session.level).unwrap().len();
    let details_a = player::build_player_dto(&session, &data, player_a)
        .unwrap()
        .unwrap();
    let pal_box_a = details_a.pal_box.clone().unwrap();
    let container_index = world::build_character_container_index(&session.level);
    let &entry_index_a = container_index.get(&pal_box_a.id).unwrap();
    let slots_before = containers::read_character_container(&session.level, entry_index_a)
        .unwrap()
        .slots;

    let error = pal::delete_player_pals(&mut session, player_a, &[foreign_pal_id]).unwrap_err();
    assert!(matches!(error, CoreError::PalNotFound(id) if id == foreign_pal_id));

    assert_eq!(
        world::character_map(&session.level).unwrap().len(),
        entry_count_before,
        "a rejected delete must not remove player B's pal from the save"
    );
    assert!(
        world::character_map(&session.level)
            .unwrap()
            .iter()
            .any(|e| world::entry_instance_id(e) == Some(foreign_pal_id)),
        "player B's pal must still exist in the character map"
    );
    let slots_after = containers::read_character_container(&session.level, entry_index_a)
        .unwrap()
        .slots;
    assert_eq!(
        slots_before, slots_after,
        "player A's own pal box must be untouched by a rejected delete of a \
         pal A never owned"
    );
}

#[test]
fn heal_pals_clears_sickness_and_skips_a_missing_id_without_erroring() {
    let mut session = common::load_fixture_session("world1");
    let data = game_data();
    let player_id = loaded_session(&mut session, &data);
    let details = player::build_player_dto(&session, &data, player_id)
        .unwrap()
        .unwrap();
    if details.pals.is_empty() {
        eprintln!("world1's chosen player has no pals; nothing to heal");
        return;
    }
    let mut pal_ids: Vec<Uuid> = details.pals.iter().map(|(id, _)| *id).collect();
    let missing_id = Uuid::new_v4();
    pal_ids.push(missing_id); // pal_ops.py: a missing id is skipped, not an error

    pal::heal_pals(&mut session, &data, &pal_ids).unwrap();

    let healed = player::build_player_dto(&session, &data, player_id)
        .unwrap()
        .unwrap();
    for (_, healed_pal) in healed.pals.iter() {
        assert!(!healed_pal.is_sick);
        assert_eq!(healed_pal.sanity, 100.0);
    }
}

#[test]
fn heal_all_player_pals_heals_every_owned_pal() {
    let mut session = common::load_fixture_session("world1");
    let data = game_data();
    let player_id = loaded_session(&mut session, &data);
    let before = player::build_player_dto(&session, &data, player_id)
        .unwrap()
        .unwrap();
    if before.pals.is_empty() {
        eprintln!("world1's chosen player has no pals; nothing to heal");
        return;
    }

    pal::heal_all_player_pals(&mut session, &data, player_id).unwrap();

    let after = player::build_player_dto(&session, &data, player_id)
        .unwrap()
        .unwrap();
    for (_, healed_pal) in after.pals.iter() {
        assert!(!healed_pal.is_sick);
        assert_eq!(healed_pal.sanity, 100.0);
    }
}

// world1's founding guild + its one real base -- see `guild_details.rs`'s
// own constants doc comment: worker container SlotNum 1, 0 filled (empty).
const WORLD1_GUILD_WITH_BASE: &str = "54491484-4e6c-7327-70b2-868f350929f6";
const WORLD1_BASE_ID: &str = "4bb24de8-4965-af19-f596-e296089e8ab0";

/// Real-save proof that base pals do NOT carry PARITY-BUG-2: the base's
/// worker container starts with its ONLY slot (index 0) empty, so
/// `add_guild_pal` must succeed there (`Base.add_pal` checks `is None`, not
/// falsy) -- in direct contrast to `clone_pal_at_slot_zero_...` below.
/// Also the real-save proof for the newly-found `OwnerPlayerUId`
/// safe_remove-wrong-dict bug (see `add_guild_pal`'s doc comment): the
/// freshly created base pal must carry a PRESENT (nil) OwnerPlayerUId, not
/// an absent one.
#[test]
fn add_guild_pal_at_slot_zero_succeeds_and_leaves_owner_player_uid_present() {
    let mut session = common::load_fixture_session("world1");
    let data = game_data();
    let guild_id: Uuid = WORLD1_GUILD_WITH_BASE.parse().unwrap();
    let base_id: Uuid = WORLD1_BASE_ID.parse().unwrap();
    // get_guild_details warms `session.loaded_guilds` the same way Task 8's
    // lazy-load path does -- add_guild_pal requires the guild to be loaded.
    psp_core::domain::guild::get_guild_details(&mut session, &data, guild_id)
        .unwrap()
        .expect("guild loads");

    let container_index = world::build_character_container_index(&session.level);
    let base_camp = world::base_camp_map(&session.level)
        .unwrap()
        .unwrap()
        .iter()
        .find(|entry| psp_core::props::as_uuid(&entry.key) == Some(base_id))
        .unwrap();
    let (_, worker_container_id) =
        psp_core::domain::guild::base_guild_and_container(base_camp).unwrap();
    let entry_index = *container_index.get(&worker_container_id).unwrap();
    let before = containers::read_character_container(&session.level, entry_index).unwrap();
    assert!(
        before.slots.is_empty(),
        "test precondition: world1's base worker container must start empty"
    );

    session.caches.character_container_index = Some(container_index);

    let new_pal = pal::add_guild_pal(
        &mut session,
        &data,
        guild_id,
        base_id,
        "Sheepball",
        "basepal",
        None,
    )
    .unwrap()
    .expect("Base.add_pal's `is None` check accepts slot 0 -- no PARITY-BUG-2 here");

    assert_eq!(new_pal.storage_slot, 0);
    assert_eq!(new_pal.storage_id, worker_container_id);
    assert_eq!(
        new_pal.owner_uid,
        Some(psp_core::props::EMPTY_UUID),
        "reproduces base.py's safe_remove-wrong-dict no-op: OwnerPlayerUId \
         stays present (nil), never actually removed -- see this task's report"
    );
    assert!(
        session.caches.character_container_index.is_none(),
        "add_guild_pal must invalidate caches"
    );

    // delete_guild_pals round-trips this back out and invalidates again.
    session.caches.character_index = Some(world::build_character_index(&session.level));
    pal::delete_guild_pals(&mut session, guild_id, base_id, &[new_pal.instance_id]).unwrap();
    assert!(session.caches.character_index.is_none());
    let after = containers::read_character_container(&session.level, entry_index).unwrap();
    assert!(after.slots.is_empty(), "the base container is empty again");
    assert!(world::character_map(&session.level)
        .unwrap()
        .iter()
        .all(|e| world::entry_instance_id(e) != Some(new_pal.instance_id)));
}

// ============================================================================
// Synthetic multi-guild/base fixtures -- world1's only real base has a
// single already-consumed slot (see `add_guild_pal_at_slot_zero_...`
// above), leaving no room to independently prove `clone_guild_pal`'s cache
// invalidation, nor to exercise a genuine cross-guild/cross-base ownership
// mismatch. Built the same way `clone_bug_fixture` below is: from scratch,
// with full control over occupancy.
// ============================================================================

fn shuffle_guid_bytes(b: [u8; 16]) -> [u8; 16] {
    [
        b[3], b[2], b[1], b[0], b[7], b[6], b[5], b[4], b[11], b[10], b[9], b[8], b[15], b[14],
        b[13], b[12],
    ]
}

/// `WorkerDirector.RawData`'s fixed 118-byte blob -- see
/// `psp_core::palbin::worker_director_container_id`'s own doc comment for
/// the exact field layout (`container_id` at byte offset 98).
fn worker_director_blob(container_id: Uuid) -> Vec<u8> {
    let mut blob = vec![0u8; 118];
    blob[98..114].copy_from_slice(&shuffle_guid_bytes(*container_id.as_bytes()));
    blob
}

fn zero_transform() -> uesave::games::palworld::PalTransform {
    use uesave::{Double, Quat, Vector};
    uesave::games::palworld::PalTransform {
        rotation: Quat {
            x: Double(0.0),
            y: Double(0.0),
            z: Double(0.0),
            w: Double(1.0),
        },
        translation: Vector {
            x: Double(0.0),
            y: Double(0.0),
            z: Double(0.0),
        },
        scale: Vector {
            x: Double(1.0),
            y: Double(1.0),
            z: Double(1.0),
        },
    }
}

fn base_camp_entry(base_id: Uuid, guild_id: Uuid, worker_container_id: Uuid) -> MapEntry {
    use uesave::games::palworld::PalBaseCamp;
    use uesave::ByteArray;
    let camp = PalBaseCamp {
        id: psp_core::props::uuid_to_guid(base_id),
        name: String::new(),
        state: 0,
        transform: zero_transform(),
        area_range: 0.0,
        group_id_belong_to: psp_core::props::uuid_to_guid(guild_id),
        fast_travel_local_transform: zero_transform(),
        owner_map_object_instance_id: uesave::FGuid::nil(),
        trailing_bytes: [0; 4],
    };
    let mut worker_properties = Properties::default();
    worker_properties.insert(
        "RawData",
        Property::Array(ValueVec::Byte(ByteArray::Byte(worker_director_blob(
            worker_container_id,
        )))),
    );
    let mut value_properties = Properties::default();
    value_properties.insert(
        "RawData",
        Property::Struct(StructValue::PalBaseCamp(Box::new(camp))),
    );
    value_properties.insert(
        "WorkerDirector",
        Property::Struct(StructValue::Struct(worker_properties)),
    );
    MapEntry {
        key: guid_property(base_id),
        value: Property::Struct(StructValue::Struct(value_properties)),
    }
}

fn guild_group_entry(guild_id: Uuid) -> MapEntry {
    use uesave::games::palworld::PalGroupData;
    let mut value_properties = Properties::default();
    value_properties.insert(
        "GroupType",
        Property::Enum("EPalGroupType::Guild".to_string()),
    );
    let group_data = PalGroupData {
        group_id: psp_core::props::uuid_to_guid(guild_id),
        group_name: String::new(),
        individual_character_handle_ids: vec![],
        remaining_data: vec![],
    };
    value_properties.insert(
        "RawData",
        Property::Struct(StructValue::PalGroupData(group_data)),
    );
    MapEntry {
        key: guid_property(guild_id),
        value: Property::Struct(StructValue::Struct(value_properties)),
    }
}

/// One `SaveSession` holding N independent guild/base/worker-container
/// triples, each with `session.loaded_guilds` warmed the way
/// `get_guild_details` would leave it -- every base starts with an EMPTY
/// worker container (`CharacterSaveParameterMap` starts empty too); callers
/// seed pals via the real `pal::add_guild_pal` entry point, not hand-built
/// entries, so the fixture's pals are exactly what production code would
/// create.
fn multi_guild_base_session(bases: &[(Uuid, Uuid, Uuid, i32)]) -> SaveSession {
    let mut container_entries = Vec::new();
    let mut base_entries = Vec::new();
    let mut group_entries = Vec::new();
    for &(guild_id, base_id, container_id, slot_num) in bases {
        container_entries.push(empty_character_container_entry(container_id, slot_num));
        base_entries.push(base_camp_entry(base_id, guild_id, container_id));
        group_entries.push(guild_group_entry(guild_id));
    }
    let mut world_save_data = Properties::default();
    world_save_data.insert("CharacterSaveParameterMap", Property::Map(vec![]));
    world_save_data.insert(
        "CharacterContainerSaveData",
        Property::Map(container_entries),
    );
    world_save_data.insert("ItemContainerSaveData", Property::Map(vec![]));
    world_save_data.insert("GroupSaveDataMap", Property::Map(group_entries));
    world_save_data.insert("BaseCampSaveData", Property::Map(base_entries));
    world_save_data.insert(
        "DynamicItemSaveData",
        Property::Array(ValueVec::Struct(vec![])),
    );
    let mut root_properties = Properties::default();
    root_properties.insert(
        "worldSaveData",
        Property::Struct(StructValue::Struct(world_save_data)),
    );
    let level = minimal_save(root_properties);
    let mut session = SaveSession::new_for_tests(SaveKind::InMemory, level);
    for &(guild_id, _, _, _) in bases {
        session.loaded_guilds.insert(guild_id);
    }
    session
}

/// Important-2 fix (this task's review): `clone_guild_pal` had no dedicated
/// positive cache-invalidation proof (world1's only real base has a single
/// already-consumed slot). Mirrors `add_and_delete_player_pal_invalidate_
/// caches_and_shift_the_rebuilt_index`'s own pattern exactly, through the
/// guild/base entry points instead of the player ones: warm the index,
/// clone (an add), assert invalidation, rebuild, record the clone's
/// position; warm again, delete the EARLIER (seed) pal through
/// `delete_guild_pals`, assert invalidation again, rebuild, and assert the
/// cloned pal's position actually shifted down by one -- the concrete,
/// provable consequence a stale index would miss.
#[test]
fn clone_guild_pal_invalidates_caches_and_the_rebuilt_index_reflects_both_the_clone_and_a_later_delete(
) {
    let data = game_data();
    let guild_id = Uuid::new_v4();
    let base_id = Uuid::new_v4();
    let container_id = Uuid::new_v4();
    let mut session = multi_guild_base_session(&[(guild_id, base_id, container_id, 2)]);

    let seed = pal::add_guild_pal(
        &mut session,
        &data,
        guild_id,
        base_id,
        "Sheepball",
        "seed",
        None,
    )
    .unwrap()
    .expect("fixture worker container has room for the seed pal");
    // `clone_guild_pal`'s own (already-reviewed, unchanged) source-pal
    // lookup scopes via `guild::base_container_membership` -- Task 8's
    // "SlotId" (mixed-case)-only rule, `_load_pals_for_container`'s own real
    // Python behavior. `add_guild_pal`/`new_pal_entry` always write "SlotID"
    // (uppercase, `PalObjects.PalCharacterSlotId`'s own literal spelling),
    // so a freshly seeded pal must be re-spelled "SlotId" here to simulate
    // what every REAL, already-saved base pal in this port's own fixtures
    // actually looks like on disk (11/11 world1 pals, per this task's
    // report) -- otherwise `clone_guild_pal` would never find this seed pal
    // at all, regardless of the fix under test here.
    {
        let entries = world::character_map_mut(&mut session.level).unwrap();
        let entry = entries
            .iter_mut()
            .find(|e| world::entry_instance_id(e) == Some(seed.instance_id))
            .unwrap();
        if let Some(save_parameter) = world::entry_save_parameter_mut(entry) {
            if let Some(slot_property) = save_parameter
                .0
                .shift_remove(&uesave::PropertyKey::from("SlotID"))
            {
                save_parameter.insert("SlotId", slot_property);
            }
        }
    }
    let entry_count_before = world::character_map(&session.level).unwrap().len();

    session.caches.character_index = Some(world::build_character_index(&session.level));
    let cloned = pal::clone_guild_pal(&mut session, &data, guild_id, base_id, &seed)
        .unwrap()
        .expect("fixture worker container has room for the clone");
    assert_ne!(cloned.instance_id, seed.instance_id);
    assert_eq!(cloned.storage_id, container_id);
    assert_eq!(
        world::character_map(&session.level).unwrap().len(),
        entry_count_before + 1
    );
    assert!(
        session.caches.character_index.is_none(),
        "clone_guild_pal must invalidate caches"
    );

    let index_after_clone = world::build_character_index(&session.level);
    let position_after_clone = *index_after_clone.get(&cloned.instance_id).unwrap();
    assert_eq!(position_after_clone, entry_count_before);

    session.caches.character_index = Some(index_after_clone);
    pal::delete_guild_pals(&mut session, guild_id, base_id, &[seed.instance_id]).unwrap();
    assert!(
        session.caches.character_index.is_none(),
        "delete_guild_pals must invalidate caches too"
    );
    let index_after_delete = world::build_character_index(&session.level);
    let position_after_delete = *index_after_delete
        .get(&cloned.instance_id)
        .expect("the cloned pal must still be present after deleting the earlier seed pal");
    assert_eq!(
        position_after_delete,
        position_after_clone - 1,
        "removing an earlier entry must shift every later position -- a stale \
         (not-invalidated) index would still claim the cloned pal lives at its \
         pre-delete position"
    );
}

/// Critical fix (this task's review): `delete_guild_pals` must reject a
/// `pal_id` belonging to a DIFFERENT guild/base BEFORE mutating anything --
/// the guild/base analogue of
/// `delete_player_pals_rejects_a_pal_not_owned_by_this_player_without_
/// mutating_anything` above. Without the membership guard,
/// `delete_pal_entry`'s unscoped whole-map search would delete guild B's
/// base pal entirely from the save when called through guild A / base A.
#[test]
fn delete_guild_pals_rejects_a_pal_from_a_different_base_without_mutating_anything() {
    let data = game_data();
    let guild_a = Uuid::new_v4();
    let base_a = Uuid::new_v4();
    let container_a = Uuid::new_v4();
    let guild_b = Uuid::new_v4();
    let base_b = Uuid::new_v4();
    let container_b = Uuid::new_v4();
    let mut session = multi_guild_base_session(&[
        (guild_a, base_a, container_a, 2),
        (guild_b, base_b, container_b, 2),
    ]);

    pal::add_guild_pal(&mut session, &data, guild_a, base_a, "Sheepball", "a", None)
        .unwrap()
        .expect("base a has room");
    let pal_b = pal::add_guild_pal(&mut session, &data, guild_b, base_b, "Sheepball", "b", None)
        .unwrap()
        .expect("base b has room");

    let entry_count_before = world::character_map(&session.level).unwrap().len();
    let container_index = world::build_character_container_index(&session.level);
    let &entry_index_b = container_index.get(&container_b).unwrap();
    let slots_before = containers::read_character_container(&session.level, entry_index_b)
        .unwrap()
        .slots;

    let error =
        pal::delete_guild_pals(&mut session, guild_a, base_a, &[pal_b.instance_id]).unwrap_err();
    assert!(matches!(error, CoreError::PalNotFound(id) if id == pal_b.instance_id));

    assert_eq!(
        world::character_map(&session.level).unwrap().len(),
        entry_count_before,
        "a rejected delete must not remove guild B's base pal from the save"
    );
    assert!(
        world::character_map(&session.level)
            .unwrap()
            .iter()
            .any(|e| world::entry_instance_id(e) == Some(pal_b.instance_id)),
        "guild B's base pal must still exist in the character map"
    );
    let slots_after = containers::read_character_container(&session.level, entry_index_b)
        .unwrap()
        .slots;
    assert_eq!(
        slots_before, slots_after,
        "base B's worker container must be untouched by a rejected delete of \
         a pal base A never owned"
    );
}

// ============================================================================
// Synthetic, deterministic coverage: the two things a real fixture cannot
// reliably force (a pal box whose first free slot is EXACTLY 0, and a DPS
// array -- world1 has none) -- see this task's report for why these are
// built by hand rather than mutated real save data.
// ============================================================================

fn player_character_entry(player_id: Uuid) -> MapEntry {
    let mut key_props = Properties::default();
    key_props.insert("PlayerUId", guid_property(player_id));
    key_props.insert("InstanceId", guid_property(Uuid::new_v4()));
    let mut save_parameter = Properties::default();
    save_parameter.insert("IsPlayer", Property::Bool(true));
    let mut object_props = Properties::default();
    object_props.insert(
        "SaveParameter",
        Property::Struct(StructValue::Struct(save_parameter)),
    );
    let character_data = uesave::games::palworld::PalCharacterData {
        object: object_props,
        unknown_bytes: [0; 4],
        group_id: uesave::FGuid::nil(),
        trailing_bytes: [0; 4],
    };
    let mut value_props = Properties::default();
    value_props.insert(
        "RawData",
        Property::Struct(StructValue::PalCharacterData(character_data)),
    );
    MapEntry {
        key: Property::Struct(StructValue::Struct(key_props)),
        value: Property::Struct(StructValue::Struct(value_props)),
    }
}

fn empty_character_container_entry(container_id: Uuid, slot_num: i32) -> MapEntry {
    let mut key_props = Properties::default();
    key_props.insert("ID", guid_property(container_id));
    let mut value_props = Properties::default();
    value_props.insert("SlotNum", psp_core::props::int_property(slot_num));
    value_props.insert("Slots", Property::Array(ValueVec::Struct(vec![])));
    MapEntry {
        key: Property::Struct(StructValue::Struct(key_props)),
        value: Property::Struct(StructValue::Struct(value_props)),
    }
}

/// A minimal, self-contained `SaveSession` with one loaded player who owns
/// exactly one pal, and an empty (SlotNum 1) pal box -- enough to force the
/// pal box's first free slot to be exactly 0, deterministically.
fn clone_bug_fixture() -> (SaveSession, GameData, Uuid, PalDto) {
    let data = game_data();
    let player_id = Uuid::new_v4();
    let pal_box_id = Uuid::new_v4();
    let otomo_id = Uuid::new_v4();
    let source_pal_id = Uuid::new_v4();

    let player_entry = player_character_entry(player_id);
    let source_entry = pal::new_pal_entry(
        "Sheepball",
        source_pal_id,
        player_id,
        pal_box_id,
        0,
        None,
        "Wooly",
        &data,
    );
    let pal_box_entry = empty_character_container_entry(pal_box_id, 1);

    let mut world_save_data = Properties::default();
    world_save_data.insert(
        "CharacterSaveParameterMap",
        Property::Map(vec![player_entry, source_entry]),
    );
    world_save_data.insert(
        "CharacterContainerSaveData",
        Property::Map(vec![pal_box_entry]),
    );
    world_save_data.insert("ItemContainerSaveData", Property::Map(vec![]));
    world_save_data.insert("GroupSaveDataMap", Property::Map(vec![]));
    world_save_data.insert(
        "DynamicItemSaveData",
        Property::Array(ValueVec::Struct(vec![])),
    );
    let mut root_properties = Properties::default();
    root_properties.insert(
        "worldSaveData",
        Property::Struct(StructValue::Struct(world_save_data)),
    );
    let level = minimal_save(root_properties);

    let mut session = SaveSession::new_for_tests(SaveKind::InMemory, level);

    let mut player_save_data = Properties::default();
    let mut pal_box_id_struct = Properties::default();
    pal_box_id_struct.insert("ID", guid_property(pal_box_id));
    player_save_data.insert(
        "PalStorageContainerId",
        Property::Struct(StructValue::Struct(pal_box_id_struct)),
    );
    let mut otomo_id_struct = Properties::default();
    otomo_id_struct.insert("ID", guid_property(otomo_id));
    player_save_data.insert(
        "OtomoCharacterContainerId",
        Property::Struct(StructValue::Struct(otomo_id_struct)),
    );
    let mut player_root_properties = Properties::default();
    player_root_properties.insert(
        "SaveData",
        Property::Struct(StructValue::Struct(player_save_data)),
    );
    let player_sav = minimal_save(player_root_properties);

    session.loaded_players.insert(
        player_id,
        LoadedPlayer {
            uid: player_id,
            sav: player_sav,
            dps: None,
        },
    );

    let source_dto = {
        let entries = world::character_map(&session.level).unwrap();
        let entry = entries.iter().find(|e| !world::entry_is_player(e)).unwrap();
        pal::pal_dto_from_entry(entry, &data).unwrap()
    };

    (session, data, pal_box_id, source_dto)
}

/// The dedicated PARITY-BUG-2 pin: `Player.clone_pal`'s `if not
/// storage_slot: return` (`player.py`) treats slot index 0 as falsy, so a
/// genuinely empty pal box (first free slot 0) is wrongly reported as
/// "full" -- this is a deliberate, preserved parity bug, not fixed here.
#[test]
fn clone_pal_at_slot_zero_is_deliberately_treated_as_full_parity_bug_2() {
    let (mut session, data, pal_box_id, source_dto) = clone_bug_fixture();
    let entry_count_before = world::character_map(&session.level).unwrap().len();
    let container_index = world::build_character_container_index(&session.level);
    let &entry_index = container_index.get(&pal_box_id).unwrap();
    assert!(
        containers::read_character_container(&session.level, entry_index)
            .unwrap()
            .slots
            .is_empty(),
        "test setup: pal box must start empty so the first free slot is 0"
    );

    let result = pal::clone_pal(&mut session, &data, &source_dto).unwrap();

    assert!(
        result.is_none(),
        "PARITY-BUG-2: a genuinely available first slot (index 0) must be \
         wrongly reported as \"pal box full\", matching Player.clone_pal's \
         `if not storage_slot: return` in player.py"
    );
    assert_eq!(
        world::character_map(&session.level).unwrap().len(),
        entry_count_before,
        "no new CharacterSaveParameterMap entry may be created when the bug fires"
    );

    // Precise reproduction of Python's actual on-disk consequence: the pal
    // box's Slots array already gained an orphaned entry (from
    // `self.pal_box.add_pal(new_pal_id)`, called BEFORE the falsy check)
    // that Python's early `return` never undoes -- this port does not clean
    // it up either, matching Python exactly rather than silently fixing the
    // leak.
    let view = containers::read_character_container(&session.level, entry_index).unwrap();
    assert_eq!(
        view.slots.len(),
        1,
        "the orphaned slot from the failed clone attempt must remain, matching \
         Python's own mutate-then-bail behavior"
    );
    assert_eq!(view.slots[0].slot_index, 0);
    let orphan_pal_id = view.slots[0].pal_id.unwrap();
    assert_ne!(orphan_pal_id, source_dto.instance_id);
    assert!(
        world::character_map(&session.level)
            .unwrap()
            .iter()
            .all(|e| world::entry_instance_id(e) != Some(orphan_pal_id)),
        "the orphaned slot's pal id must not correspond to any real \
         character-map entry -- it was never actually created"
    );
}

/// Contrast case: the SAME player/source setup, but the source pal's
/// `dto.instance_id` is swapped for one that doesn't belong to this player
/// (or exist at all). Because the pal-box mutation happens BEFORE the
/// source lookup in Python (and in this port, matching that order), the
/// same orphaned-slot side effect occurs here too -- this is not
/// PARITY-BUG-2 itself, but the same underlying mutate-before-check
/// mechanism applied to a different failure.
#[test]
fn clone_pal_with_an_unowned_source_id_also_leaves_the_orphaned_slot() {
    let (mut session, data, pal_box_id, mut source_dto) = clone_bug_fixture();
    // Force the box to have TWO slots so slot 0 isn't the bug being tested
    // here -- only the "source not found/not owned" branch is under test.
    {
        let entries = world::character_container_map_mut(&mut session.level).unwrap();
        let value_props = psp_core::props::struct_props_mut(&mut entries[0].value).unwrap();
        value_props.insert("SlotNum", psp_core::props::int_property(2));
    }
    // Pre-occupy slot 0 with an unrelated pal id so the next add lands at 1.
    containers::character_container_add_pal(&mut session.level, 0, Uuid::new_v4(), Some(0))
        .unwrap();
    source_dto.instance_id = Uuid::new_v4(); // no such entry exists

    let entry_count_before = world::character_map(&session.level).unwrap().len();
    let result = pal::clone_pal(&mut session, &data, &source_dto).unwrap();
    assert!(result.is_none());
    assert_eq!(
        world::character_map(&session.level).unwrap().len(),
        entry_count_before
    );
    let container_index = world::build_character_container_index(&session.level);
    let &entry_index = container_index.get(&pal_box_id).unwrap();
    let view = containers::read_character_container(&session.level, entry_index).unwrap();
    assert_eq!(
        view.slots.len(),
        2,
        "the phantom slot from the pal-box mutation (which ran before the \
         missing-source check) must remain"
    );
}

// ---- DPS ops (synthetic -- world1 has no _dps.sav for either player) ----

fn dps_slot(character_id: &str, instance_id: Uuid) -> StructValue {
    let mut save_parameter = Properties::default();
    if character_id != "None" {
        save_parameter.insert("CharacterID", psp_core::props::name_property(character_id));
    }
    save_parameter.insert("Level", psp_core::props::byte_property(5));
    save_parameter.insert("Talent_HP", psp_core::props::byte_property(30));
    let mut container_struct = Properties::default();
    container_struct.insert("ID", guid_property(Uuid::nil()));
    let mut slot_struct = Properties::default();
    slot_struct.insert(
        "ContainerId",
        Property::Struct(StructValue::Struct(container_struct)),
    );
    slot_struct.insert("SlotIndex", psp_core::props::int_property(-1));
    save_parameter.insert("SlotID", Property::Struct(StructValue::Struct(slot_struct)));
    save_parameter.insert(
        "GotStatusPointList",
        Property::Array(ValueVec::Struct(vec![StructValue::Struct({
            let mut p = Properties::default();
            p.insert("StatusName", psp_core::props::name_property("最大HP"));
            p.insert("StatusPoint", psp_core::props::int_property(0));
            p
        })])),
    );
    save_parameter.insert(
        "GotExStatusPointList",
        Property::Array(ValueVec::Struct(vec![])),
    );

    let mut inner_instance_id = Properties::default();
    inner_instance_id.insert("InstanceId", guid_property(instance_id));
    let mut slot_props = Properties::default();
    slot_props.insert(
        "SaveParameter",
        Property::Struct(StructValue::Struct(save_parameter)),
    );
    slot_props.insert(
        "InstanceId",
        Property::Struct(StructValue::Struct(inner_instance_id)),
    );
    StructValue::Struct(slot_props)
}

/// A minimal `SaveSession` with one loaded player whose `.dps` save has a
/// two-slot `SaveParameterArray`: slot 0 empty (template), slot 1 already
/// holding a LUCKY pal (`IsRarePal: true`) -- deliberately, to pin the
/// found-but-not-on-the-PARITY-BUG-list `Pal.reset()` quirk (never touches
/// `IsRarePal`).
fn dps_fixture() -> (SaveSession, GameData, Uuid) {
    let data = game_data();
    let player_id = Uuid::new_v4();

    let mut empty_slot_props = match dps_slot("None", Uuid::nil()) {
        StructValue::Struct(p) => p,
        _ => unreachable!(),
    };
    let _ = &mut empty_slot_props;

    let mut lucky_slot = match dps_slot("Sheepball", Uuid::new_v4()) {
        StructValue::Struct(p) => p,
        _ => unreachable!(),
    };
    if let Some(save_parameter) = lucky_slot
        .0
        .get_mut(&uesave::PropertyKey::from("SaveParameter"))
        .and_then(psp_core::props::struct_props_mut)
    {
        save_parameter.insert("IsRarePal", Property::Bool(true));
    }

    let mut dps_root_properties = Properties::default();
    dps_root_properties.insert(
        "SaveParameterArray",
        Property::Array(ValueVec::Struct(vec![
            dps_slot("None", Uuid::nil()),
            StructValue::Struct(lucky_slot),
        ])),
    );
    let dps_sav = minimal_save(dps_root_properties);

    let mut world_save_data = Properties::default();
    world_save_data.insert("CharacterSaveParameterMap", Property::Map(vec![]));
    world_save_data.insert("CharacterContainerSaveData", Property::Map(vec![]));
    world_save_data.insert("ItemContainerSaveData", Property::Map(vec![]));
    world_save_data.insert("GroupSaveDataMap", Property::Map(vec![]));
    world_save_data.insert(
        "DynamicItemSaveData",
        Property::Array(ValueVec::Struct(vec![])),
    );
    let mut root_properties = Properties::default();
    root_properties.insert(
        "worldSaveData",
        Property::Struct(StructValue::Struct(world_save_data)),
    );
    let level = minimal_save(root_properties);
    let mut session = SaveSession::new_for_tests(SaveKind::InMemory, level);

    let mut player_save_data = Properties::default();
    let mut pal_box_id_struct = Properties::default();
    pal_box_id_struct.insert("ID", guid_property(Uuid::new_v4()));
    player_save_data.insert(
        "PalStorageContainerId",
        Property::Struct(StructValue::Struct(pal_box_id_struct)),
    );
    let mut player_root_properties = Properties::default();
    player_root_properties.insert(
        "SaveData",
        Property::Struct(StructValue::Struct(player_save_data)),
    );
    let player_sav = minimal_save(player_root_properties);

    session.loaded_players.insert(
        player_id,
        LoadedPlayer {
            uid: player_id,
            sav: player_sav,
            dps: Some(dps_sav),
        },
    );

    (session, data, player_id)
}

#[test]
fn add_player_dps_pal_fills_the_first_empty_slot_and_computes_max_hp() {
    let (mut session, data, player_id) = dps_fixture();

    let (slot_index, new_pal) =
        pal::add_player_dps_pal(&mut session, &data, player_id, "Sheepball", "Combat", None)
            .unwrap()
            .expect("slot 0 is empty");
    assert_eq!(slot_index, 0);
    assert_eq!(new_pal.character_id, "Sheepball");
    assert_eq!(new_pal.owner_uid, Some(player_id));
    assert_eq!(new_pal.gender, psp_core::dto::pal::PalGender::Female);
    assert_eq!(new_pal.hp, new_pal.max_hp);
    assert!(new_pal.hp > 0);
}

/// Pins the newly-found `Pal.reset()` quirk: `reset()` never assigns
/// `self.is_lucky`, so a DPS slot recycled from a previously-lucky pal
/// keeps its `IsRarePal` flag straight through `reset()` -- explicitly
/// requesting THAT slot (index 1, pre-seeded lucky) for a brand new pal
/// must still come back lucky.
#[test]
fn add_player_dps_pal_into_a_recycled_slot_inherits_a_stale_is_rare_pal_flag() {
    let (mut session, data, player_id) = dps_fixture();

    let (slot_index, new_pal) = pal::add_player_dps_pal(
        &mut session,
        &data,
        player_id,
        "Sheepball",
        "Combat",
        Some(1),
    )
    .unwrap()
    .expect("slot 1 explicitly requested");
    assert_eq!(slot_index, 1);
    assert_eq!(
        new_pal.is_lucky,
        Some(true),
        "reset() never touches IsRarePal -- a recycled slot's stale lucky \
         flag survives into the freshly created pal (found-but-not-on-the-\
         PARITY-BUG-list Python quirk; see this task's report)"
    );
}

/// A dedicated fixture for the Important-1 fix (this task's review):
/// `add_player_dps_pal` never wrote `FullStomach` at all. Slot 0 is
/// never-used (`CharacterID` "None", no `FullStomach` key whatsoever).
/// Slot 1 is recycled from a real, previously-used "Alpaca" pal (real
/// `max_full_stomach` 225.0 per `data/json/pals.json`) carrying a stale,
/// deliberately-bogus `FullStomach` (999.0, chosen to collide with neither
/// the missing-key default 150.0 nor either species' real max) that must
/// never survive into a freshly created pal.
fn dps_fixture_for_stomach() -> (SaveSession, GameData, Uuid) {
    let data = game_data();
    let player_id = Uuid::new_v4();

    let empty_slot = dps_slot("None", Uuid::nil());

    let mut recycled_slot_props = match dps_slot("Alpaca", Uuid::new_v4()) {
        StructValue::Struct(p) => p,
        _ => unreachable!(),
    };
    if let Some(save_parameter) = recycled_slot_props
        .0
        .get_mut(&uesave::PropertyKey::from("SaveParameter"))
        .and_then(psp_core::props::struct_props_mut)
    {
        save_parameter.insert("FullStomach", psp_core::props::float_property(999.0));
    }

    let mut dps_root_properties = Properties::default();
    dps_root_properties.insert(
        "SaveParameterArray",
        Property::Array(ValueVec::Struct(vec![
            empty_slot,
            StructValue::Struct(recycled_slot_props),
        ])),
    );
    let dps_sav = minimal_save(dps_root_properties);

    let mut world_save_data = Properties::default();
    world_save_data.insert("CharacterSaveParameterMap", Property::Map(vec![]));
    world_save_data.insert("CharacterContainerSaveData", Property::Map(vec![]));
    world_save_data.insert("ItemContainerSaveData", Property::Map(vec![]));
    world_save_data.insert("GroupSaveDataMap", Property::Map(vec![]));
    world_save_data.insert(
        "DynamicItemSaveData",
        Property::Array(ValueVec::Struct(vec![])),
    );
    let mut root_properties = Properties::default();
    root_properties.insert(
        "worldSaveData",
        Property::Struct(StructValue::Struct(world_save_data)),
    );
    let level = minimal_save(root_properties);
    let mut session = SaveSession::new_for_tests(SaveKind::InMemory, level);

    let mut player_save_data = Properties::default();
    let mut pal_box_id_struct = Properties::default();
    pal_box_id_struct.insert("ID", guid_property(Uuid::new_v4()));
    player_save_data.insert(
        "PalStorageContainerId",
        Property::Struct(StructValue::Struct(pal_box_id_struct)),
    );
    let mut player_root_properties = Properties::default();
    player_root_properties.insert(
        "SaveData",
        Property::Struct(StructValue::Struct(player_save_data)),
    );
    let player_sav = minimal_save(player_root_properties);

    session.loaded_players.insert(
        player_id,
        LoadedPlayer {
            uid: player_id,
            sav: player_sav,
            dps: Some(dps_sav),
        },
    );

    (session, data, player_id)
}

#[test]
fn add_player_dps_pal_writes_a_flat_default_full_stomach_for_a_never_used_slot() {
    let (mut session, data, player_id) = dps_fixture_for_stomach();

    let (slot_index, new_pal) = pal::add_player_dps_pal(
        &mut session,
        &data,
        player_id,
        "Sheepball",
        "Combat",
        Some(0),
    )
    .unwrap()
    .expect("slot 0 explicitly requested");
    assert_eq!(slot_index, 0);
    assert_eq!(
        new_pal.stomach, 300.0,
        "_set_max_stomach() (pal.py) falls back to the flat 300.0 default \
         when the slot's PREVIOUS (pre-reset) CharacterID -- \"None\" here \
         -- has no pals.json entry"
    );
}

/// Proves both halves at once: the stale 999.0 already sitting in the slot
/// must be overwritten (never inherited), and the value written must be
/// species-aware off the slot's PREVIOUS occupant ("Alpaca", 225.0) -- NOT
/// the newly-requested species ("Sheepball", which would be 150.0 if this
/// port mistakenly used the new species instead).
#[test]
fn add_player_dps_pal_into_a_recycled_slot_overwrites_stale_full_stomach_using_the_previous_occupants_species(
) {
    let (mut session, data, player_id) = dps_fixture_for_stomach();

    let (slot_index, new_pal) = pal::add_player_dps_pal(
        &mut session,
        &data,
        player_id,
        "Sheepball", // the NEW species being requested
        "Combat",
        Some(1), // recycled from "Alpaca"
    )
    .unwrap()
    .expect("slot 1 explicitly requested");
    assert_eq!(slot_index, 1);
    assert_eq!(
        new_pal.stomach, 225.0,
        "_set_max_stomach() (pal.py) runs during Pal.__init__, BEFORE reset()/ \
         character_id reassignment -- it keys off the slot's PREVIOUS \
         occupant (\"Alpaca\", max_full_stomach 225.0 per data/json/pals.json), \
         never the stale 999.0 already in the slot and never the newly- \
         requested \"Sheepball\" -- see this task's report"
    );
}

#[test]
fn delete_player_dps_pals_resets_the_slot_and_clears_the_outer_instance_id() {
    let (mut session, data, player_id) = dps_fixture();
    let loaded = session.loaded_players.get(&player_id).unwrap();
    let dps_save = loaded.dps.as_ref().unwrap();
    let slots_before = psp_core::props::struct_values(
        dps_save
            .root
            .properties
            .0
            .get(&uesave::PropertyKey::from("SaveParameterArray"))
            .unwrap(),
    )
    .unwrap();
    let lucky_dto_before = pal::pal_dto_from_dps_slot(&slots_before[1], &data).unwrap();
    assert_eq!(lucky_dto_before.character_id, "Sheepball");

    pal::delete_player_dps_pals(&mut session, &data, player_id, &[1]).unwrap();

    let loaded = session.loaded_players.get(&player_id).unwrap();
    let dps_save = loaded.dps.as_ref().unwrap();
    let slots_after = psp_core::props::struct_values(
        dps_save
            .root
            .properties
            .0
            .get(&uesave::PropertyKey::from("SaveParameterArray"))
            .unwrap(),
    )
    .unwrap();
    let StructValue::Struct(slot_props) = &slots_after[1] else {
        panic!("slot 1 must still be a struct");
    };
    let save_parameter = psp_core::props::struct_props(
        slot_props
            .0
            .get(&uesave::PropertyKey::from("SaveParameter"))
            .unwrap(),
    )
    .unwrap();
    assert_eq!(
        psp_core::props::get(save_parameter, &["CharacterID"]).and_then(psp_core::props::as_str),
        Some("None"),
        "reset() must clear CharacterID back to \"None\""
    );
    let outer_instance_id = psp_core::props::get(slot_props, &["InstanceId", "InstanceId"])
        .and_then(psp_core::props::as_uuid)
        .unwrap();
    assert_eq!(
        outer_instance_id,
        psp_core::props::EMPTY_UUID,
        "the outer slot InstanceId.InstanceId must also be cleared to EMPTY \
         (Pal.reset()'s `self.instance_id = PalObjects.EMPTY_UUID`, missed \
         by the brief's own reference code -- see this task's report)"
    );
}

/// Real-save proof that all three DPS ops gracefully no-op / return `None`
/// when the player has no `_dps.sav` at all (world1's real, committed
/// state for every player) -- not a crash, matching this port's
/// never-panic-on-untrusted-input policy.
#[test]
fn dps_ops_gracefully_return_none_when_the_player_has_no_dps_file() {
    let mut session = common::load_fixture_session("world1");
    let data = game_data();
    let player_id = loaded_session(&mut session, &data);
    assert!(
        session
            .loaded_players
            .get(&player_id)
            .unwrap()
            .dps
            .is_none(),
        "test precondition: world1 has no _dps.sav for this player"
    );

    assert!(
        pal::add_player_dps_pal(&mut session, &data, player_id, "Sheepball", "x", None)
            .unwrap()
            .is_none()
    );
    let some_dto = {
        let details = player::build_player_dto(&session, &data, player_id)
            .unwrap()
            .unwrap();
        let first: Option<PalDto> = details.pals.iter().next().map(|(_, dto)| dto.clone());
        first
    };
    if let Some(dto) = some_dto {
        assert!(pal::clone_dps_pal(&mut session, &data, &dto)
            .unwrap()
            .is_none());
    }
    // Must not panic even though there is nothing to reset.
    pal::delete_player_dps_pals(&mut session, &data, player_id, &[0]).unwrap();
}

// ============================================================================
// Corpus-gated (optional `PSP_TEST_SAVE_DIR`) coverage -- also keeps
// `common::load_corpus_session` from going unused in this binary, matching
// this workspace's established convention (`pal_write.rs`'s own final test).
// ============================================================================

#[test]
fn add_and_delete_player_pal_round_trips_across_the_whole_corpus() {
    let Some(mut session) = common::load_corpus_session() else {
        return;
    };
    let data = game_data();
    let Some(&player_id) = session.player_summaries.keys().next() else {
        return;
    };
    let Some(_) =
        player::get_player_details(&mut session, &data, player_id, &null_progress()).unwrap()
    else {
        return;
    };
    let details = player::build_player_dto(&session, &data, player_id)
        .unwrap()
        .unwrap();
    let Some(pal_box_id) = details.pal_box_id else {
        return;
    };
    let entry_count_before = world::character_map(&session.level).unwrap().len();

    let Some(new_pal) = pal::add_player_pal(
        &mut session,
        &data,
        player_id,
        "Sheepball",
        "corpus",
        pal_box_id,
        None,
    )
    .unwrap() else {
        return; // pal box full in this corpus save -- nothing to prove
    };
    assert_eq!(
        world::character_map(&session.level).unwrap().len(),
        entry_count_before + 1
    );

    pal::delete_player_pals(&mut session, player_id, &[new_pal.instance_id]).unwrap();
    assert_eq!(
        world::character_map(&session.level).unwrap().len(),
        entry_count_before
    );
}
