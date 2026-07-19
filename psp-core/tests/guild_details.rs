mod common;

use psp_core::domain::{guild, guild_tail, world};
use psp_core::dto::guild::GuildLabResearchInfo;
use psp_core::error::CoreError;
use psp_core::gamedata::GameData;
use psp_core::session::{SaveKind, SaveSession};
use psp_core::ue::{
    Header, MapEntry, PackageVersion, Properties, Property, PropertySchemas, Root, Save,
    StructValue,
};
use uuid::Uuid;

fn game_data() -> GameData {
    let json_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../data/json");
    GameData::load(&json_dir).expect("data dir")
}

// world1's founding guild: admin 8c2f1930 ("O"), base_camp_level 1, one base
// (`4bb24de8-...`) whose worker container is empty (SlotNum 1, 0 filled) but
// which owns 4 storage containers (one "ItemChest", three "CommonDropItem3D"),
// and 150 lab research entries (all work_amount 0.0).
const WORLD1_GUILD_WITH_BASE: &str = "54491484-4e6c-7327-70b2-868f350929f6";
const WORLD1_GUILD_ADMIN: &str = "8c2f1930-0000-0000-0000-000000000000";
const WORLD1_BASE_ID: &str = "4bb24de8-4965-af19-f596-e296089e8ab0";
const WORLD1_WORKER_CONTAINER: &str = "a77f85ca-4037-97d8-acef-fcb73f1d931b";
const WORLD1_GUILD_CHEST: &str = "1b1b065d-4812-11ba-e444-8f84bbbe40fd";
// world1's second guild: admin 43797f87 ("sky"), no bases.
const WORLD1_GUILD_NO_BASES: &str = "004e71b6-4166-2b71-eb6a-539ae931ca34";
const WORLD1_GUILD_NO_BASES_ADMIN: &str = "43797f87-0000-0000-0000-000000000000";

