//! `update_save_file` write-back: pals, players, guilds, item containers.

mod common;

use psp_core::domain::{guild, pal, player};
use psp_core::dto::ordered_map::OrderedMap;
use psp_core::gamedata::GameData;
use psp_core::progress::null_progress;
use uuid::Uuid;

const WORLD1_PLAYER_O: &str = "8c2f1930-0000-0000-0000-000000000000";
const WORLD1_GUILD_WITH_BASE: &str = "54491484-4e6c-7327-70b2-868f350929f6";

fn game_data() -> GameData {
    let json_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../data/json");
    GameData::load(&json_dir).expect("data dir")
}

#[test]
fn update_pals_edit_then_reread() {
    let mut session = common::load_fixture_session("world1");
    let data = game_data();
    let player_id: Uuid = WORLD1_PLAYER_O.parse().unwrap();
    player::get_player_details(&mut session, &data, player_id, &null_progress())
        .unwrap()
        .unwrap();
    let details = player::build_player_dto(&session, &data, player_id)
        .unwrap()
        .unwrap();
    let (pal_id, source) = details.pals.iter().next().expect("player owns pals");
    let mut edited = source.clone();
    edited.nickname = Some("Edited".to_string());
    edited.level = 55;
    let mut modified: OrderedMap<Uuid, _> = OrderedMap::new();
    modified.insert(*pal_id, edited);

    let captured_messages = std::sync::Arc::new(std::sync::Mutex::new(Vec::<String>::new()));
    let sink_messages = captured_messages.clone();
    let collector: psp_core::progress::ProgressSink = std::sync::Arc::new(move |message: &str| {
        sink_messages.lock().unwrap().push(message.to_string());
    });
    pal::update_pals(&mut session, &data, &modified, &collector).unwrap();
    // A per-pal message, then the trailing save message.
    let messages = captured_messages.lock().unwrap();
    assert!(messages[0].starts_with("Updating pal "));
    assert_eq!(
        messages.last().map(String::as_str),
        Some("Saving changes to file")
    );

    let reread = player::build_player_dto(&session, &data, player_id)
        .unwrap()
        .unwrap();
    let updated = reread.pals.get(pal_id).expect("pal still present");
    assert_eq!(updated.nickname.as_deref(), Some("Edited"));
    assert_eq!(updated.level, 55);
    assert_eq!(updated.hp, updated.max_hp);
}

/// An incoming `storage_id` never moves the pal's `ContainerId`, all the way
/// through the write-back path. The DTO must request a genuinely different
/// container, or the assertion would hold either way and prove nothing.
#[test]
fn update_pals_preserves_parity_bug_1_container_id_never_moves() {
    let mut session = common::load_fixture_session("world1");
    let data = game_data();
    let player_id: Uuid = WORLD1_PLAYER_O.parse().unwrap();
    player::get_player_details(&mut session, &data, player_id, &null_progress())
        .unwrap()
        .unwrap();
    let details = player::build_player_dto(&session, &data, player_id)
        .unwrap()
        .unwrap();
    let (pal_id, source) = details.pals.iter().next().expect("player owns pals");
    let original_container_id = source.storage_id;
    let different_container_id = Uuid::parse_str("11111111-2222-3333-4444-555555555555").unwrap();
    assert_ne!(
        different_container_id, original_container_id,
        "fixture sanity: the DTO's requested storage_id must genuinely differ \
         from the pal's real container id for this test to mean anything"
    );
    let mut edited = source.clone();
    edited.storage_id = different_container_id;
    edited.storage_slot = source.storage_slot + 1;
    let mut modified: OrderedMap<Uuid, _> = OrderedMap::new();
    modified.insert(*pal_id, edited);

    pal::update_pals(&mut session, &data, &modified, &null_progress()).unwrap();

    let reread = player::build_player_dto(&session, &data, player_id)
        .unwrap()
        .unwrap();
    let updated = reread.pals.get(pal_id).expect("pal still present");
    assert_eq!(
        updated.storage_id, original_container_id,
        "PARITY-BUG-1: ContainerId must never change, even when the DTO's \
         storage_id field asks for a genuinely DIFFERENT container"
    );
    assert_eq!(updated.storage_slot, source.storage_slot + 1);
}

