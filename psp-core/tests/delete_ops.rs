mod common;

use psp_core::domain::guild_tail;
use psp_core::domain::{guild, player, world};
use psp_core::error::CoreError;
use psp_core::gamedata::GameData;
use psp_core::progress::null_progress;
use psp_core::props;
use psp_core::session::{LoadedPlayer, PlayerFileData, SaveKind, SaveSession};
use psp_core::ue::{
    Header, MapEntry, PackageVersion, Properties, Property, PropertyKey, PropertySchemas, Root,
    Save, StructValue, ValueVec,
};
use uuid::Uuid;

fn game_data() -> GameData {
    let json_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../data/json");
    GameData::load(&json_dir).expect("data dir")
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

fn container_id_key(entry: &MapEntry) -> Option<Uuid> {
    props::struct_props(&entry.key)
        .and_then(|key| key.0.get(&PropertyKey::from("ID")))
        .and_then(props::as_uuid)
}

// world1 has 2 guilds, each with exactly one player who is that guild's sole
// admin, one base whose worker container is empty but which owns 4 storage
// containers, and one guild chest.

const WORLD1_GUILD_WITH_BASE: &str = "54491484-4e6c-7327-70b2-868f350929f6";
const WORLD1_GUILD_NO_BASES: &str = "004e71b6-4166-2b71-eb6a-539ae931ca34";
const WORLD1_BASE_ID: &str = "4bb24de8-4965-af19-f596-e296089e8ab0";
const WORLD1_GUILD_CHEST: &str = "1b1b065d-4812-11ba-e444-8f84bbbe40fd";

/// Deleting a guild admin is refused: nothing removed, nothing mutated.
#[test]
fn delete_admin_player_is_refused_and_nothing_is_deleted() {
    let mut session = common::load_fixture_session("world1");
    let data = game_data();
    let guild_id: Uuid = WORLD1_GUILD_WITH_BASE.parse().unwrap();

    let details = guild::get_guild_details(&mut session, &data, guild_id)
        .unwrap()
        .expect("guild loads");
    let admin_id = details.admin_player_uid.expect("guild has an admin");
    player::get_player_details(&mut session, &data, admin_id, &null_progress())
        .unwrap()
        .expect("admin player loads");

    let entry_count_before = world::character_map(&session.level).unwrap().len();
    let container_count_before = world::character_container_map(&session.level)
        .unwrap()
        .len();

    let deleted = player::delete_player(&mut session, &data, admin_id, &null_progress()).unwrap();

    assert!(!deleted, "admin deletion refused (player_ops.py:34-40)");
    assert!(session.player_file_refs.contains_key(&admin_id));
    assert!(session.loaded_players.contains_key(&admin_id));
    assert_eq!(
        world::character_map(&session.level).unwrap().len(),
        entry_count_before,
        "an admin-refused delete must not remove any character-map entry"
    );
    assert_eq!(
        world::character_container_map(&session.level)
            .unwrap()
            .len(),
        container_count_before
    );
}

/// A `player_id` never loaded this session is a hard error, raised before any
/// mutation.
#[test]
fn delete_unloaded_player_is_an_error_and_mutates_nothing() {
    let mut session = common::load_fixture_session("world1");
    let data = game_data();
    let entry_count_before = world::character_map(&session.level).unwrap().len();

    let error =
        player::delete_player(&mut session, &data, Uuid::new_v4(), &null_progress()).unwrap_err();

    assert!(
        matches!(error, CoreError::Other(message) if message.contains("not found in the save file"))
    );
    assert_eq!(
        world::character_map(&session.level).unwrap().len(),
        entry_count_before
    );
}

/// A `guild_id` never loaded this session is a hard error -- and the check
/// must not lazily load the guild as a side effect.
#[test]
fn delete_unloaded_guild_is_an_error_and_mutates_nothing() {
    let mut session = common::load_fixture_session("world1");
    let data = game_data();
    let group_count_before = world::group_map(&session.level).unwrap().len();

    let error =
        guild::delete_guild_and_players(&mut session, &data, Uuid::new_v4(), &null_progress())
            .unwrap_err();

    assert!(
        matches!(error, CoreError::Other(message) if message.contains("not found in the save file"))
    );
    assert_eq!(
        world::group_map(&session.level).unwrap().len(),
        group_count_before
    );
}

/// Deleting one guild must leave the other guild's tail bytes and its admin
/// player entirely untouched -- a delete whose sweep is scoped too broadly
/// (matching any guild rather than the target one) goes red here.
#[test]
fn deleting_one_world1_guild_leaves_the_other_guild_and_its_admin_byte_identical() {
    let mut session = common::load_fixture_session("world1");
    let data = game_data();
    let target_guild: Uuid = WORLD1_GUILD_NO_BASES.parse().unwrap();
    let other_guild: Uuid = WORLD1_GUILD_WITH_BASE.parse().unwrap();

    guild::get_guild_details(&mut session, &data, target_guild)
        .unwrap()
        .expect("target guild loads");
    let other_details = guild::get_guild_details(&mut session, &data, other_guild)
        .unwrap()
        .expect("other guild loads");
    let other_admin = other_details
        .admin_player_uid
        .expect("other guild has an admin");

    let other_tail_before = {
        let entries = world::group_map(&session.level).unwrap();
        let entry = entries
            .iter()
            .find(|e| props::as_uuid(&e.key) == Some(other_guild))
            .unwrap();
        guild_tail::entry_group_data(entry).unwrap().data.clone()
    };
    let other_admin_entry_before = world::character_map(&session.level)
        .unwrap()
        .iter()
        .find(|e| world::entry_player_uid(e) == Some(other_admin))
        .cloned()
        .expect("other guild's admin present before");

    guild::delete_guild_and_players(&mut session, &data, target_guild, &null_progress()).unwrap();

    assert!(guild::guild_entry_index(&session, target_guild)
        .unwrap()
        .is_none());
    assert!(!session.loaded_guilds.contains(&target_guild));

    let other_tail_after = {
        let entries = world::group_map(&session.level).unwrap();
        let entry = entries
            .iter()
            .find(|e| props::as_uuid(&e.key) == Some(other_guild))
            .expect("other guild's group entry must still exist");
        guild_tail::entry_group_data(entry).unwrap().data.clone()
    };
    assert_eq!(
        other_tail_before, other_tail_after,
        "deleting one guild must never touch a different guild's raw tail bytes"
    );

    let other_admin_entry_after = world::character_map(&session.level)
        .unwrap()
        .iter()
        .find(|e| world::entry_player_uid(e) == Some(other_admin))
        .cloned()
        .expect("other guild's admin must still be present");
    assert_eq!(
        other_admin_entry_before, other_admin_entry_after,
        "deleting an unrelated guild must not mutate another guild's admin player entry"
    );
}

/// `delete_guild_and_players` removes the guild's `GroupSaveDataMap`,
/// `GuildExtraSaveDataMap` and `BaseCampSaveData` entries plus its bases'
/// storage containers, but deliberately leaves the guild's own chest behind
/// as an orphan, for save-file fidelity with the game. Deleting the chest
/// would flip the final assertion here.
#[test]
fn delete_guild_removes_group_extra_and_base_entries_but_leaves_the_chest_orphaned() {
    let mut session = common::load_fixture_session("world1");
    let data = game_data();
    let guild_id: Uuid = WORLD1_GUILD_WITH_BASE.parse().unwrap();
    let base_id: Uuid = WORLD1_BASE_ID.parse().unwrap();
    let chest_id: Uuid = WORLD1_GUILD_CHEST.parse().unwrap();

    let guild_before = guild::get_guild_details(&mut session, &data, guild_id)
        .unwrap()
        .expect("guild loads");
    assert!(
        world::item_container_map(&session.level)
            .unwrap()
            .iter()
            .any(|e| container_id_key(e) == Some(chest_id)),
        "test precondition: the real guild chest container exists before delete"
    );
    let base_storage_ids: Vec<Uuid> = guild_before
        .bases
        .as_ref()
        .and_then(|bases| bases.get(&base_id))
        .map(|base| base.storage_containers.iter().map(|(id, _)| *id).collect())
        .unwrap_or_default();
    assert_eq!(
        base_storage_ids.len(),
        4,
        "test precondition: world1's real base has exactly 4 storage containers \
         (verified independently in guild_details.rs)"
    );

    guild::delete_guild_and_players(&mut session, &data, guild_id, &null_progress()).unwrap();

    assert!(guild::guild_entry_index(&session, guild_id)
        .unwrap()
        .is_none());
    assert!(guild::guild_extra_entry_index(&session, guild_id)
        .unwrap()
        .is_none());
    assert!(
        world::base_camp_map(&session.level)
            .unwrap()
            .map(|entries| entries
                .iter()
                .all(|e| props::as_uuid(&e.key) != Some(base_id)))
            .unwrap_or(true),
        "the real base camp entry must be removed"
    );
    assert!(!session.loaded_guilds.contains(&guild_id));

    let item_containers_after = world::item_container_map(&session.level).unwrap();
    for storage_id in &base_storage_ids {
        assert!(
            !item_containers_after
                .iter()
                .any(|e| container_id_key(e) == Some(*storage_id)),
            "base storage container {storage_id} must be deleted along with its base"
        );
    }

    assert!(
        world::item_container_map(&session.level)
            .unwrap()
            .iter()
            .any(|e| container_id_key(e) == Some(chest_id)),
        "save-file fidelity (see delete_guild_and_players's own doc \
         comment): the guild's own item-storage container is never added to \
         container_ids_to_delete, so it survives delete_guild_and_players as an \
         orphaned ItemContainerSaveData entry"
    );

    assert!(session.caches.character_container_index.is_none());
    assert!(session.caches.item_container_index.is_none());
}

// Both world1 guilds have exactly one player (their own sole admin), so no
// fixture here can exercise deleting a NON-admin player. The synthetic
// two-player guild below is the only coverage for that path.

fn guid_property(id: Uuid) -> Property {
    props::guid_property(id)
}

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
    let character_data = psp_core::ue::games::palworld::PalCharacterData {
        object: object_props,
        unknown_bytes: [0; 4],
        group_id: psp_core::ue::FGuid::nil(),
        trailing_bytes: [0; 4],
    };
    let mut value_props = Properties::default();
    value_props.insert(
        "RawData",
        Property::Struct(StructValue::Game(psp_core::ue::PalStruct::CharacterData(character_data))),
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
    value_props.insert("SlotNum", props::int_property(slot_num));
    value_props.insert("Slots", Property::Array(ValueVec::Struct(vec![])));
    MapEntry {
        key: Property::Struct(StructValue::Struct(key_props)),
        value: Property::Struct(StructValue::Struct(value_props)),
    }
}

fn character_container_entry_with_pal(container_id: Uuid, slot_num: i32, pal_id: Uuid) -> MapEntry {
    let mut key_props = Properties::default();
    key_props.insert("ID", guid_property(container_id));
    let mut value_props = Properties::default();
    value_props.insert("SlotNum", props::int_property(slot_num));
    let mut slot_props = Properties::default();
    slot_props.insert("SlotIndex", props::int_property(0));
    slot_props.insert(
        "RawData",
        Property::Struct(StructValue::Game(psp_core::ue::PalStruct::CharacterContainer(
            psp_core::ue::games::palworld::PalCharacterContainer {
                player_uid: props::uuid_to_guid(props::EMPTY_UUID),
                instance_id: props::uuid_to_guid(pal_id),
                permission_tribe_id: 0,
                trailing_bytes: None,
            },
        ))),
    );
    value_props.insert(
        "Slots",
        Property::Array(ValueVec::Struct(vec![StructValue::Struct(slot_props)])),
    );
    MapEntry {
        key: Property::Struct(StructValue::Struct(key_props)),
        value: Property::Struct(StructValue::Struct(value_props)),
    }
}

fn player_sav_with_containers(pal_box_id: Uuid, otomo_id: Uuid) -> Save {
    let mut pal_box_id_struct = Properties::default();
    pal_box_id_struct.insert("ID", guid_property(pal_box_id));
    let mut otomo_id_struct = Properties::default();
    otomo_id_struct.insert("ID", guid_property(otomo_id));
    let mut player_save_data = Properties::default();
    player_save_data.insert(
        "PalStorageContainerId",
        Property::Struct(StructValue::Struct(pal_box_id_struct)),
    );
    player_save_data.insert(
        "OtomoCharacterContainerId",
        Property::Struct(StructValue::Struct(otomo_id_struct)),
    );
    let mut player_root_properties = Properties::default();
    player_root_properties.insert(
        "SaveData",
        Property::Struct(StructValue::Struct(player_save_data)),
    );
    minimal_save(player_root_properties)
}

fn guild_group_entry(
    guild_id: Uuid,
    handle_ids: &[Uuid],
    guild: psp_core::ue::games::palworld::PalGuildGroup,
) -> MapEntry {
    let mut value_properties = Properties::default();
    value_properties.insert(
        "GroupType",
        Property::Enum("EPalGroupType::Guild".to_string()),
    );
    let group_data = psp_core::ue::games::palworld::PalGroupData {
        group_id: props::uuid_to_guid(guild_id),
        group_name: String::new(),
        individual_character_handle_ids: handle_ids
            .iter()
            .map(|id| psp_core::ue::games::palworld::PalInstanceId {
                guid: props::uuid_to_guid(props::EMPTY_UUID),
                instance_id: props::uuid_to_guid(*id),
            })
            .collect(),
        data: psp_core::ue::games::palworld::PalGroupVariant::Guild(guild),
    };
    value_properties.insert(
        "RawData",
        Property::Struct(StructValue::Game(psp_core::ue::PalStruct::GroupData(group_data))),
    );
    MapEntry {
        key: guid_property(guild_id),
        value: Property::Struct(StructValue::Struct(value_properties)),
    }
}

fn guild_extra_entry(guild_id: Uuid) -> MapEntry {
    MapEntry {
        key: guid_property(guild_id),
        value: Property::Struct(StructValue::Struct(Properties::default())),
    }
}

struct TwoPlayerGuild {
    session: SaveSession,
    data: GameData,
    guild_id: Uuid,
    admin_id: Uuid,
    member_id: Uuid,
    member_pal_id: Uuid,
    member_pal_box_id: Uuid,
}

/// A guild with an admin plus one non-admin member who owns a single pal in
/// their own pal box. Both players' guild handles AND the pal's are recorded
/// in `individual_character_handle_ids` up front, so a delete's handle cleanup
/// (or lack of it) is directly observable afterward.
fn two_player_guild_session(guild_loaded: bool) -> TwoPlayerGuild {
    let data = game_data();
    let guild_id = Uuid::new_v4();
    let admin_id = Uuid::new_v4();
    let member_id = Uuid::new_v4();
    let admin_pal_box = Uuid::new_v4();
    let admin_party = Uuid::new_v4();
    let member_pal_box = Uuid::new_v4();
    let member_party = Uuid::new_v4();
    let member_pal_id = Uuid::new_v4();

    let member_pal_entry = psp_core::domain::pal::new_pal_entry(
        "SheepBall",
        member_pal_id,
        member_id,
        member_pal_box,
        0,
        Some(guild_id),
        "Wooly",
        &data,
    );
    let admin_entry = player_character_entry(admin_id);
    let member_entry = player_character_entry(member_id);

    let guild = guild_tail::pre_update_guild(
        1,
        "Two Player Guild",
        admin_id,
        &[(admin_id, 0, "Admin"), (member_id, 0, "Member")],
    );
    let group_entry = guild_group_entry(guild_id, &[admin_id, member_id, member_pal_id], guild);
    let guild_extra = guild_extra_entry(guild_id);

    let mut world_save_data = Properties::default();
    world_save_data.insert(
        "CharacterSaveParameterMap",
        Property::Map(vec![admin_entry, member_entry, member_pal_entry]),
    );
    world_save_data.insert(
        "CharacterContainerSaveData",
        Property::Map(vec![
            empty_character_container_entry(admin_pal_box, 1),
            empty_character_container_entry(admin_party, 1),
            character_container_entry_with_pal(member_pal_box, 1, member_pal_id),
            empty_character_container_entry(member_party, 1),
        ]),
    );
    world_save_data.insert("ItemContainerSaveData", Property::Map(vec![]));
    world_save_data.insert("GroupSaveDataMap", Property::Map(vec![group_entry]));
    world_save_data.insert("GuildExtraSaveDataMap", Property::Map(vec![guild_extra]));
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
    for (player_id, pal_box_id, otomo_id) in [
        (admin_id, admin_pal_box, admin_party),
        (member_id, member_pal_box, member_party),
    ] {
        session.loaded_players.insert(
            player_id,
            LoadedPlayer {
                uid: player_id,
                sav: player_sav_with_containers(pal_box_id, otomo_id),
                dps: None,
            },
        );
        session.player_file_refs.insert(
            player_id,
            PlayerFileData::Bytes {
                sav: None,
                dps: None,
            },
        );
    }
    if guild_loaded {
        session.loaded_guilds.insert(guild_id);
    }

    TwoPlayerGuild {
        session,
        data,
        guild_id,
        admin_id,
        member_id,
        member_pal_id,
        member_pal_box_id: member_pal_box,
    }
}

fn group_data_for(session: &SaveSession, guild_id: Uuid) -> &psp_core::ue::games::palworld::PalGroupData {
    let entries = world::group_map(&session.level).unwrap();
    let entry = entries
        .iter()
        .find(|e| props::as_uuid(&e.key) == Some(guild_id))
        .expect("guild group entry must still exist");
    guild_tail::entry_group_data(entry).expect("group data must be typed")
}

/// Deleting the non-admin member removes everything of theirs (character-map
/// entry, pal, containers, file ref, guild membership row and handle) while
/// leaving the admin's own entry byte-identical -- an over-broad delete that
/// searches the character map unscoped goes red here.
#[test]
fn delete_non_admin_player_removes_everything_and_leaves_the_admin_byte_identical() {
    let TwoPlayerGuild {
        mut session,
        data,
        guild_id,
        admin_id,
        member_id,
        member_pal_id,
        member_pal_box_id,
    } = two_player_guild_session(true);

    let admin_entry_before = world::character_map(&session.level)
        .unwrap()
        .iter()
        .find(|e| world::entry_player_uid(e) == Some(admin_id))
        .cloned()
        .unwrap();

    let deleted = player::delete_player(&mut session, &data, member_id, &null_progress()).unwrap();
    assert!(deleted);

    assert!(!world::character_map(&session.level)
        .unwrap()
        .iter()
        .any(|e| world::entry_player_uid(e) == Some(member_id)));
    assert!(!world::character_map(&session.level)
        .unwrap()
        .iter()
        .any(|e| world::entry_instance_id(e) == Some(member_pal_id)));
    assert!(!session.loaded_players.contains_key(&member_id));
    assert!(!session.player_file_refs.contains_key(&member_id));
    assert!(!world::character_container_map(&session.level)
        .unwrap()
        .iter()
        .any(|e| container_id_key(e) == Some(member_pal_box_id)));

    let admin_entry_after = world::character_map(&session.level)
        .unwrap()
        .iter()
        .find(|e| world::entry_player_uid(e) == Some(admin_id))
        .cloned()
        .expect("admin must still be present");
    assert_eq!(
        admin_entry_before, admin_entry_after,
        "deleting the member must not mutate the admin's character-map entry byte-for-byte"
    );
    assert!(session.loaded_players.contains_key(&admin_id));

    let group_data = group_data_for(&session, guild_id);
    let guild = guild_tail::as_guild(group_data).unwrap();
    assert_eq!(guild_tail::guild_player_count(guild), 1);
    assert_eq!(guild_tail::guild_player_uids(guild)[0], admin_id);
    assert_eq!(guild_tail::guild_admin_uid(guild), admin_id);
    assert_eq!(guild.guild_name, "Two Player Guild");

    assert!(!group_data
        .individual_character_handle_ids
        .iter()
        .any(|h| props::guid_to_uuid(&h.instance_id) == member_id));
    assert!(group_data
        .individual_character_handle_ids
        .iter()
        .any(|h| props::guid_to_uuid(&h.instance_id) == admin_id));

    // The caches must be reset AND the rebuilt index must no longer resolve
    // the deleted pal/container -- `is_none()` alone would also hold for a
    // mutation that never happened.
    assert!(session.caches.character_index.is_none());
    assert!(session.caches.character_container_index.is_none());
    let character_index = world::build_character_index(&session.level);
    assert!(!character_index.contains_key(&member_pal_id));
    let container_index = world::build_character_container_index(&session.level);
    assert!(!container_index.contains_key(&member_pal_box_id));
}

/// Deleting a player leaves their pals' guild handles dangling: the whole-
/// player delete path never calls `delete_character_handle` for box/party
/// pals, unlike a single-pal delete. Deliberate, for save-file fidelity with
/// the game -- cleaning the handles up would flip this assertion.
#[test]
fn delete_player_leaves_the_deleted_players_own_pal_guild_handle_dangling() {
    let TwoPlayerGuild {
        mut session,
        data,
        guild_id,
        member_id,
        member_pal_id,
        ..
    } = two_player_guild_session(true);

    player::delete_player(&mut session, &data, member_id, &null_progress()).unwrap();

    // The pal is gone from the character map, but its guild handle is not.
    assert!(!world::character_map(&session.level)
        .unwrap()
        .iter()
        .any(|e| world::entry_instance_id(e) == Some(member_pal_id)));
    let group_data = group_data_for(&session, guild_id);
    assert!(
        group_data
            .individual_character_handle_ids
            .iter()
            .any(|h| props::guid_to_uuid(&h.instance_id) == member_pal_id),
        "save-file fidelity (byte-identical save output, \
         see delete_player_and_pals_for_guild's own doc comment): the deleted \
         player's own pal's individual_character_handle_ids entry must remain \
         dangling in the guild's raw tail"
    );
}

#[test]
fn delete_admin_player_is_refused_when_their_guild_is_loaded_this_session() {
    let TwoPlayerGuild {
        mut session,
        data,
        admin_id,
        ..
    } = two_player_guild_session(true);
    let entry_count_before = world::character_map(&session.level).unwrap().len();

    let deleted = player::delete_player(&mut session, &data, admin_id, &null_progress()).unwrap();

    assert!(!deleted);
    assert_eq!(
        world::character_map(&session.level).unwrap().len(),
        entry_count_before
    );
}

/// The admin-refusal check is scoped to `session.loaded_guilds`, never the
/// raw `GroupSaveDataMap`: an admin whose guild was never loaded this session
/// counts as guildless, so the delete proceeds rather than being refused.
#[test]
fn delete_admin_player_is_allowed_when_their_guild_was_never_loaded_this_session() {
    let TwoPlayerGuild {
        mut session,
        data,
        admin_id,
        ..
    } = two_player_guild_session(false); // guild NOT inserted into loaded_guilds

    let deleted = player::delete_player(&mut session, &data, admin_id, &null_progress()).unwrap();

    assert!(
        deleted,
        "an admin whose guild was never loaded this session must be treated as \
         guildless by delete_player, matching real Python's self._guilds-scoped \
         _player_guild lookup -- deletion must proceed, not be refused"
    );
    assert!(!world::character_map(&session.level)
        .unwrap()
        .iter()
        .any(|e| world::entry_player_uid(e) == Some(admin_id)));
}

// world1's base storage containers (the only real containers deleted above)
// hold zero dynamic items, so the dynamic-item cascade has no real-save
// coverage. The synthetic container below is its only coverage.

fn item_container_entry_with_dynamic_slot(container_id: Uuid, local_id: Uuid) -> MapEntry {
    let mut key_props = Properties::default();
    key_props.insert("ID", guid_property(container_id));
    let mut slot_props = Properties::default();
    slot_props.insert("SlotIndex", props::int_property(0));
    slot_props.insert(
        "RawData",
        Property::Struct(StructValue::Game(psp_core::ue::PalStruct::ItemContainerSlots(
            psp_core::ue::games::palworld::PalItemContainerSlot {
                slot_index: 0,
                count: 1,
                item: psp_core::ue::games::palworld::PalItemId {
                    static_id: "WeaponFire_Bow".to_string(),
                    dynamic_id: psp_core::ue::games::palworld::PalDynamicId {
                        created_world_id: psp_core::ue::FGuid::nil(),
                        local_id_in_created_world: props::uuid_to_guid(local_id),
                    },
                },
                trailing_bytes: vec![0u8; 16],
            },
        ))),
    );
    let mut value_props = Properties::default();
    value_props.insert("SlotNum", props::int_property(1));
    value_props.insert(
        "Slots",
        Property::Array(ValueVec::Struct(vec![StructValue::Struct(slot_props)])),
    );
    MapEntry {
        key: Property::Struct(StructValue::Struct(key_props)),
        value: Property::Struct(StructValue::Struct(value_props)),
    }
}

fn dynamic_item_value(local_id: Uuid) -> StructValue {
    let mut item_props = Properties::default();
    item_props.insert(
        "RawData",
        Property::Struct(StructValue::Game(psp_core::ue::PalStruct::DynamicItem(Box::new(
            psp_core::ue::games::palworld::PalDynamicItem {
                id: psp_core::ue::games::palworld::PalDynamicId {
                    created_world_id: psp_core::ue::FGuid::nil(),
                    local_id_in_created_world: props::uuid_to_guid(local_id),
                },
                static_id: "WeaponFire_Bow".to_string(),
                item_type: psp_core::ue::games::palworld::PalDynamicItemType::Unknown { trailer: vec![] },
            },
        )))),
    );
    StructValue::Struct(item_props)
}

/// Deleting a container removes its `ItemContainerSaveData` entry and
/// cascades to the `DynamicItemSaveData` entry its slot referenced, resetting
/// both caches. The rebuilt indexes are checked too: `is_none()` alone would
/// also hold for a mutation that never happened.
#[test]
fn delete_item_containers_cascades_its_dynamic_item_and_invalidates_both_indexes() {
    let container_id = Uuid::new_v4();
    let other_container_id = Uuid::new_v4();
    let local_id = Uuid::new_v4();

    let mut world_save_data = Properties::default();
    world_save_data.insert(
        "ItemContainerSaveData",
        Property::Map(vec![
            item_container_entry_with_dynamic_slot(container_id, local_id),
            empty_character_container_entry(other_container_id, 1), // unrelated survivor
        ]),
    );
    world_save_data.insert("CharacterSaveParameterMap", Property::Map(vec![]));
    world_save_data.insert("CharacterContainerSaveData", Property::Map(vec![]));
    world_save_data.insert("GroupSaveDataMap", Property::Map(vec![]));
    world_save_data.insert(
        "DynamicItemSaveData",
        Property::Array(ValueVec::Struct(vec![dynamic_item_value(local_id)])),
    );
    let mut root_properties = Properties::default();
    root_properties.insert(
        "worldSaveData",
        Property::Struct(StructValue::Struct(world_save_data)),
    );
    let mut session = SaveSession::new_for_tests(SaveKind::InMemory, minimal_save(root_properties));

    assert_eq!(world::item_container_map(&session.level).unwrap().len(), 2);
    assert_eq!(world::dynamic_item_values(&session.level).unwrap().len(), 1);

    psp_core::domain::containers::delete_item_containers(&mut session, &[container_id]).unwrap();

    let containers_after = world::item_container_map(&session.level).unwrap();
    assert_eq!(containers_after.len(), 1);
    assert!(!containers_after
        .iter()
        .any(|e| container_id_key(e) == Some(container_id)));
    assert!(containers_after
        .iter()
        .any(|e| container_id_key(e) == Some(other_container_id)));
    assert!(world::dynamic_item_values(&session.level)
        .unwrap()
        .is_empty());

    assert!(session.caches.item_container_index.is_none());
    assert!(session.caches.dynamic_item_index.is_none());
    let item_index = world::build_item_container_index(&session.level);
    assert!(!item_index.contains_key(&container_id));
    assert!(item_index.contains_key(&other_container_id));
    let dynamic_index = world::build_dynamic_item_index(&session.level);
    assert!(!dynamic_index.contains_key(&local_id));
}

/// The same cross-entity property the synthetic two-player guild proves,
/// spot-checked against the committed `v1_relics` corpus fixture: deleting
/// a non-admin player leaves every other player's entry untouched.
#[test]
fn delete_non_admin_player_round_trips_against_an_optional_real_corpus_save() {
    let mut session = common::load_corpus_session();
    let data = game_data();
    let player_ids: Vec<Uuid> = session.player_summaries.keys().copied().collect();

    let mut target = None;
    for player_id in &player_ids {
        let Some(details) =
            player::get_player_details(&mut session, &data, *player_id, &null_progress()).unwrap()
        else {
            continue;
        };
        let is_admin = guild::find_player_guild_id(&mut session, *player_id)
            .unwrap()
            .and_then(|guild_id| guild::get_guild_details(&mut session, &data, guild_id).unwrap())
            .and_then(|guild_details| guild_details.admin_player_uid)
            == Some(*player_id);
        if !is_admin {
            target = Some(details.uid);
            break;
        }
    }
    let Some(player_id) = target else {
        eprintln!("corpus has no non-admin player; skipping");
        return;
    };

    let other_entries_before: Vec<MapEntry> = world::character_map(&session.level)
        .unwrap()
        .iter()
        .filter(|e| world::entry_is_player(e) && world::entry_player_uid(e) != Some(player_id))
        .cloned()
        .collect();

    let deleted = player::delete_player(&mut session, &data, player_id, &null_progress()).unwrap();
    assert!(deleted);
    assert!(!world::character_map(&session.level)
        .unwrap()
        .iter()
        .any(|e| world::entry_player_uid(e) == Some(player_id)));

    let other_entries_after: Vec<MapEntry> = world::character_map(&session.level)
        .unwrap()
        .iter()
        .filter(|e| world::entry_is_player(e) && world::entry_player_uid(e) != Some(player_id))
        .cloned()
        .collect();
    assert_eq!(
        other_entries_before, other_entries_after,
        "deleting one player must not touch any other player's character-map entry"
    );
}
