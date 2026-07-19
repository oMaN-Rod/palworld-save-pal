//! `update_save_file` write-back: pals, players, guilds, item containers.

mod common;

use std::collections::{BTreeMap, BTreeSet};

use psp_core::domain::{guild, pal, player};
use psp_core::dto::ordered_map::OrderedMap;
use psp_core::gamedata::GameData;
use psp_core::progress::null_progress;
use uuid::Uuid;

const WORLD1_PLAYER_O: &str = "8c2f1930-0000-0000-0000-000000000000";
const WORLD1_GUILD_WITH_BASE: &str = "54491484-4e6c-7327-70b2-868f350929f6";
/// `v1_relics` "zBlasters": possess map holds CapturePower alone.
const V1_PLAYER_CAPTURE_POWER_ONLY: &str = "62b176f8-0000-0000-0000-000000000000";
/// `v1_relics` "espat": 12 relic types, most at 0 unspent, ranks bought in nearly all --
/// the real save proving a 0-valued key is a normal state.
const V1_PLAYER_MANY_RELIC_RANKS: &str = "e1530496-0000-0000-0000-000000000000";

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
fn update_pals_keeps_container_id_stable_on_move() {
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
        "save-file fidelity: ContainerId must never change, even when the DTO's \
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
            .get_mut(&psp_core::ue::PropertyKey::from("SaveData"))
            .expect("player has SaveData");
        let save_data =
            psp_core::props::struct_props_mut(save_data_property).expect("SaveData is a struct");
        save_data
            .0
            .shift_remove(&psp_core::ue::PropertyKey::from("CompletedQuestArray"));
        save_data
            .0
            .shift_remove(&psp_core::ue::PropertyKey::from("OrderedQuestArray"));

        // Each player `.sav` is its own standalone `psp_core::ue::Save`, so dropping
        // these schemas here cannot affect any other player.
        let mut stripped_schemas = psp_core::ue::PropertySchemas::new();
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
    let psp_core::ue::StructValue::Struct(quest) = &ordered[0] else {
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
    let mut session = common::load_corpus_session();
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

/// `RelicPossessNum` counts *unspent effigy relics*. It must change only when
/// effigies are newly collected: never for fast-travel unlocks, and never on an
/// unchanged resave.
#[test]
fn relic_possess_num_only_counts_newly_collected_effigies() {
    let mut session = common::load_fixture_session("world1");
    let data = game_data();
    let player_id: Uuid = WORLD1_PLAYER_O.parse().unwrap();
    player::get_player_details(&mut session, &data, player_id, &null_progress())
        .unwrap()
        .unwrap();

    let base = player::build_player_dto(&session, &data, player_id)
        .unwrap()
        .unwrap();
    let start = base.effigy_possess_num;

    // Fast-travel unlocks alone must not touch the relic counter.
    let mut dto = base.clone();
    dto.unlocked_fast_travel_points = Some(vec!["FT_A".into(), "FT_B".into(), "FT_C".into()]);
    dto.collected_effigies = Some(vec![]);
    let mut m = OrderedMap::new();
    m.insert(player_id, dto);
    player::update_players(&mut session, &data, &m, &null_progress()).unwrap();
    let after_ft = player::build_player_dto(&session, &data, player_id)
        .unwrap()
        .unwrap();
    assert_eq!(
        after_ft.effigy_possess_num, start,
        "unlocking fast-travel points must not change RelicPossessNum"
    );

    // Collecting 2 new effigies grants exactly 2.
    let mut dto = after_ft.clone();
    dto.collected_effigies = Some(vec!["EF_1".into(), "EF_2".into()]);
    let mut m = OrderedMap::new();
    m.insert(player_id, dto);
    player::update_players(&mut session, &data, &m, &null_progress()).unwrap();
    let after_two = player::build_player_dto(&session, &data, player_id)
        .unwrap()
        .unwrap();
    assert_eq!(
        after_two.effigy_possess_num,
        start + 2,
        "collecting 2 new effigies must grant exactly 2 relics"
    );

    // An unchanged resave must be a no-op.
    let mut m = OrderedMap::new();
    m.insert(player_id, after_two.clone());
    player::update_players(&mut session, &data, &m, &null_progress()).unwrap();
    let after_resave = player::build_player_dto(&session, &data, player_id)
        .unwrap()
        .unwrap();
    assert_eq!(
        after_resave.effigy_possess_num, after_two.effigy_possess_num,
        "an unchanged resave must not change RelicPossessNum"
    );

    // Collecting 1 more (keeping the existing 2) grants exactly 1.
    let mut dto = after_resave.clone();
    dto.collected_effigies = Some(vec!["EF_1".into(), "EF_2".into(), "EF_3".into()]);
    let mut m = OrderedMap::new();
    m.insert(player_id, dto);
    player::update_players(&mut session, &data, &m, &null_progress()).unwrap();
    let after_third = player::build_player_dto(&session, &data, player_id)
        .unwrap()
        .unwrap();
    assert_eq!(
        after_third.effigy_possess_num,
        start + 3,
        "one newly collected effigy must grant exactly one relic"
    );
}

/// After an effigy unlock on a 1.0 save, the legacy fields and the 1.0 by-type
/// structures must still agree. The game's fixup already ran
/// (`bCaptureCompletionRelicFixupDone` is `true` in every real save), so it will
/// never reconcile them for us.
#[test]
fn effigy_unlock_keeps_1_0_relic_structures_consistent() {
    let mut session = common::load_fixture_session("v1_relics");
    let data = game_data();
    // A player with a second, non-CapturePower relic type: without one, the
    // "other types are untouched" assertions below would be vacuous.
    let player_id = common::first_player_with_non_capture_power_relics(&mut session, &data);

    // Snapshot every non-CapturePower type's flag set *before* the write. Comparing the
    // exp index against the by-type flags the code under test just wrote can only prove
    // they agree with each other -- a regression that wiped the other types' flags would
    // move both sides together and still pass. These are the fixed reference points.
    let before_by_type = common::relic_by_type_flags(&common::player_sav_json(&session, player_id));
    let other_types_before: BTreeMap<String, BTreeSet<String>> = before_by_type
        .iter()
        .filter(|(ty, flags)| ty.as_str() != common::CAPTURE_POWER_RELIC && !flags.is_empty())
        .map(|(ty, flags)| (ty.clone(), flags.clone()))
        .collect();
    assert!(
        !other_types_before.is_empty(),
        "fixture sanity: this player must carry a non-CapturePower relic type with flags"
    );
    for (ty, flags) in &other_types_before {
        println!("non-CapturePower relic type {ty}: {} flag(s)", flags.len());
    }

    let mut dto = player::build_player_dto(&session, &data, player_id)
        .unwrap()
        .unwrap();

    // Collect one more effigy than the player currently has.
    let mut effigies = dto.collected_effigies.clone().unwrap_or_default();
    assert!(
        !effigies.is_empty(),
        "fixture sanity: this player must already carry effigies"
    );
    effigies.push("NEWLY_COLLECTED_EFFIGY".to_string());
    dto.collected_effigies = Some(effigies);

    let mut m = OrderedMap::new();
    m.insert(player_id, dto);
    player::update_players(&mut session, &data, &m, &null_progress()).unwrap();

    let sav = common::player_sav_json(&session, player_id);
    let flat = common::relic_flat_flags(&sav);
    let by_type = common::relic_by_type_flags(&sav);
    let possess = common::relic_possess_num(&sav);
    let possess_map = common::relic_possess_num_map(&sav);
    let exp_index = common::relic_bonus_exp_table_index(&sav);

    // (2) flat == the CapturePower by-type flag set
    assert_eq!(
        flat,
        by_type
            .get("EPalRelicType::CapturePower")
            .cloned()
            .unwrap_or_default(),
        "the flat effigy flag map must equal the CapturePower by-type flag set"
    );
    assert!(
        flat.contains("NEWLY_COLLECTED_EFFIGY"),
        "the newly collected effigy must appear in both representations"
    );

    // (3) scalar == map[CapturePower]
    assert_eq!(
        possess,
        possess_map
            .get("EPalRelicType::CapturePower")
            .copied()
            .unwrap_or(0),
        "RelicPossessNum must equal RelicPossessNumMap[CapturePower]"
    );

    // (1) exp table index == total by-type true flags
    let total: usize = by_type.values().map(|s| s.len()).sum();
    assert_eq!(
        exp_index as usize, total,
        "RelicBonusExpTableIndex must equal the total by-type flag count"
    );

    // (4) An effigy unlock grants CapturePower only. Every other relic type's flag set
    // must come through the write non-empty and byte-for-byte unchanged -- asserted
    // against the snapshot taken before the write, not against what the code just wrote.
    for (ty, expected) in &other_types_before {
        let actual = by_type.get(ty);
        assert_eq!(
            actual,
            Some(expected),
            "relic type {ty} must keep its flag set unchanged across an effigy unlock"
        );
        assert!(
            actual.is_some_and(|flags| !flags.is_empty()),
            "relic type {ty} must still carry flags after an effigy unlock"
        );
    }
    // The exp index therefore counts more than just CapturePower.
    let capture_power_flags = by_type
        .get(common::CAPTURE_POWER_RELIC)
        .map(|f| f.len())
        .unwrap_or(0);
    assert!(
        total > capture_power_flags,
        "the exp index must count non-CapturePower flags too, else this test is vacuous"
    );
}

/// The worldmap UI un-collects an effigy on a single click, with no confirmation.
/// A collect/un-collect/re-collect cycle therefore lands the flags exactly back where
/// they started, and `RelicPossessNum` must land back where it started too. Counting
/// only additions inflates it by one per cycle -- the very inflation this counter's
/// handling exists to prevent.
#[test]
fn effigy_toggle_cycle_does_not_inflate_relic_possess_num() {
    let mut session = common::load_fixture_session("world1");
    let data = game_data();
    let player_id: Uuid = WORLD1_PLAYER_O.parse().unwrap();
    player::get_player_details(&mut session, &data, player_id, &null_progress())
        .unwrap()
        .unwrap();

    // Start from a player holding two effigies.
    let mut dto = player::build_player_dto(&session, &data, player_id)
        .unwrap()
        .unwrap();
    dto.collected_effigies = Some(vec!["EF_1".into(), "EF_2".into()]);
    let mut m = OrderedMap::new();
    m.insert(player_id, dto);
    player::update_players(&mut session, &data, &m, &null_progress()).unwrap();
    let collected = player::build_player_dto(&session, &data, player_id)
        .unwrap()
        .unwrap();
    let baseline = collected.effigy_possess_num;

    // Click EF_2's marker: it is spliced out of `collected_effigies`.
    let mut dto = collected.clone();
    dto.collected_effigies = Some(vec!["EF_1".into()]);
    let mut m = OrderedMap::new();
    m.insert(player_id, dto);
    player::update_players(&mut session, &data, &m, &null_progress()).unwrap();

    // Click it again: it is put back. The flags are now identical to `collected`.
    let mut dto = player::build_player_dto(&session, &data, player_id)
        .unwrap()
        .unwrap();
    dto.collected_effigies = Some(vec!["EF_1".into(), "EF_2".into()]);
    let mut m = OrderedMap::new();
    m.insert(player_id, dto);
    player::update_players(&mut session, &data, &m, &null_progress()).unwrap();

    let after = player::build_player_dto(&session, &data, player_id)
        .unwrap()
        .unwrap();
    assert_eq!(
        after.collected_effigies,
        Some(vec!["EF_1".to_string(), "EF_2".to_string()]),
        "fixture sanity: the toggle cycle must leave the flags where they started"
    );
    assert_eq!(
        after.effigy_possess_num, baseline,
        "an off/on effigy toggle cycle must leave RelicPossessNum exactly where it started"
    );
}

/// Un-collecting effigies gives their relics back, one per effigy -- symmetric with
/// the frontend, which already decrements the inventory `Relic` item on removal.
#[test]
fn removing_effigies_lowers_relic_possess_num_by_the_number_removed() {
    let mut session = common::load_fixture_session("world1");
    let data = game_data();
    let player_id: Uuid = WORLD1_PLAYER_O.parse().unwrap();
    player::get_player_details(&mut session, &data, player_id, &null_progress())
        .unwrap()
        .unwrap();

    let base = player::build_player_dto(&session, &data, player_id)
        .unwrap()
        .unwrap();
    let start = base.effigy_possess_num;

    let mut dto = base.clone();
    dto.collected_effigies = Some(vec!["EF_1".into(), "EF_2".into(), "EF_3".into()]);
    let mut m = OrderedMap::new();
    m.insert(player_id, dto);
    player::update_players(&mut session, &data, &m, &null_progress()).unwrap();
    let after_three = player::build_player_dto(&session, &data, player_id)
        .unwrap()
        .unwrap();
    assert_eq!(after_three.effigy_possess_num, start + 3);

    // Remove two of the three.
    let mut dto = after_three.clone();
    dto.collected_effigies = Some(vec!["EF_2".into()]);
    let mut m = OrderedMap::new();
    m.insert(player_id, dto);
    player::update_players(&mut session, &data, &m, &null_progress()).unwrap();
    let after_removal = player::build_player_dto(&session, &data, player_id)
        .unwrap()
        .unwrap();
    assert_eq!(
        after_removal.effigy_possess_num,
        start + 1,
        "removing 2 effigies must lower RelicPossessNum by exactly 2"
    );
}

/// A relic already spent on a rank cannot be un-spent, so a real save holds fewer
/// unspent relics than it has effigy flags. Un-collecting every effigy must floor the
/// counter at 0, never drive it negative.
#[test]
fn relic_possess_num_never_goes_below_zero() {
    let mut session = common::load_fixture_session("v1_relics");
    let data = game_data();
    let player_id = common::first_player_with_non_capture_power_relics(&mut session, &data);

    let base = player::build_player_dto(&session, &data, player_id)
        .unwrap()
        .unwrap();
    let held = base.effigy_possess_num;
    let effigies = base.collected_effigies.clone().unwrap_or_default();
    assert!(
        effigies.len() as i64 > held,
        "fixture sanity: this player must have spent relics (more effigy flags ({}) than \
         unspent relics held ({held})), else flooring is never exercised",
        effigies.len()
    );

    // Un-collect every effigy: more removals than there are relics to give back.
    let mut dto = base.clone();
    dto.collected_effigies = Some(vec![]);
    let mut m = OrderedMap::new();
    m.insert(player_id, dto);
    player::update_players(&mut session, &data, &m, &null_progress()).unwrap();

    let after = player::build_player_dto(&session, &data, player_id)
        .unwrap()
        .unwrap();
    assert_eq!(
        after.effigy_possess_num, 0,
        "removing more effigies than there are unspent relics must floor RelicPossessNum at 0"
    );

    let sav = common::player_sav_json(&session, player_id);
    assert_eq!(
        common::relic_possess_num(&sav),
        0,
        "the written RelicPossessNum must be 0, not negative"
    );
    assert_eq!(
        common::relic_possess_num_map(&sav)
            .get(common::CAPTURE_POWER_RELIC)
            .copied()
            .unwrap_or(0),
        0,
        "RelicPossessNumMap[CapturePower] must mirror the floored scalar"
    );
}

/// A pre-1.0 save has none of the 1.0 relic fields. The new code must not invent
/// them.
#[test]
fn effigy_unlock_does_not_add_1_0_relic_fields_to_a_pre_1_0_save() {
    let mut session = common::load_fixture_session("world1");
    let data = game_data();
    let player_id: Uuid = WORLD1_PLAYER_O.parse().unwrap();
    player::get_player_details(&mut session, &data, player_id, &null_progress())
        .unwrap()
        .unwrap();
    let mut dto = player::build_player_dto(&session, &data, player_id)
        .unwrap()
        .unwrap();
    dto.collected_effigies = Some(vec!["EF_1".into()]);
    let mut m = OrderedMap::new();
    m.insert(player_id, dto);
    player::update_players(&mut session, &data, &m, &null_progress()).unwrap();

    let sav = common::player_sav_json(&session, player_id);
    let text = serde_json::to_string(&sav).unwrap();
    assert!(
        !text.contains("RelicPossessNumMap"),
        "must not add RelicPossessNumMap to a pre-1.0 save"
    );
    assert!(
        !text.contains("RelicObtainForInstanceFlagByType"),
        "must not add RelicObtainForInstanceFlagByType to a pre-1.0 save"
    );
    assert!(
        !text.contains("RelicBonusExpTableIndex"),
        "must not add RelicBonusExpTableIndex to a pre-1.0 save"
    );
}

/// Palworld 1.0's other 11 relic types live ONLY in the by-type structures. Collecting one
/// must move that type's `Flags` and its own `RelicPossessNumMap` entry, and must leave the
/// legacy CapturePower-only mirrors (`RelicObtainForInstanceFlag`, `RelicPossessNum`)
/// completely untouched.
///
/// `StaminaReduction` is deliberately a type this player has NO by-type entry for. The game
/// appends an entry lazily, on first collection of that type, so the write path has to
/// create it -- every fixture player is missing at least one of the 12.
#[test]
fn collecting_a_typed_relic_updates_only_that_type() {
    let mut session = common::load_fixture_session("v1_relics");
    let data = game_data();
    let player_id: Uuid = V1_PLAYER_MANY_RELIC_RANKS.parse().unwrap();
    player::get_player_details(&mut session, &data, player_id, &null_progress())
        .unwrap()
        .unwrap();

    let before = common::player_sav_json(&session, player_id);
    let flat_before = common::relic_flat_flags(&before);
    let possess_before = common::relic_possess_num(&before);
    let by_type_before = common::relic_by_type_flags(&before);
    let possess_map_before = common::relic_possess_num_map(&before);
    assert!(
        !flat_before.is_empty(),
        "fixture sanity: this player must already carry effigies"
    );
    assert!(
        !by_type_before.contains_key("EPalRelicType::StaminaReduction"),
        "fixture sanity: this player must have NO StaminaReduction by-type entry, so that \
         the write path is forced to create one"
    );

    let mut dto = player::build_player_dto(&session, &data, player_id)
        .unwrap()
        .unwrap();
    let mut relics = dto.collected_relics.clone().unwrap_or_default();
    relics.insert(
        "stamina_reduction".to_string(),
        vec!["NEW_STAMINA_RELIC".to_string()],
    );
    dto.collected_relics = Some(relics);
    let mut m = OrderedMap::new();
    m.insert(player_id, dto);
    player::update_players(&mut session, &data, &m, &null_progress()).unwrap();

    let after = common::player_sav_json(&session, player_id);
    let by_type_after = common::relic_by_type_flags(&after);
    let possess_map_after = common::relic_possess_num_map(&after);

    assert_eq!(
        by_type_after.get("EPalRelicType::StaminaReduction"),
        Some(&BTreeSet::from(["NEW_STAMINA_RELIC".to_string()])),
        "the collected relic's guid must land in the StaminaReduction by-type flag set"
    );
    assert_eq!(
        possess_map_after
            .get("EPalRelicType::StaminaReduction")
            .copied(),
        Some(1),
        "RelicPossessNumMap[StaminaReduction] must move by StaminaReduction's OWN net delta"
    );

    // The legacy pair mirrors CapturePower alone, and nothing about CapturePower changed.
    assert_eq!(
        common::relic_flat_flags(&after),
        flat_before,
        "collecting a non-effigy relic must not touch the flat RelicObtainForInstanceFlag map"
    );
    assert_eq!(
        common::relic_possess_num(&after),
        possess_before,
        "collecting a non-effigy relic must not touch the scalar RelicPossessNum"
    );
    assert_eq!(
        possess_map_after.get(common::CAPTURE_POWER_RELIC).copied(),
        possess_map_before
            .get(common::CAPTURE_POWER_RELIC)
            .copied(),
        "RelicPossessNumMap[CapturePower] must still mirror the untouched scalar"
    );

    // Every other type comes through byte-for-byte: a per-type delta must not leak sideways.
    for (ty, flags) in &by_type_before {
        assert_eq!(
            by_type_after.get(ty),
            Some(flags),
            "relic type {ty} must keep its flag set unchanged when another type is collected"
        );
    }
    for (ty, count) in &possess_map_before {
        assert_eq!(
            possess_map_after.get(ty),
            Some(count),
            "RelicPossessNumMap[{ty}] must be unchanged when another type is collected"
        );
    }
}

/// `RelicBonusExpTableIndex` is the total true flag count across ALL by-type entries --
/// not CapturePower's alone. A typed relic write must move it too.
#[test]
fn relic_bonus_exp_table_index_counts_every_type_after_a_typed_relic_write() {
    let mut session = common::load_fixture_session("v1_relics");
    let data = game_data();
    let player_id: Uuid = V1_PLAYER_MANY_RELIC_RANKS.parse().unwrap();
    player::get_player_details(&mut session, &data, player_id, &null_progress())
        .unwrap()
        .unwrap();

    let before = common::player_sav_json(&session, player_id);
    let total_before: usize = common::relic_by_type_flags(&before)
        .values()
        .map(BTreeSet::len)
        .sum();
    let capture_power_before = common::relic_by_type_flags(&before)
        .get(common::CAPTURE_POWER_RELIC)
        .map(BTreeSet::len)
        .unwrap_or(0);
    assert!(
        total_before > capture_power_before,
        "fixture sanity: the player must carry non-CapturePower flags, else 'all types' is vacuous"
    );

    let mut dto = player::build_player_dto(&session, &data, player_id)
        .unwrap()
        .unwrap();
    let mut relics = dto.collected_relics.clone().unwrap_or_default();
    relics.insert(
        "stamina_reduction".to_string(),
        vec!["NEW_STAMINA_RELIC".to_string()],
    );
    dto.collected_relics = Some(relics);
    let mut m = OrderedMap::new();
    m.insert(player_id, dto);
    player::update_players(&mut session, &data, &m, &null_progress()).unwrap();

    let after = common::player_sav_json(&session, player_id);
    let total_after: usize = common::relic_by_type_flags(&after)
        .values()
        .map(BTreeSet::len)
        .sum();
    assert_eq!(
        total_after,
        total_before + 1,
        "the one newly collected relic must appear in the by-type totals"
    );
    assert_eq!(
        common::relic_bonus_exp_table_index(&after) as usize,
        total_after,
        "RelicBonusExpTableIndex must equal the total true flags across ALL relic types"
    );
}

/// A relic already spent on a rank cannot be un-spent, so a type's possess count must floor
/// at 0 rather than go negative -- the same invariant the effigy scalar has. This player
/// holds 0 unspent GliderSpeed relics while carrying GliderSpeed flags, so un-collecting
/// them all asks for a negative count.
#[test]
fn removing_typed_relics_floors_that_types_possess_count_at_zero() {
    let mut session = common::load_fixture_session("v1_relics");
    let data = game_data();
    let player_id: Uuid = V1_PLAYER_MANY_RELIC_RANKS.parse().unwrap();
    player::get_player_details(&mut session, &data, player_id, &null_progress())
        .unwrap()
        .unwrap();

    let before = common::player_sav_json(&session, player_id);
    let glider_flags = common::relic_by_type_flags(&before)
        .get("EPalRelicType::GliderSpeed")
        .cloned()
        .unwrap_or_default();
    let glider_possess = common::relic_possess_num_map(&before)
        .get("EPalRelicType::GliderSpeed")
        .copied()
        .unwrap_or(0);
    assert!(
        glider_flags.len() as i64 > glider_possess,
        "fixture sanity: this player must have spent GliderSpeed relics ({} flags, {glider_possess} \
         unspent), else the floor is never exercised",
        glider_flags.len()
    );

    let mut dto = player::build_player_dto(&session, &data, player_id)
        .unwrap()
        .unwrap();
    let mut relics = dto.collected_relics.clone().unwrap_or_default();
    relics.insert("glider_speed".to_string(), vec![]);
    dto.collected_relics = Some(relics);
    let mut m = OrderedMap::new();
    m.insert(player_id, dto);
    player::update_players(&mut session, &data, &m, &null_progress()).unwrap();

    let after = common::player_sav_json(&session, player_id);
    assert_eq!(
        common::relic_by_type_flags(&after)
            .get("EPalRelicType::GliderSpeed")
            .cloned()
            .unwrap_or_default(),
        BTreeSet::new(),
        "un-collecting every GliderSpeed relic must clear that type's flags"
    );
    assert_eq!(
        common::relic_possess_num_map(&after)
            .get("EPalRelicType::GliderSpeed")
            .copied(),
        Some(0),
        "RelicPossessNumMap[GliderSpeed] must floor at 0, never go negative"
    );
    assert_eq!(
        common::relic_bonus_exp_table_index(&after) as usize,
        common::relic_by_type_flags(&after)
            .values()
            .map(BTreeSet::len)
            .sum::<usize>(),
        "the exp index must still equal the total flags across all types after a removal"
    );
}

/// A pre-1.0 save has no by-type relic structures at all. A DTO carrying typed relics --
/// which the frontend sends on every save -- must not conjure them into existence.
#[test]
fn typed_relic_write_does_not_add_1_0_relic_fields_to_a_pre_1_0_save() {
    let mut session = common::load_fixture_session("world1");
    let data = game_data();
    let player_id: Uuid = WORLD1_PLAYER_O.parse().unwrap();
    player::get_player_details(&mut session, &data, player_id, &null_progress())
        .unwrap()
        .unwrap();
    let mut dto = player::build_player_dto(&session, &data, player_id)
        .unwrap()
        .unwrap();
    dto.collected_effigies = Some(vec!["EF_1".into()]);
    let mut relics: BTreeMap<String, Vec<String>> = BTreeMap::new();
    relics.insert(
        "stamina_reduction".to_string(),
        vec!["NEW_STAMINA_RELIC".to_string()],
    );
    relics.insert("capture_power".to_string(), vec!["EF_1".to_string()]);
    dto.collected_relics = Some(relics);
    let mut m = OrderedMap::new();
    m.insert(player_id, dto);
    player::update_players(&mut session, &data, &m, &null_progress()).unwrap();

    let sav = common::player_sav_json(&session, player_id);
    let text = serde_json::to_string(&sav).unwrap();
    assert!(
        !text.contains("RelicObtainForInstanceFlagByType"),
        "must not add RelicObtainForInstanceFlagByType to a pre-1.0 save"
    );
    assert!(
        !text.contains("RelicPossessNumMap"),
        "must not add RelicPossessNumMap to a pre-1.0 save"
    );
    assert!(
        !text.contains("RelicBonusExpTableIndex"),
        "must not add RelicBonusExpTableIndex to a pre-1.0 save"
    );
    assert!(
        !text.contains("NEW_STAMINA_RELIC"),
        "a typed relic must not leak into a pre-1.0 save's flat effigy flag map"
    );
}

/// An unchanged resave of a 1.0 save must leave every relic structure exactly as it was.
/// The by-type write path rewrites `Flags` for each type on every save, so a bug that
/// reordered, dropped or duplicated a type would show up here first.
#[test]
fn unchanged_resave_of_a_1_0_save_leaves_every_relic_structure_identical() {
    let mut session = common::load_fixture_session("v1_relics");
    let data = game_data();
    let player_id: Uuid = V1_PLAYER_MANY_RELIC_RANKS.parse().unwrap();
    player::get_player_details(&mut session, &data, player_id, &null_progress())
        .unwrap()
        .unwrap();

    let before = common::player_sav_json(&session, player_id);
    let flat_before = common::relic_flat_flags(&before);
    let by_type_before = common::relic_by_type_flags(&before);
    let possess_before = common::relic_possess_num(&before);
    let possess_map_before = common::relic_possess_num_map(&before);
    let exp_index_before = common::relic_bonus_exp_table_index(&before);

    let dto = player::build_player_dto(&session, &data, player_id)
        .unwrap()
        .unwrap();
    let mut m = OrderedMap::new();
    m.insert(player_id, dto);
    player::update_players(&mut session, &data, &m, &null_progress()).unwrap();

    let after = common::player_sav_json(&session, player_id);
    assert_eq!(common::relic_flat_flags(&after), flat_before);
    assert_eq!(common::relic_by_type_flags(&after), by_type_before);
    assert_eq!(common::relic_possess_num(&after), possess_before);
    assert_eq!(common::relic_possess_num_map(&after), possess_map_before);
    assert_eq!(
        common::relic_bonus_exp_table_index(&after),
        exp_index_before
    );
}

/// The set-based comparisons above (`relic_flat_flags`/`relic_by_type_flags` return
/// `BTreeSet`s) would pass even if a write silently sorted or reordered a flag map. Order
/// is deliberately CALLER order, not sorted -- see `relic_flag_write`'s comment -- and this
/// compares the raw on-disk `Vec`s, which an accidental reorder would actually fail.
#[test]
fn unchanged_resave_of_a_1_0_save_preserves_flag_order() {
    let mut session = common::load_fixture_session("v1_relics");
    let data = game_data();
    let player_id: Uuid = V1_PLAYER_MANY_RELIC_RANKS.parse().unwrap();
    player::get_player_details(&mut session, &data, player_id, &null_progress())
        .unwrap()
        .unwrap();

    let before = common::player_sav_json(&session, player_id);
    let flat_before = common::relic_flat_flags_ordered(&before);
    let by_type_before = common::relic_by_type_flags_ordered(&before);
    assert!(
        flat_before.len() > 1,
        "fixture sanity: need at least 2 flags for order to be a meaningful check"
    );
    assert!(
        by_type_before.values().any(|flags| flags.len() > 1),
        "fixture sanity: need at least one by-type entry with 2+ flags"
    );

    let dto = player::build_player_dto(&session, &data, player_id)
        .unwrap()
        .unwrap();
    let mut m = OrderedMap::new();
    m.insert(player_id, dto);
    player::update_players(&mut session, &data, &m, &null_progress()).unwrap();

    let after = common::player_sav_json(&session, player_id);
    assert_eq!(
        common::relic_flat_flags_ordered(&after),
        flat_before,
        "an unchanged resave must preserve RelicObtainForInstanceFlag's on-disk order"
    );
    assert_eq!(
        common::relic_by_type_flags_ordered(&after),
        by_type_before,
        "an unchanged resave must preserve every by-type Flags map's on-disk order"
    );
}

/// `relic_flag_write` dedupes the `Flags` map it builds, not just the delta it reports:
/// `RelicBonusExpTableIndex` counts `Flags` map ENTRIES, so a caller-supplied duplicate
/// guid that survived into the map would inflate the index by one per repeat, even though
/// it represents a single collected relic.
#[test]
fn duplicate_guid_in_collected_relics_does_not_inflate_exp_table_index() {
    let mut session = common::load_fixture_session("v1_relics");
    let data = game_data();
    let player_id: Uuid = V1_PLAYER_MANY_RELIC_RANKS.parse().unwrap();
    player::get_player_details(&mut session, &data, player_id, &null_progress())
        .unwrap()
        .unwrap();

    let before = common::player_sav_json(&session, player_id);
    let total_before: usize = common::relic_by_type_flags(&before)
        .values()
        .map(BTreeSet::len)
        .sum();
    assert!(
        !common::relic_by_type_flags(&before).contains_key("EPalRelicType::StaminaReduction"),
        "fixture sanity: this player must have NO StaminaReduction by-type entry"
    );

    let mut dto = player::build_player_dto(&session, &data, player_id)
        .unwrap()
        .unwrap();
    let mut relics = dto.collected_relics.clone().unwrap_or_default();
    relics.insert(
        "stamina_reduction".to_string(),
        vec![
            "DUPLICATE_STAMINA_RELIC".to_string(),
            "DUPLICATE_STAMINA_RELIC".to_string(),
        ],
    );
    dto.collected_relics = Some(relics);
    let mut m = OrderedMap::new();
    m.insert(player_id, dto);
    player::update_players(&mut session, &data, &m, &null_progress()).unwrap();

    let after = common::player_sav_json(&session, player_id);
    assert_eq!(
        common::relic_by_type_flags(&after)
            .get("EPalRelicType::StaminaReduction")
            .cloned()
            .unwrap_or_default(),
        BTreeSet::from(["DUPLICATE_STAMINA_RELIC".to_string()]),
        "a duplicate guid must collapse to a single Flags entry"
    );
    let total_after: usize = common::relic_by_type_flags(&after)
        .values()
        .map(BTreeSet::len)
        .sum();
    assert_eq!(
        total_after,
        total_before + 1,
        "a duplicate guid must add exactly one flag total, not two"
    );
    assert_eq!(
        common::relic_bonus_exp_table_index(&after) as usize,
        total_after,
        "RelicBonusExpTableIndex must not be inflated by the duplicate guid"
    );
    assert_eq!(
        common::relic_possess_num_map(&after)
            .get("EPalRelicType::StaminaReduction")
            .copied(),
        Some(1),
        "RelicPossessNumMap[StaminaReduction] must move by 1, not 2, for one duplicated guid"
    );
}

/// `RelicObtainForInstanceFlagByType`'s element schemas (`.Type`, `.Flags`) are normally
/// learned by uesave from an existing element at read time. Every real save examined
/// already has at least one entry once the property exists at all, so the append path has
/// never needed its own `ensure_schema`. That is a property of the GAME's writer, though,
/// not something this code can assume -- an array present with zero entries is a
/// structurally valid shape nothing rules out. This test grafts exactly that shape onto a
/// real 1.0 fixture (clearing the array AND stripping the schemas an existing element
/// would have taught uesave) and forces the very first append by collecting a typed relic.
#[test]
fn typed_relic_write_creates_first_entry_in_an_empty_by_type_array() {
    let mut session = common::load_fixture_session("v1_relics");
    let data = game_data();
    let player_id: Uuid = V1_PLAYER_MANY_RELIC_RANKS.parse().unwrap();
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
            .get_mut(&psp_core::ue::PropertyKey::from("SaveData"))
            .expect("player has SaveData");
        let save_data =
            psp_core::props::struct_props_mut(save_data_property).expect("SaveData is a struct");
        let record_data_property = save_data
            .0
            .get_mut(&psp_core::ue::PropertyKey::from("RecordData"))
            .expect("player has RecordData");
        let record_data = psp_core::props::struct_props_mut(record_data_property)
            .expect("RecordData is a struct");
        let by_type_property = record_data
            .0
            .get_mut(&psp_core::ue::PropertyKey::from(
                "RelicObtainForInstanceFlagByType",
            ))
            .expect("fixture sanity: player has RelicObtainForInstanceFlagByType");
        let by_type_values = psp_core::props::struct_values_mut(by_type_property)
            .expect("RelicObtainForInstanceFlagByType is a struct array");
        assert!(
            !by_type_values.is_empty(),
            "fixture sanity: the array must start non-empty, so clearing it is a real edit"
        );
        by_type_values.clear();

        // Strip the `.Type`/`.Flags` schemas an existing element would otherwise have
        // taught uesave, reproducing an array that has never held an entry.
        let mut stripped_schemas = psp_core::ue::PropertySchemas::new();
        for (path, tag) in loaded.sav.schemas.schemas() {
            if path.ends_with(".RelicObtainForInstanceFlagByType.Type")
                || path.ends_with(".RelicObtainForInstanceFlagByType.Flags")
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
            !loaded
                .sav
                .schemas
                .schemas()
                .keys()
                .any(|path| path.ends_with(".RelicObtainForInstanceFlagByType.Type")
                    || path.ends_with(".RelicObtainForInstanceFlagByType.Flags")),
            "test setup: .Type/.Flags schemas must be stripped"
        );
    }

    let mut dto = player::build_player_dto(&session, &data, player_id)
        .unwrap()
        .unwrap();
    assert!(
        dto.collected_relics
            .as_ref()
            .and_then(|r| r.get("stamina_reduction"))
            .is_none_or(Vec::is_empty),
        "fixture sanity: no StaminaReduction relics before the edit, since the array was \
         just cleared"
    );
    let mut relics = dto.collected_relics.clone().unwrap_or_default();
    relics.insert(
        "stamina_reduction".to_string(),
        vec!["NEW_STAMINA_RELIC".to_string()],
    );
    dto.collected_relics = Some(relics);
    let mut m = OrderedMap::new();
    m.insert(player_id, dto);
    player::update_players(&mut session, &data, &m, &null_progress()).unwrap();

    let player_files = session.player_sav_bytes().expect(
        "collecting a relic must serialize even when RelicObtainForInstanceFlagByType starts \
         with zero entries and no learned .Type/.Flags schema",
    );
    let (sav_bytes, _dps_bytes) = player_files.get(&player_id).expect("player serialized");
    let reparsed = psp_core::savio::read_sav_bytes(sav_bytes).expect("reparse written .sav");
    let sav_json = serde_json::to_value(&reparsed).expect("sav to json");
    let by_type_after = common::relic_by_type_flags(&sav_json);
    assert_eq!(
        by_type_after.get("EPalRelicType::StaminaReduction"),
        Some(&BTreeSet::from(["NEW_STAMINA_RELIC".to_string()])),
        "the first-ever append into the empty array must create the StaminaReduction entry"
    );
}

/// Palworld 1.0 renamed both quest arrays to `<Base>_FullRelease`. This 1.0
/// fixture player genuinely carries 19 completed and 13 current quests -- reading
/// the pre-1.0 names finds neither and reports both as empty.
const V1_PLAYER_WITH_QUESTS: &str = "b38a3ab1-0000-0000-0000-000000000000";

/// Every property key anywhere in a serialized `.sav`, with uesave's `_<index>`
/// disambiguating suffix stripped. Matching a bare NAME against the raw JSON text
/// would be a trap: `CompletedQuestArray` is a strict PREFIX of
/// `CompletedQuestArray_FullRelease`, so a substring check cannot tell them apart.
fn property_names(value: &serde_json::Value, out: &mut BTreeSet<String>) {
    match value {
        serde_json::Value::Object(map) => {
            for (key, child) in map {
                if let Some((name, index)) = key.rsplit_once('_') {
                    if !name.is_empty() && index.chars().all(|c| c.is_ascii_digit()) {
                        out.insert(name.to_string());
                    }
                }
                property_names(child, out);
            }
        }
        serde_json::Value::Array(items) => {
            for item in items {
                property_names(item, out);
            }
        }
        _ => {}
    }
}

fn v1_quest_session() -> (psp_core::session::SaveSession, GameData, Uuid) {
    let mut session = common::load_fixture_session("v1_relics");
    let data = game_data();
    let player_id: Uuid = V1_PLAYER_WITH_QUESTS.parse().unwrap();
    player::get_player_details(&mut session, &data, player_id, &null_progress())
        .unwrap()
        .expect("v1_relics carries this player");
    (session, data, player_id)
}

/// The headline bug: a 1.0 save's missions must actually be read.
#[test]
fn v1_save_missions_are_read() {
    let (session, data, player_id) = v1_quest_session();
    let dto = player::build_player_dto(&session, &data, player_id)
        .unwrap()
        .unwrap();
    assert_eq!(
        dto.completed_missions.len(),
        19,
        "the 1.0 fixture player carries 19 completed quests in \
         CompletedQuestArray_FullRelease"
    );
    assert_eq!(
        dto.current_missions.len(),
        13,
        "the 1.0 fixture player carries 13 current quests in \
         OrderedQuestArray_FullRelease"
    );
}

/// Writing a 1.0 player back unchanged must not lose their missions -- which it
/// would, if the write landed on the bare-named property the game never reads.
#[test]
fn v1_save_missions_round_trip_unchanged() {
    let (mut session, data, player_id) = v1_quest_session();
    let dto = player::build_player_dto(&session, &data, player_id)
        .unwrap()
        .unwrap();
    let mut modified = OrderedMap::new();
    modified.insert(player_id, dto);
    player::update_players(&mut session, &data, &modified, &null_progress()).unwrap();

    let reread = player::build_player_dto(&session, &data, player_id)
        .unwrap()
        .unwrap();
    assert_eq!(reread.completed_missions.len(), 19);
    assert_eq!(reread.current_missions.len(), 13);
}

/// An edit to a 1.0 player's missions must persist.
#[test]
fn v1_save_mission_edit_persists() {
    let (mut session, data, player_id) = v1_quest_session();
    let mut dto = player::build_player_dto(&session, &data, player_id)
        .unwrap()
        .unwrap();
    assert!(
        !dto.completed_missions
            .contains(&"PSP_TEST_QUEST".to_string()),
        "fixture sanity: the appended quest must not already be present"
    );
    dto.completed_missions.push("PSP_TEST_QUEST".to_string());
    let mut modified = OrderedMap::new();
    modified.insert(player_id, dto);
    player::update_players(&mut session, &data, &modified, &null_progress()).unwrap();

    let reread = player::build_player_dto(&session, &data, player_id)
        .unwrap()
        .unwrap();
    assert_eq!(reread.completed_missions.len(), 20);
    assert!(
        reread
            .completed_missions
            .contains(&"PSP_TEST_QUEST".to_string()),
        "the appended quest must survive the write"
    );
    assert_eq!(
        reread.current_missions.len(),
        13,
        "the untouched current missions must survive too"
    );
}

/// A 1.0 save must gain no bare-named quest property: writing one invents a
/// property the game never wrote, and leaves the real `_FullRelease` data stale.
#[test]
fn v1_save_gains_no_bare_named_quest_property() {
    let (mut session, data, player_id) = v1_quest_session();
    let dto = player::build_player_dto(&session, &data, player_id)
        .unwrap()
        .unwrap();
    let mut modified = OrderedMap::new();
    modified.insert(player_id, dto);
    player::update_players(&mut session, &data, &modified, &null_progress()).unwrap();

    let sav = common::player_sav_json(&session, player_id);
    let mut names = BTreeSet::new();
    property_names(&sav, &mut names);

    assert!(
        names.contains("CompletedQuestArray_FullRelease"),
        "fixture sanity: the 1.0 save must carry the _FullRelease completed array; \
         found: {names:?}"
    );
    assert!(
        names.contains("OrderedQuestArray_FullRelease"),
        "fixture sanity: the 1.0 save must carry the _FullRelease ordered array"
    );
    assert!(
        !names.contains("CompletedQuestArray"),
        "must not invent a bare CompletedQuestArray on a 1.0 save"
    );
    assert!(
        !names.contains("OrderedQuestArray"),
        "must not invent a bare OrderedQuestArray on a 1.0 save"
    );
}

/// A rank granted for a relic type with no `RelicPossessNumMap` key is invisible in game.
/// In every real 1.0 save, `rank > 0` implies a key. The value is unspent relics and may
/// legitimately be 0, so the key is created at 0 rather than handing out free relics.
#[test]
fn granting_a_relic_rank_creates_its_possess_map_key() {
    let mut session = common::load_fixture_session("v1_relics");
    let data = game_data();
    // A 1.0 player whose map carries CapturePower alone.
    let player_id: Uuid = V1_PLAYER_CAPTURE_POWER_ONLY.parse().unwrap();
    // Only a lazily loaded player has a `.sav` to inspect or write.
    player::get_player_details(&mut session, &data, player_id, &null_progress())
        .unwrap()
        .unwrap();

    let before = common::relic_possess_num_map(&common::player_sav_json(&session, player_id));
    assert_eq!(
        before.keys().collect::<Vec<_>>(),
        vec![common::CAPTURE_POWER_RELIC],
        "fixture sanity: this player's possess map must hold CapturePower alone"
    );
    let capture_power_before = before[common::CAPTURE_POWER_RELIC];

    let mut dto = player::build_player_dto(&session, &data, player_id)
        .unwrap()
        .unwrap();
    dto.status_point_list.insert("swim_speed".to_string(), 5);
    dto.status_point_list.insert("climb_speed".to_string(), 0);

    let mut m = OrderedMap::new();
    m.insert(player_id, dto);
    player::update_players(&mut session, &data, &m, &null_progress()).unwrap();

    let after = common::relic_possess_num_map(&common::player_sav_json(&session, player_id));

    assert_eq!(
        after.get("EPalRelicType::SwimSpeed").copied(),
        Some(0),
        "granting a swim_speed rank must create RelicPossessNumMap[SwimSpeed] at 0 -- \
         without the key the game never lists the stat"
    );
    assert!(
        !after.contains_key("EPalRelicType::ClimbSpeed"),
        "a rank-0 stat must not create a key"
    );
    assert_eq!(
        after.get(common::CAPTURE_POWER_RELIC).copied(),
        Some(capture_power_before),
        "an existing count is the player's real unspent relics -- never rewrite it"
    );

    let dto = player::build_player_dto(&session, &data, player_id)
        .unwrap()
        .unwrap();
    assert_eq!(dto.status_point_list.get("swim_speed").copied(), Some(5));
}

/// An unchanged resave of a real 1.0 save must add no keys: every rank it carries
/// already has one.
#[test]
fn resaving_a_relic_player_unchanged_adds_no_possess_map_keys() {
    let mut session = common::load_fixture_session("v1_relics");
    let data = game_data();
    let player_id: Uuid = V1_PLAYER_MANY_RELIC_RANKS.parse().unwrap();
    // Only a lazily loaded player has a `.sav` to inspect or write.
    player::get_player_details(&mut session, &data, player_id, &null_progress())
        .unwrap()
        .unwrap();

    let before = common::relic_possess_num_map(&common::player_sav_json(&session, player_id));
    assert!(
        before.len() > 1,
        "fixture sanity: this player must carry several relic types"
    );

    let dto = player::build_player_dto(&session, &data, player_id)
        .unwrap()
        .unwrap();
    let mut m = OrderedMap::new();
    m.insert(player_id, dto);
    player::update_players(&mut session, &data, &m, &null_progress()).unwrap();

    let after = common::relic_possess_num_map(&common::player_sav_json(&session, player_id));
    assert_eq!(
        after, before,
        "an unchanged resave must leave RelicPossessNumMap byte-identical"
    );
}

/// A pre-1.0 save has no `RelicPossessNumMap`; granting a relic rank must not invent one.
#[test]
fn granting_a_relic_rank_on_a_pre_1_0_save_invents_no_possess_map() {
    let mut session = common::load_fixture_session("world1");
    let data = game_data();
    let player_id: Uuid = WORLD1_PLAYER_O.parse().unwrap();
    // Only a lazily loaded player has a `.sav` to inspect or write.
    player::get_player_details(&mut session, &data, player_id, &null_progress())
        .unwrap()
        .unwrap();

    assert!(
        common::relic_possess_num_map(&common::player_sav_json(&session, player_id)).is_empty(),
        "fixture sanity: a pre-1.0 save carries no RelicPossessNumMap"
    );

    let mut dto = player::build_player_dto(&session, &data, player_id)
        .unwrap()
        .unwrap();
    dto.status_point_list.insert("swim_speed".to_string(), 5);

    let mut m = OrderedMap::new();
    m.insert(player_id, dto);
    player::update_players(&mut session, &data, &m, &null_progress()).unwrap();

    assert!(
        common::relic_possess_num_map(&common::player_sav_json(&session, player_id)).is_empty(),
        "a pre-1.0 save must not gain a RelicPossessNumMap"
    );
}