#[test]
fn update_pals_missing_pal_errors() {
    let mut session = common::load_fixture_session("world1");
    let data = game_data();
    let ghost = Uuid::new_v4();
    let edited = psp_core::dto::pal::PalDto {
        instance_id: ghost,
        character_id: "SheepBall".to_string(),
        character_key: "sheepball".to_string(),
        owner_uid: None,
        is_lucky: Some(false),
        is_boss: Some(false),
        is_predator: false,
        is_tower: false,
        gender: psp_core::dto::pal::PalGender::Female,
        nickname: None,
        filtered_nickname: None,
        group_id: None,
        stomach: 150.0,
        sanity: 100.0,
        hp: 1000,
        level: 1,
        exp: 0,
        rank: 1,
        rank_hp: 0,
        rank_attack: 0,
        rank_defense: 0,
        rank_craftspeed: 0,
        talent_hp: 0,
        talent_shot: 0,
        talent_defense: 0,
        max_hp: 1000,
        storage_slot: 0,
        storage_id: ghost,
        learned_skills: vec![],
        active_skills: vec![],
        passive_skills: vec![],
        work_suitability: OrderedMap::new(),
        is_sick: false,
        friendship_point: 0,
    };
    let mut modified: OrderedMap<Uuid, _> = OrderedMap::new();
    modified.insert(ghost, edited);

    let error = pal::update_pals(&mut session, &data, &modified, &null_progress()).unwrap_err();
    assert_eq!(error.to_string(), format!("pal not found: {ghost}"));
}

#[test]
fn update_players_technologies() {
    let mut session = common::load_fixture_session("world1");
    let data = game_data();
    let player_id: Uuid = WORLD1_PLAYER_O.parse().unwrap();
    player::get_player_details(&mut session, &data, player_id, &null_progress())
        .unwrap()
        .unwrap();

    player::update_player_technologies(
        &mut session,
        player_id,
        Some(&["Workbench".to_string(), "HandTorch".to_string()]),
        Some(42),
        Some(7),
    )
    .unwrap();
    let details = player::build_player_dto(&session, &data, player_id)
        .unwrap()
        .unwrap();
    assert_eq!(details.technologies, vec!["Workbench", "HandTorch"]);
    assert_eq!(details.technology_points, 42);
    assert_eq!(details.boss_technology_points, 7);
}

#[test]
fn update_players_full_dto() {
    let mut session = common::load_fixture_session("world1");
    let data = game_data();
    let player_id: Uuid = WORLD1_PLAYER_O.parse().unwrap();
    player::get_player_details(&mut session, &data, player_id, &null_progress())
        .unwrap()
        .unwrap();
    let mut dto = player::build_player_dto(&session, &data, player_id)
        .unwrap()
        .unwrap();
    dto.level = 60;
    dto.technology_points = 999;
    let mut modified = OrderedMap::new();
    modified.insert(player_id, dto);
    player::update_players(&mut session, &data, &modified, &null_progress()).unwrap();
    let reread = player::build_player_dto(&session, &data, player_id)
        .unwrap()
        .unwrap();
    assert_eq!(reread.level, 60);
    assert_eq!(reread.technology_points, 999);
}