#[test]
fn guild_details_load_real_base_lab_research_and_guild_chest() {
    let mut session = common::load_fixture_session("world1");
    let data = game_data();
    let guild_id: Uuid = WORLD1_GUILD_WITH_BASE.parse().unwrap();

    let details = guild::get_guild_details(&mut session, &data, guild_id)
        .unwrap()
        .expect("guild loads");

    assert_eq!(details.id, Some(guild_id));
    assert_eq!(details.name, Some("Unnamed Guild".to_string()));
    assert_eq!(details.base_camp_level, Some(1));
    let expected_admin: Uuid = WORLD1_GUILD_ADMIN.parse().unwrap();
    assert_eq!(details.players, vec![expected_admin]);
    assert_eq!(
        details.admin_player_uid,
        Some(details.players[0]),
        "admin = first player (guild.py:76-77)"
    );

    let lab_research = details.lab_research.clone().unwrap_or_default();
    assert_eq!(lab_research.len(), 150);
    assert_eq!(details.lab_research_data.len(), lab_research.len());
    let handcraft1 = lab_research
        .iter()
        .find(|entry| entry.research_id == "Handcraft1")
        .expect("Handcraft1 present");
    assert_eq!(handcraft1.work_amount, 0.0);

    assert_eq!(
        details.container_id,
        Some(WORLD1_GUILD_CHEST.parse().unwrap())
    );
    let guild_chest = details.guild_chest.as_ref().expect("guild chest resolves");
    assert_eq!(guild_chest.slot_num, 54);
    assert_eq!(guild_chest.r#type, "GuildChest");
    assert_eq!(guild_chest.key, Some("GuildChest".to_string()));

    let bases = details.bases.as_ref().expect("bases is Some, not None");
    assert_eq!(bases.len(), 1);
    let base_id: Uuid = WORLD1_BASE_ID.parse().unwrap();
    let base = bases.get(&base_id).expect("the real base is present");
    assert_eq!(base.id, base_id);
    assert_eq!(
        base.name,
        Some("新規生成拠点テンプレート名0(仮)".to_string())
    );
    assert_eq!(base.area_range, Some(3500.0));
    let location = base.location.as_ref().expect("location present");
    assert!((location.x - 685.8294149692176).abs() < 1e-6);
    assert!((location.y - (-136953.51467997508)).abs() < 1e-6);
    assert!((location.z - 2886.9534098080685).abs() < 1e-6);
    assert_eq!(
        base.container_id,
        Some(WORLD1_WORKER_CONTAINER.parse().unwrap())
    );
    assert_eq!(base.slot_count, Some(1));
    assert!(
        base.pals.is_empty(),
        "the real worker container is empty (SlotNum 1, 0 filled)"
    );
    let pal_container = base.pal_container.as_ref().expect("pal_container present");
    assert_eq!(pal_container.r#type, "Base");
    assert_eq!(pal_container.player_uid, psp_core::props::EMPTY_UUID);
    assert_eq!(pal_container.size, 1);
    assert!(pal_container.slots.is_empty());

    assert_eq!(base.storage_containers.len(), 4);
    let item_chest = base
        .storage_containers
        .iter()
        .find(|(_, container)| container.key.as_deref() == Some("ItemChest"))
        .expect("ItemChest storage container present");
    assert_eq!(item_chest.1.slot_num, 10);
    assert_eq!(item_chest.1.r#type, "BaseContainer");

    assert!(session.loaded_guilds.contains(&guild_id));
    assert!(session.guild_summaries[&guild_id].loaded);
}

/// A guild with zero bases must surface `bases: Some(empty map)`, never
/// `None` -- the wire contract the frontend relies on.
#[test]
fn guild_details_with_no_bases_is_some_empty_map_not_none() {
    let mut session = common::load_fixture_session("world1");
    let data = game_data();
    let guild_id: Uuid = WORLD1_GUILD_NO_BASES.parse().unwrap();

    let details = guild::get_guild_details(&mut session, &data, guild_id)
        .unwrap()
        .expect("guild loads");

    let expected_admin: Uuid = WORLD1_GUILD_NO_BASES_ADMIN.parse().unwrap();
    assert_eq!(details.players, vec![expected_admin]);
    let bases = details
        .bases
        .expect("bases is Some, not None, even when empty");
    assert!(bases.is_empty());
    // This guild's tail carries no base ids, so its chest is whatever
    // `GuildExtraSaveDataMap` says: assert only that the container_id and
    // guild_chest agree with each other (both present or both absent).
    assert_eq!(
        details.guild_chest.is_some(),
        details.container_id.is_some()
    );
}

/// Broad but shallow: every guild in both fixtures loads and flips `loaded`.
#[test]
fn every_fixture_guild_loads_without_panicking() {
    let mut guild_count = 0;
    for fixture_name in ["world1", "world2"] {
        let mut session = common::load_fixture_session(fixture_name);
        let data = game_data();
        let guild_ids: Vec<Uuid> = session.guild_summaries.keys().copied().collect();
        assert!(!guild_ids.is_empty(), "{fixture_name} has no guilds");
        for guild_id in guild_ids {
            let details = guild::get_guild_details(&mut session, &data, guild_id)
                .unwrap()
                .unwrap_or_else(|| panic!("{fixture_name} guild {guild_id} failed to load"));
            assert_eq!(details.id, Some(guild_id));
            assert!(session.loaded_guilds.contains(&guild_id));
            assert!(session.guild_summaries[&guild_id].loaded);
            guild_count += 1;
        }
    }
    assert_eq!(guild_count, 3, "world1 has 2 guilds, world2 has 1");
}

/// The same sweep against the committed `v1_relics` corpus fixture.
#[test]
fn every_corpus_guild_loads_without_panicking() {
    let mut session = common::load_corpus_session();
    let data = game_data();
    let guild_ids: Vec<Uuid> = session.guild_summaries.keys().copied().collect();
    assert!(!guild_ids.is_empty());
    for guild_id in guild_ids {
        let details = guild::get_guild_details(&mut session, &data, guild_id)
            .unwrap()
            .expect("every guild_summaries entry must be loadable on demand");
        assert_eq!(details.id, Some(guild_id));
        assert!(details.bases.is_some());
    }
}

#[test]
fn unknown_guild_returns_none() {
    let mut session = common::load_fixture_session("world1");
    let data = game_data();

    let result = guild::get_guild_details(&mut session, &data, Uuid::new_v4()).unwrap();

    assert!(result.is_none());
}

#[test]
fn lab_research_update_round_trips() {
    let mut session = common::load_fixture_session("world1");
    let data = game_data();
    let guild_id: Uuid = WORLD1_GUILD_WITH_BASE.parse().unwrap();

    guild::get_guild_details(&mut session, &data, guild_id).unwrap();
    let updates = vec![GuildLabResearchInfo {
        research_id: "Research_Tech_01".to_string(),
        work_amount: 250.0,
    }];
    guild::update_lab_research(&mut session, guild_id, &updates).unwrap();

    let details = guild::get_guild_details(&mut session, &data, guild_id)
        .unwrap()
        .unwrap();
    let research = details.lab_research.unwrap_or_default();
    assert_eq!(research.len(), 1, "full replacement, not a merge");
    assert_eq!(research[0].research_id, "Research_Tech_01");
    assert_eq!(research[0].work_amount, 250.0);
    assert_eq!(details.lab_research_data.len(), 1);
}

#[test]
fn update_lab_research_on_a_guild_never_loaded_is_guild_not_found() {
    let mut session = common::load_fixture_session("world1");
    let guild_id: Uuid = WORLD1_GUILD_WITH_BASE.parse().unwrap();
    assert!(!session.loaded_guilds.contains(&guild_id));

    let result = guild::update_lab_research(&mut session, guild_id, &[]);

    assert!(matches!(result, Err(CoreError::GuildNotFound(id)) if id == guild_id));
}

/// A loaded guild whose `GuildExtraSaveDataMap` entry carries no `"Lab"`
/// property must no-op, not error. Synthetic: no fixture guild is missing
/// `Lab`.
#[test]
fn update_lab_research_with_no_lab_data_is_a_silent_no_op() {
    let guild_id: Uuid = "11111111-2222-3333-4444-555555555555".parse().unwrap();

    let mut extra_value_properties = Properties::default();
    extra_value_properties.insert("SomeOtherField", Property::Bool(true)); // no "Lab" key
    let extra_entry = MapEntry {
        key: Property::Struct(StructValue::Guid(psp_core::props::uuid_to_guid(guild_id))),
        value: Property::Struct(StructValue::Struct(extra_value_properties)),
    };
    let mut world_save_data = Properties::default();
    world_save_data.insert("GuildExtraSaveDataMap", Property::Map(vec![extra_entry]));
    let mut root_properties = Properties::default();
    root_properties.insert(
        "worldSaveData",
        Property::Struct(StructValue::Struct(world_save_data)),
    );
    let level = Save {
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
            properties: root_properties,
        },
        extra: Vec::new(),
    };
    let mut session = SaveSession::new_for_tests(SaveKind::InMemory, level);
    session.loaded_guilds.insert(guild_id);

    let result = guild::update_lab_research(
        &mut session,
        guild_id,
        &[GuildLabResearchInfo {
            research_id: "X".to_string(),
            work_amount: 1.0,
        }],
    );

    assert!(result.is_ok(), "missing Lab data must no-op, not error");
}

fn raw_tail_bytes(
    session: &SaveSession,
    guild_id: Uuid,
) -> psp_core::ue::games::palworld::PalGroupVariant {
    let entries = world::group_map(&session.level).unwrap();
    let entry_index = guild::guild_entry_index(session, guild_id)
        .unwrap()
        .expect("guild present in GroupSaveDataMap");
    guild_tail::entry_group_data(&entries[entry_index])
        .unwrap()
        .data
        .clone()
}

/// `update_lab_research` writes only into `GuildExtraSaveDataMap.Lab`, a
/// separate struct from `GroupSaveDataMap`'s guild tail, so neither it nor
/// `get_guild_details` may perturb a single tail byte.
#[test]
fn guild_details_and_lab_research_update_never_touch_the_raw_guild_tail_bytes() {
    let mut session = common::load_fixture_session("world1");
    let data = game_data();
    let guild_id: Uuid = WORLD1_GUILD_WITH_BASE.parse().unwrap();

    let original_bytes = raw_tail_bytes(&session, guild_id);

    guild::get_guild_details(&mut session, &data, guild_id).unwrap();
    guild::update_lab_research(
        &mut session,
        guild_id,
        &[GuildLabResearchInfo {
            research_id: "X".to_string(),
            work_amount: 5.0,
        }],
    )
    .unwrap();
    guild::get_guild_details(&mut session, &data, guild_id).unwrap();

    assert_eq!(
        raw_tail_bytes(&session, guild_id),
        original_bytes,
        "get_guild_details/update_lab_research must never mutate GroupSaveDataMap's raw tail"
    );
}

/// Neither guild call inserts or removes a map entry -- `update_lab_research`
/// replaces a `Vec` nested inside one already-positioned entry -- so every
/// position-keyed index must resolve identically afterward and no cache
/// invalidation is required.
#[test]
fn guild_operations_never_move_any_world_tree_index_position() {
    let mut session = common::load_fixture_session("world1");
    let data = game_data();
    let guild_id: Uuid = WORLD1_GUILD_WITH_BASE.parse().unwrap();

    let character_index_before = world::build_character_index(&session.level);
    let item_container_index_before = world::build_item_container_index(&session.level);
    let character_container_index_before = world::build_character_container_index(&session.level);

    guild::get_guild_details(&mut session, &data, guild_id).unwrap();
    guild::update_lab_research(
        &mut session,
        guild_id,
        &[GuildLabResearchInfo {
            research_id: "X".to_string(),
            work_amount: 1.0,
        }],
    )
    .unwrap();
    guild::get_guild_details(&mut session, &data, guild_id).unwrap();

    assert_eq!(
        character_index_before,
        world::build_character_index(&session.level)
    );
    assert_eq!(
        item_container_index_before,
        world::build_item_container_index(&session.level)
    );
    assert_eq!(
        character_container_index_before,
        world::build_character_container_index(&session.level)
    );
}