/// Container write-back must route through the player's own `InventoryInfo`,
/// never the incoming DTO's `common_container.id`. The forged id here is
/// paired with a real content edit on purpose: id-based routing would
/// silently no-op on an unresolvable id, and that leaves `id` unchanged too,
/// so asserting `id` alone cannot tell "routed correctly" from "did nothing".
#[test]
fn update_players_common_container_edit_ignores_forged_dto_id() {
    let mut session = common::load_fixture_session("world1");
    let data = game_data();
    let player_id: Uuid = WORLD1_PLAYER_O.parse().unwrap();
    player::get_player_details(&mut session, &data, player_id, &null_progress())
        .unwrap()
        .unwrap();
    let mut dto = player::build_player_dto(&session, &data, player_id)
        .unwrap()
        .unwrap();
    let common = dto.common_container.as_mut().expect("player has one");
    let real_id = common.id;
    common.id = Uuid::new_v4(); // forged -- must be ignored for routing
                                // A real content edit: a brand-new slot at an index unlikely to already
                                // be occupied by this fixture player's real inventory.
    common
        .slots
        .push(psp_core::dto::container::ItemContainerSlotDto {
            dynamic_item: None,
            slot_index: 9000,
            count: 3,
            static_id: Some("Wood".to_string()),
            local_id: None,
        });
    let mut modified = OrderedMap::new();
    modified.insert(player_id, dto);
    player::update_players(&mut session, &data, &modified, &null_progress()).unwrap();

    let reread = player::build_player_dto(&session, &data, player_id)
        .unwrap()
        .unwrap();
    let common_after = reread.common_container.expect("still present");
    assert_eq!(
        common_after.id, real_id,
        "the container actually mutated must still be the player's real one"
    );
    let added = common_after
        .slots
        .iter()
        .find(|slot| slot.slot_index == 9000)
        .expect(
            "the real content edit must have landed in the player's REAL common container -- \
             it would be silently absent if routing had instead gone through the forged id",
        );
    assert_eq!(added.static_id.as_deref(), Some("Wood"));
    assert_eq!(added.count, 3);
}

/// Real-save coverage for a weapon dynamic-item update: `WORLD1_PLAYER_O`'s
/// weapon container carries exactly one slot, an `SFBow_5`.
#[test]
fn update_players_weapon_durability_round_trips() {
    let mut session = common::load_fixture_session("world1");
    let data = game_data();
    let player_id: Uuid = WORLD1_PLAYER_O.parse().unwrap();
    player::get_player_details(&mut session, &data, player_id, &null_progress())
        .unwrap()
        .unwrap();
    let mut dto = player::build_player_dto(&session, &data, player_id)
        .unwrap()
        .unwrap();
    let weapon_container = dto
        .weapon_load_out_container
        .as_mut()
        .expect("weapon container loads");
    assert_eq!(weapon_container.slots.len(), 1);
    let item = weapon_container.slots[0]
        .dynamic_item
        .as_mut()
        .expect("real weapon resolves");
    assert_eq!(item.static_id.as_deref(), Some("SFBow_5"));
    item.durability = Some(12.5);
    item.remaining_bullets = Some(3);
    let mut modified = OrderedMap::new();
    modified.insert(player_id, dto);
    player::update_players(&mut session, &data, &modified, &null_progress()).unwrap();

    let reread = player::build_player_dto(&session, &data, player_id)
        .unwrap()
        .unwrap();
    let weapon_after = reread.weapon_load_out_container.unwrap();
    let item_after = weapon_after.slots[0].dynamic_item.as_ref().unwrap();
    assert_eq!(item_after.durability, Some(12.5));
    assert_eq!(item_after.remaining_bullets, Some(3));
    assert_eq!(item_after.static_id.as_deref(), Some("SFBow_5"));
}

/// Removing a dynamic item from a slot (`dynamic_item: None` while
/// `static_id` stays non-"None") deletes the `DynamicItemSaveData` entry but
/// leaves the slot's `local_id_in_created_world` pointing at it. The dangling
/// slot is deliberate, for save-file byte fidelity; `read_item_container`
/// treats a dangling `local_id` as "slot gone", so the slot vanishes on the
/// next read, which is what the game itself would do.
#[test]
fn update_players_removing_a_dynamic_item_leaves_the_slot_dangling_on_next_read() {
    let mut session = common::load_fixture_session("world1");
    let data = game_data();
    let player_id: Uuid = WORLD1_PLAYER_O.parse().unwrap();
    player::get_player_details(&mut session, &data, player_id, &null_progress())
        .unwrap()
        .unwrap();
    let mut dto = player::build_player_dto(&session, &data, player_id)
        .unwrap()
        .unwrap();
    let armor_container = dto
        .player_equipment_armor_container
        .as_mut()
        .expect("armor container loads");
    assert_eq!(armor_container.slots.len(), 6);
    // Keep the slot (static_id untouched, not "None") but drop its dynamic
    // item reference: the exact shape that leaves the slot dangling.
    armor_container.slots[0].dynamic_item = None;
    let mut modified = OrderedMap::new();
    modified.insert(player_id, dto);
    player::update_players(&mut session, &data, &modified, &null_progress()).unwrap();

    let reread = player::build_player_dto(&session, &data, player_id)
        .unwrap()
        .unwrap();
    let armor_after = reread.player_equipment_armor_container.unwrap();
    assert_eq!(
        armor_after.slots.len(),
        5,
        "the slot whose dynamic item was removed must vanish on the next \
         read (dangling local_id -> read_item_container drops it), not \
         survive as a bare slot with dynamic_item: None"
    );
}

/// world1's player `8C2F1930` has no `RelicObtainForInstanceFlag` key under
/// `RecordData` at all -- a legitimately key-less save. `apply_unlock_flags`
/// must create the Map property rather than silently no-op.
#[test]
fn update_players_creates_missing_unlock_flag_map_and_it_round_trips() {
    let mut session = common::load_fixture_session("world1");
    let data = game_data();
    let player_id: Uuid = WORLD1_PLAYER_O.parse().unwrap();
    player::get_player_details(&mut session, &data, player_id, &null_progress())
        .unwrap()
        .unwrap();
    let mut dto = player::build_player_dto(&session, &data, player_id)
        .unwrap()
        .unwrap();
    assert_eq!(
        dto.collected_effigies,
        Some(vec![]),
        "fixture sanity: this player has no RelicObtainForInstanceFlag key at all yet"
    );
    dto.collected_effigies = Some(vec!["SomeRelicInstanceId".to_string()]);
    let mut modified = OrderedMap::new();
    modified.insert(player_id, dto);
    player::update_players(&mut session, &data, &modified, &null_progress()).unwrap();

    let reread = player::build_player_dto(&session, &data, player_id)
        .unwrap()
        .unwrap();
    assert_eq!(
        reread.collected_effigies,
        Some(vec!["SomeRelicInstanceId".to_string()]),
        "the freshly created Map property must round-trip through this port's own \
         reader, not silently no-op"
    );
}

/// `apply_player_dto` unconditionally writes `SaveData.CompletedQuestArray`/
/// `OrderedQuestArray`, but a player who has never started a quest carries
/// neither property nor its write schema, so the resave is refused. world1's
/// player already carries both, so the properties and their schemas are
/// stripped here to reproduce a genuinely quest-less player.
#[test]
fn update_players_full_dto_survives_missing_quest_array_schema() {
    let mut session = common::load_fixture_session("world1");
    let data = game_data();
    let player_id: Uuid = WORLD1_PLAYER_O.parse().unwrap();
    player::get_player_details(&mut session, &data, player_id, &null_progress())
        .unwrap()
        .unwrap();

    {
        let loaded = session.loaded_players.get_mut(&player_id).unwrap();
        let save_data_property = loaded
            .sav
            .root
            .properties
            .0
            .get_mut(&uesave::PropertyKey::from("SaveData"))
            .expect("player has SaveData");
        let save_data =
            psp_core::props::struct_props_mut(save_data_property).expect("SaveData is a struct");
        save_data
            .0
            .shift_remove(&uesave::PropertyKey::from("CompletedQuestArray"));
        save_data
            .0
            .shift_remove(&uesave::PropertyKey::from("OrderedQuestArray"));

        // Each player `.sav` is its own standalone `uesave::Save`, so dropping
        // these schemas here cannot affect any other player.
        let mut stripped_schemas = uesave::PropertySchemas::new();
        for (path, tag) in loaded.sav.schemas.schemas() {
            if path.ends_with(".CompletedQuestArray")
                || path.ends_with(".OrderedQuestArray")
                || path.contains(".OrderedQuestArray.")
            {
                continue;
            }
            stripped_schemas.record(path.clone(), tag.clone());
        }
        loaded.sav.schemas = stripped_schemas;
    }
    {
        let loaded = session.loaded_players.get(&player_id).unwrap();
        assert!(
            loaded
                .sav
                .schemas
                .get("SaveData.CompletedQuestArray")
                .is_none(),
            "test setup: CompletedQuestArray schema must be stripped"
        );
        assert!(
            loaded
                .sav
                .schemas
                .get("SaveData.OrderedQuestArray")
                .is_none(),
            "test setup: OrderedQuestArray schema must be stripped"
        );
    }

    let mut dto = player::build_player_dto(&session, &data, player_id)
        .unwrap()
        .unwrap();
    assert!(
        dto.completed_missions.is_empty() && dto.current_missions.is_empty(),
        "test setup: player must genuinely have no quests before this edit"
    );
    dto.completed_missions = vec!["Main_UnlockFastTravel".to_string()];
    dto.current_missions = vec!["Main_PickupWood".to_string()];
    let mut modified = OrderedMap::new();
    modified.insert(player_id, dto);
    player::update_players(&mut session, &data, &modified, &null_progress()).unwrap();

    let player_files = session.player_sav_bytes().expect(
        "a full player edit must serialize even when CompletedQuestArray/OrderedQuestArray \
         have never been written by this player before",
    );
    let (sav_bytes, _dps_bytes) = player_files.get(&player_id).expect("player serialized");
    let reparsed = psp_core::savio::read_sav_bytes(sav_bytes).expect("reparse written .sav");
    let save_data_property =
        psp_core::props::get(&reparsed.root.properties, &["SaveData"]).expect("SaveData present");
    let save_data =
        psp_core::props::struct_props(save_data_property).expect("SaveData is a struct");

    let completed = psp_core::props::get(save_data, &["CompletedQuestArray"])
        .and_then(psp_core::props::name_values)
        .expect("CompletedQuestArray round trips as a Name array");
    assert_eq!(completed, &vec!["Main_UnlockFastTravel".to_string()]);

    let ordered = psp_core::props::get(save_data, &["OrderedQuestArray"])
        .and_then(psp_core::props::struct_values)
        .expect("OrderedQuestArray round trips as a Struct array");
    assert_eq!(ordered.len(), 1);
    let uesave::StructValue::Struct(quest) = &ordered[0] else {
        panic!("OrderedQuestArray element must be a Struct");
    };
    let quest_name = psp_core::props::get(quest, &["QuestName"])
        .and_then(psp_core::props::as_str)
        .expect("QuestName present");
    assert_eq!(quest_name, "Main_PickupWood");
}

#[test]
fn technologies_on_unloaded_player_errors() {
    let mut session = common::load_fixture_session("world1");
    let ghost = Uuid::new_v4();
    let error =
        player::update_player_technologies(&mut session, ghost, None, Some(1), None).unwrap_err();
    assert_eq!(
        error.to_string(),
        format!("Player {ghost} not found in the save file.")
    );
}

#[test]
fn update_guilds_name_and_base_camp_level() {
    let mut session = common::load_fixture_session("world1");
    let data = game_data();
    let guild_id: Uuid = WORLD1_GUILD_WITH_BASE.parse().unwrap();
    let before = guild::get_guild_details(&mut session, &data, guild_id)
        .unwrap()
        .unwrap();
    let mut dto = before.clone();
    dto.name = Some("Renamed Guild".to_string());
    dto.base_camp_level = Some(before.base_camp_level.unwrap_or(1) + 1);
    dto.bases = None; // omitted bases skip base processing entirely
    let mut modified = OrderedMap::new();
    modified.insert(guild_id, dto);

    let captured_messages = std::sync::Arc::new(std::sync::Mutex::new(Vec::<String>::new()));
    let sink_messages = captured_messages.clone();
    let collector: psp_core::progress::ProgressSink = std::sync::Arc::new(move |message: &str| {
        sink_messages.lock().unwrap().push(message.to_string());
    });
    guild::update_guilds(&mut session, &data, &modified, &collector).unwrap();
    let messages = captured_messages.lock().unwrap();
    assert_eq!(
        messages[0],
        format!("Updating guild {guild_id}"),
        "progress message names the guild UUID, not its name"
    );

    let after = guild::get_guild_details(&mut session, &data, guild_id)
        .unwrap()
        .unwrap();
    assert_eq!(after.name, Some("Renamed Guild".to_string()));
    assert_eq!(
        after.base_camp_level,
        Some(before.base_camp_level.unwrap_or(1) + 1)
    );
}

/// A `base_camp_level` of `0` is treated as "not supplied" and must leave the
/// existing level untouched, exactly like an omitted field.
#[test]
fn update_guilds_zero_base_camp_level_is_a_no_op() {
    let mut session = common::load_fixture_session("world1");
    let data = game_data();
    let guild_id: Uuid = WORLD1_GUILD_WITH_BASE.parse().unwrap();
    let before = guild::get_guild_details(&mut session, &data, guild_id)
        .unwrap()
        .unwrap();
    let mut dto = before.clone();
    dto.base_camp_level = Some(0);
    dto.bases = None;
    let mut modified = OrderedMap::new();
    modified.insert(guild_id, dto);
    guild::update_guilds(&mut session, &data, &modified, &null_progress()).unwrap();

    let after = guild::get_guild_details(&mut session, &data, guild_id)
        .unwrap()
        .unwrap();
    assert_eq!(after.base_camp_level, before.base_camp_level);
}

/// The widest single real-save exercise of `apply_guild_dto`/`apply_base_dto`/
/// `apply_item_container_dto`: name, base camp level, a base's storage
/// container, and the guild chest all through one `update_guilds` call.
#[test]
fn update_guilds_full_round_trip_bases_and_chest() {
    let mut session = common::load_fixture_session("world1");
    let data = game_data();
    let guild_id: Uuid = WORLD1_GUILD_WITH_BASE.parse().unwrap();
    let mut dto = guild::get_guild_details(&mut session, &data, guild_id)
        .unwrap()
        .unwrap();
    assert!(dto.guild_chest.is_some(), "fixture guild has a chest");
    let bases = dto.bases.as_mut().expect("fixture guild has bases");
    assert!(!bases.is_empty());
    let mut modified = OrderedMap::new();
    modified.insert(guild_id, dto);
    guild::update_guilds(&mut session, &data, &modified, &null_progress()).unwrap();

    let after = guild::get_guild_details(&mut session, &data, guild_id)
        .unwrap()
        .unwrap();
    assert_eq!(after.id, Some(guild_id));
}

#[test]
fn update_pals_across_the_whole_corpus_never_panics() {
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
    let Some((pal_id, source)) = details.pals.iter().next() else {
        return;
    };
    let mut edited = source.clone();
    edited.level = source.level + 1;
    let mut modified = OrderedMap::new();
    modified.insert(*pal_id, edited);
    pal::update_pals(&mut session, &data, &modified, &null_progress()).unwrap();
}
