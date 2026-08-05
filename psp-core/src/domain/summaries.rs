//! Player/guild summary extraction.

use std::collections::{BTreeMap, HashMap};

use crate::dto::summary::{ticks_to_datetime, GuildSummary, IsoDateTime, PlayerSummary};
use crate::error::CoreError;
use crate::palbin;
use crate::progress::ProgressSink;
use crate::props;
use crate::session::{parse_palworld_save, SaveSession};
use crate::ue::games::palworld::PalGuildGroup;
use uuid::Uuid;

use super::guild_tail;

const GROUP_TYPE_GUILD: &str = "EPalGroupType::Guild";

/// `entry.value.RawData(.PalCharacterData).object.SaveParameter` — the
/// property bag every other accessor in this module reads from. A missing or
/// mistyped link anywhere in the chain yields `None`; callers skip the entry.
pub(crate) fn save_parameter(entry: &crate::ue::MapEntry) -> Option<&crate::ue::Properties> {
    let value_properties = props::struct_properties(&entry.value)?;
    let raw_data = props::get(value_properties, &["RawData"])?;
    let crate::ue::Property::Struct(crate::ue::StructValue::Game(crate::ue::PalStruct::CharacterData(character_data))) = raw_data
    else {
        return None;
    };
    props::get(&character_data.object, &["SaveParameter"]).and_then(props::struct_properties)
}

/// `IsPlayer`, defaulting to `false` when the property is absent.
pub(crate) fn is_player_entry(save_parameter: &crate::ue::Properties) -> bool {
    props::get(save_parameter, &["IsPlayer"])
        .and_then(props::as_bool)
        .unwrap_or(false)
}

fn player_uid_from_key(entry: &crate::ue::MapEntry) -> Option<Uuid> {
    props::get_in(&entry.key, &["PlayerUId"]).and_then(props::as_uuid)
}

/// A `GroupSaveDataMap` entry's decoded guild, when it is a Guild-type group
/// whose `RawData` decodes cleanly. A nil guild id is deliberately kept here;
/// only `build_guild_summaries` filters it out.
fn guild_tail_entry(entry: &crate::ue::MapEntry) -> Option<(Uuid, &PalGuildGroup)> {
    let value_properties = props::struct_properties(&entry.value)?;
    let group_type = props::get(value_properties, &["GroupType"]).and_then(props::as_enum)?;
    if group_type != GROUP_TYPE_GUILD {
        return None;
    }
    let guild_id = props::as_uuid(&entry.key)?;
    let raw_data = props::get(value_properties, &["RawData"])?;
    let crate::ue::Property::Struct(crate::ue::StructValue::Game(crate::ue::PalStruct::GroupData(group_data))) = raw_data else {
        return None;
    };
    let guild = guild_tail::as_guild(group_data)?;
    Some((guild_id, guild))
}

pub(crate) fn build_player_guild_map(group_entries: &[crate::ue::MapEntry]) -> HashMap<Uuid, Uuid> {
    let mut player_guild_map = HashMap::new();
    for entry in group_entries {
        let Some((guild_id, guild)) = guild_tail_entry(entry) else {
            continue;
        };
        for player_uid in guild_tail::guild_player_uids(guild) {
            player_guild_map.insert(player_uid, guild_id);
        }
    }
    player_guild_map
}

/// Counts every non-player character entry against its `OwnerPlayerUId`,
/// including the nil UUID (wild/unowned pals get their own bucket).
pub(crate) fn build_pal_owner_counts(character_entries: &[crate::ue::MapEntry]) -> HashMap<Uuid, i64> {
    let mut owner_counts = HashMap::new();
    for entry in character_entries {
        let Some(parameters) = save_parameter(entry) else {
            continue;
        };
        if is_player_entry(parameters) {
            continue;
        }
        if let Some(owner_uid) =
            props::get(parameters, &["OwnerPlayerUId"]).and_then(props::as_uuid)
        {
            *owner_counts.entry(owner_uid).or_insert(0) += 1;
        }
    }
    owner_counts
}

/// Player character entries with a non-nil `PlayerUId`, paired with their
/// save parameter bag.
pub(crate) fn collect_player_entries(
    character_entries: &[crate::ue::MapEntry],
) -> Vec<(Uuid, &crate::ue::Properties)> {
    let mut players = Vec::new();
    for entry in character_entries {
        let Some(parameters) = save_parameter(entry) else {
            continue;
        };
        if !is_player_entry(parameters) {
            continue;
        }
        if let Some(uid) = player_uid_from_key(entry).filter(|uid| !uid.is_nil()) {
            players.push((uid, parameters));
        }
    }
    players
}

/// Player-facing nickname: `NickName` if present and non-empty, otherwise
/// `"Player (<uid8>)"`.
fn player_nickname(uid: Uuid, save_parameter: &crate::ue::Properties) -> String {
    props::get(save_parameter, &["NickName"])
        .and_then(props::as_str)
        .filter(|name| !name.is_empty())
        .map(str::to_owned)
        .unwrap_or_else(|| format!("Player ({})", &uid.to_string()[..8]))
}

/// The caller supplies `last_online_time`: it comes from the player's own
/// `.sav`, which this function does no I/O to read.
pub(crate) fn build_player_summary(
    uid: Uuid,
    save_parameter: &crate::ue::Properties,
    player_guild_map: &HashMap<Uuid, Uuid>,
    pal_owner_counts: &HashMap<Uuid, i64>,
    last_online_time: Option<chrono::NaiveDateTime>,
) -> PlayerSummary {
    let nickname = player_nickname(uid, save_parameter);

    let level = props::get(save_parameter, &["Level"])
        .and_then(props::as_byte)
        .map(i64::from);

    PlayerSummary {
        uid,
        nickname,
        level,
        guild_id: player_guild_map.get(&uid).copied(),
        pal_count: pal_owner_counts.get(&uid).copied().unwrap_or(0),
        last_online_time: last_online_time.map(IsoDateTime),
        loaded: false,
    }
}

/// A player save's `Timestamp` (.NET ticks) as a datetime. Zero ticks means
/// "never online", not the year 1 -- a save that has never been played writes
/// `0` rather than omitting the property.
fn last_online_time_from_root(properties: &crate::ue::Properties) -> Option<chrono::NaiveDateTime> {
    props::get(properties, &["Timestamp"])
        .and_then(props::as_datetime_ticks)
        .filter(|&ticks| ticks != 0)
        .and_then(ticks_to_datetime)
}

/// Any parse failure collapses to `(None, None)`: one corrupt player file
/// must not fail the whole save load.
fn parse_player_save_and_timestamp(
    sav_bytes: &[u8],
) -> (Option<crate::ue::Save>, Option<chrono::NaiveDateTime>) {
    let Ok(save) = parse_palworld_save(sav_bytes) else {
        return (None, None);
    };
    let last_online_time = last_online_time_from_root(&save.root.properties);
    (Some(save), last_online_time)
}

/// Worker-container ids for every base belonging to `guild_id`. A base whose
/// `WorkerDirector` blob fails to decode contributes no container id rather
/// than aborting the count.
fn guild_worker_container_ids(base_camp_entries: &[crate::ue::MapEntry], guild_id: Uuid) -> Vec<Uuid> {
    let mut container_ids = Vec::new();
    for base_entry in base_camp_entries {
        let Some(value_properties) = props::struct_properties(&base_entry.value) else {
            continue;
        };
        let Some(crate::ue::Property::Struct(crate::ue::StructValue::Game(crate::ue::PalStruct::BaseCamp(camp)))) =
            props::get(value_properties, &["RawData"])
        else {
            continue;
        };
        if props::fguid_to_uuid(&camp.group_id_belong_to) != guild_id {
            continue;
        }
        let Some(worker_blob) = props::get(value_properties, &["WorkerDirector", "RawData"])
            .and_then(props::as_byte_array)
        else {
            continue;
        };
        if let Ok(container_id) = palbin::worker_director_container_id(worker_blob) {
            container_ids.push(container_id);
        }
    }
    container_ids
}

fn base_count_for_guild(base_camp_entries: &[crate::ue::MapEntry], guild_id: Uuid) -> i64 {
    base_camp_entries
        .iter()
        .filter(|base_entry| {
            props::struct_properties(&base_entry.value)
                .and_then(|value_properties| props::get(value_properties, &["RawData"]))
                .and_then(|raw_data| match raw_data {
                    crate::ue::Property::Struct(crate::ue::StructValue::Game(crate::ue::PalStruct::BaseCamp(camp))) => {
                        Some(props::fguid_to_uuid(&camp.group_id_belong_to))
                    }
                    _ => None,
                })
                == Some(guild_id)
        })
        .count() as i64
}

/// Pals working at any of `guild_id`'s bases: those slotted into one of its
/// bases' worker containers.
fn count_guild_base_pals(
    base_camp_entries: Option<&[crate::ue::MapEntry]>,
    character_entries: &[crate::ue::MapEntry],
    guild_id: Uuid,
) -> i64 {
    let Some(base_camp_entries) = base_camp_entries.filter(|entries| !entries.is_empty()) else {
        return 0;
    };
    if character_entries.is_empty() {
        return 0;
    }
    let container_ids = guild_worker_container_ids(base_camp_entries, guild_id);

    character_entries
        .iter()
        .filter(|entry| {
            let Some(parameters) = save_parameter(entry) else {
                return false;
            };
            if is_player_entry(parameters) {
                return false;
            }
            props::get(parameters, &["SlotId", "ContainerId", "ID"])
                .and_then(props::as_uuid)
                .is_some_and(|pal_container_id| container_ids.contains(&pal_container_id))
        })
        .count() as i64
}

/// No summary is built for the nil guild UUID.
///
/// The returned `Vec<Uuid>` records save-file encounter order, which the wire
/// `guilds` array must preserve — the `BTreeMap` itself always iterates in
/// `Uuid`-sorted order and cannot answer that question.
pub(crate) fn build_guild_summaries(
    group_entries: &[crate::ue::MapEntry],
    base_camp_entries: Option<&[crate::ue::MapEntry]>,
    character_entries: &[crate::ue::MapEntry],
) -> (BTreeMap<Uuid, GuildSummary>, Vec<Uuid>) {
    let mut summaries = BTreeMap::new();
    let mut order = Vec::new();
    for entry in group_entries {
        let Some((guild_id, guild)) = guild_tail_entry(entry) else {
            continue;
        };
        if guild_id.is_nil() {
            continue;
        }
        let base_count = base_camp_entries
            .map(|entries| base_count_for_guild(entries, guild_id))
            .unwrap_or(0);
        order.push(guild_id);
        summaries.insert(
            guild_id,
            GuildSummary {
                id: guild_id,
                name: guild.guild_name.clone(),
                admin_player_uid: Some(guild_tail::guild_admin_uid(guild)),
                player_count: guild_tail::guild_player_count(guild) as i64,
                base_count,
                level: Some(i64::from(guild.base_camp_level)),
                pal_count: count_guild_base_pals(base_camp_entries, character_entries, guild_id),
                loaded: false,
            },
        );
    }
    (summaries, order)
}

/// Single entry point, called once per save load.
///
/// Output is deterministic: the `HashMap`s built along the way are pure
/// lookup tables, never iterated for output. Save-file walk order is captured
/// into `player_summary_order`/`guild_summary_order` here, before the
/// `Uuid`-sorted `BTreeMap`s would lose it.
pub fn extract_summaries(
    session: &mut SaveSession,
    progress: &ProgressSink,
) -> Result<(), CoreError> {
    progress("Extracting player summaries...");

    // Both passes hold immutable borrows of `session`, so its fields are
    // assigned once, together, at the very end.
    let character_entries = session.character_map()?;
    let group_entries = session.group_map()?;

    let pal_owner_counts = build_pal_owner_counts(character_entries);
    let player_guild_map = build_player_guild_map(group_entries);

    let mut player_summaries = BTreeMap::new();
    let mut player_summary_order = Vec::new();
    let mut parsed_player_saves = Vec::new();
    let mut filtered_without_sav_count: usize = 0;
    for (uid, parameters) in collect_player_entries(character_entries) {
        // A character entry with no `.sav` file behind it is a ghost the game
        // cannot load; it gets no summary.
        let Some(file_ref) = session.player_file_refs.get(&uid) else {
            filtered_without_sav_count += 1;
            tracing::warn!(
                player_id = %uid,
                nickname = player_nickname(uid, parameters),
                "filtering out player - no .sav file reference"
            );
            continue;
        };

        let mut last_online_time = None;
        if let Ok(Some(sav_bytes)) = file_ref.sav_bytes() {
            let (parsed_save, timestamp) = parse_player_save_and_timestamp(&sav_bytes);
            last_online_time = timestamp;
            if let Some(parsed_save) = parsed_save {
                parsed_player_saves.push((uid, parsed_save));
            }
        }

        player_summary_order.push(uid);
        player_summaries.insert(
            uid,
            build_player_summary(
                uid,
                parameters,
                &player_guild_map,
                &pal_owner_counts,
                last_online_time,
            ),
        );
    }
    if filtered_without_sav_count > 0 {
        tracing::info!(
            filtered_count = filtered_without_sav_count,
            valid_count = player_summaries.len(),
            "filtered players without .sav files"
        );
    }

    progress("Extracting guild summaries...");
    let (guild_summaries, guild_summary_order) =
        build_guild_summaries(group_entries, session.base_camp_map(), character_entries);

    session.player_summaries = player_summaries;
    session.guild_summaries = guild_summaries;
    session.player_summary_order = player_summary_order;
    session.guild_summary_order = guild_summary_order;
    for (uid, parsed_save) in parsed_player_saves {
        // First parse wins: never evict a save the session already holds.
        session.player_sav_cache.entry(uid).or_insert(parsed_save);
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::palbin::test_bytes::shuffle_guid_bytes;
    use crate::ue::games::palworld::{
        PalBaseCamp, PalCharacterData, PalGroupData, PalGroupVariant, PalGuildGroup, PalTransform,
    };
    use crate::ue::{
        ByteArray, Double, MapEntry, Properties, Property, Quat, StructValue, ValueVec, Vector,
    };

    const NIL_UUID: &str = "00000000-0000-0000-0000-000000000000";
    const PLAYER_ONE: &str = "11111111-1111-1111-1111-111111111111";
    const PLAYER_TWO: &str = "22222222-2222-2222-2222-222222222222";
    const GUILD_ID: &str = "33333333-3333-3333-3333-333333333333";
    const BASE_ID: &str = "44444444-4444-4444-4444-444444444444";
    const CONTAINER_ID: &str = "55555555-5555-5555-5555-555555555555";

    fn fguid(text: &str) -> crate::ue::FGuid {
        serde_json::from_value(serde_json::Value::String(text.to_string())).unwrap()
    }

    fn guid_property(text: &str) -> Property {
        Property::Struct(StructValue::Guid(fguid(text)))
    }

    fn character_entry(
        player_uid: &str,
        instance_id: &str,
        save_parameter: Properties,
    ) -> MapEntry {
        let mut key_properties = Properties::default();
        key_properties.insert("PlayerUId", guid_property(player_uid));
        key_properties.insert("InstanceId", guid_property(instance_id));

        let mut object = Properties::default();
        object.insert(
            "SaveParameter",
            Property::Struct(StructValue::Struct(save_parameter)),
        );
        let character_data = PalCharacterData {
            object,
            unknown_bytes: [0; 4],
            group_id: crate::ue::FGuid::nil(),
            trailing_bytes: [0; 4],
        };
        let mut value_properties = Properties::default();
        value_properties.insert(
            "RawData",
            Property::Struct(StructValue::Game(crate::ue::PalStruct::CharacterData(character_data))),
        );

        MapEntry {
            key: Property::Struct(StructValue::Struct(key_properties)),
            value: Property::Struct(StructValue::Struct(value_properties)),
        }
    }

    fn player_character_entry(player_uid: &str, nickname: &str, level: u8) -> MapEntry {
        let mut save_parameter = Properties::default();
        save_parameter.insert("IsPlayer", Property::Bool(true));
        save_parameter.insert("NickName", Property::Str(nickname.to_string()));
        save_parameter.insert("Level", Property::Byte(crate::ue::Byte::Byte(level)));
        character_entry(player_uid, player_uid, save_parameter)
    }

    fn pal_character_entry(
        owner_uid: &str,
        instance_id: &str,
        container_id: Option<&str>,
    ) -> MapEntry {
        let mut save_parameter = Properties::default();
        save_parameter.insert("OwnerPlayerUId", guid_property(owner_uid));
        if let Some(container) = container_id {
            let mut id_properties = Properties::default();
            id_properties.insert("ID", guid_property(container));
            let mut slot_properties = Properties::default();
            slot_properties.insert(
                "ContainerId",
                Property::Struct(StructValue::Struct(id_properties)),
            );
            save_parameter.insert(
                "SlotId",
                Property::Struct(StructValue::Struct(slot_properties)),
            );
        }
        character_entry(NIL_UUID, instance_id, save_parameter)
    }

    fn guild_entry(guild_id: &str, guild: PalGuildGroup) -> MapEntry {
        let mut value_properties = Properties::default();
        value_properties.insert(
            "GroupType",
            Property::Enum("EPalGroupType::Guild".to_string()),
        );
        let group_data = PalGroupData {
            group_id: fguid(guild_id),
            group_name: String::new(),
            individual_character_handle_ids: vec![],
            data: PalGroupVariant::Guild(guild),
        };
        value_properties.insert(
            "RawData",
            Property::Struct(StructValue::Game(crate::ue::PalStruct::GroupData(group_data))),
        );
        MapEntry {
            key: guid_property(guild_id),
            value: Property::Struct(StructValue::Struct(value_properties)),
        }
    }

    /// String-keyed convenience wrapper over `guild_tail::pre_update_guild`.
    fn guild(
        base_camp_level: i32,
        guild_name: &str,
        admin_player_uid: &str,
        players: &[(&str, i64, &str)],
    ) -> PalGuildGroup {
        let players: Vec<(Uuid, i64, &str)> = players
            .iter()
            .map(|(uid, last_online, name)| (uid.parse().unwrap(), *last_online, *name))
            .collect();
        guild_tail::pre_update_guild(
            base_camp_level,
            guild_name,
            admin_player_uid.parse().unwrap(),
            &players,
        )
    }

    fn zero_transform() -> PalTransform {
        PalTransform {
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

    fn base_camp_entry(base_id: &str, guild_id: &str, worker_container_id: &str) -> MapEntry {
        let camp = PalBaseCamp {
            id: fguid(base_id),
            name: String::new(),
            state: 0,
            transform: zero_transform(),
            area_range: 0.0,
            group_id_belong_to: fguid(guild_id),
            fast_travel_local_transform: zero_transform(),
            owner_map_object_instance_id: crate::ue::FGuid::nil(),
            trailing_bytes: [0; 4],
        };
        let mut worker_blob = vec![0u8; 118];
        let display_bytes = *worker_container_id
            .parse::<uuid::Uuid>()
            .unwrap()
            .as_bytes();
        worker_blob[98..114].copy_from_slice(&shuffle_guid_bytes(display_bytes));

        let mut worker_properties = Properties::default();
        worker_properties.insert(
            "RawData",
            Property::Array(ValueVec::Byte(ByteArray::Byte(worker_blob))),
        );
        let mut value_properties = Properties::default();
        value_properties.insert(
            "RawData",
            Property::Struct(StructValue::Game(crate::ue::PalStruct::BaseCamp(Box::new(camp)))),
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

    #[test]
    fn test_is_player_and_save_parameter() {
        let player = player_character_entry(PLAYER_ONE, "Tester", 9);
        let pal = pal_character_entry(PLAYER_ONE, PLAYER_TWO, None);
        assert!(is_player_entry(save_parameter(&player).unwrap()));
        assert!(!is_player_entry(save_parameter(&pal).unwrap()));
    }

    #[test]
    fn test_build_player_guild_map() {
        let tail = guild(3, "The Guild", PLAYER_ONE, &[(PLAYER_ONE, 0, "Tester")]);
        let groups = vec![guild_entry(GUILD_ID, tail)];
        let map = build_player_guild_map(&groups);
        assert_eq!(
            GUILD_ID,
            map[&PLAYER_ONE.parse::<uuid::Uuid>().unwrap()].to_string()
        );
    }

    #[test]
    fn test_build_guild_summaries_counts() {
        let tail = guild(
            5,
            "The Guild",
            PLAYER_ONE,
            &[(PLAYER_ONE, 0, "Tester"), (PLAYER_TWO, 0, "Other")],
        );
        let groups = vec![guild_entry(GUILD_ID, tail)];
        let bases = vec![base_camp_entry(BASE_ID, GUILD_ID, CONTAINER_ID)];
        let characters = vec![
            player_character_entry(PLAYER_ONE, "Tester", 9),
            pal_character_entry(
                PLAYER_ONE,
                "aaaaaaaa-0000-0000-0000-000000000001",
                Some(CONTAINER_ID),
            ),
            pal_character_entry(
                PLAYER_ONE,
                "aaaaaaaa-0000-0000-0000-000000000002",
                Some(CONTAINER_ID),
            ),
            pal_character_entry(PLAYER_ONE, "aaaaaaaa-0000-0000-0000-000000000003", None),
        ];

        let (summaries, order) = build_guild_summaries(&groups, Some(&bases), &characters);
        let guild_id = GUILD_ID.parse::<uuid::Uuid>().unwrap();
        let guild = &summaries[&guild_id];
        assert_eq!("The Guild", guild.name);
        assert_eq!(Some(PLAYER_ONE.parse().unwrap()), guild.admin_player_uid);
        assert_eq!(2, guild.player_count);
        assert_eq!(1, guild.base_count);
        assert_eq!(Some(5), guild.level);
        assert_eq!(2, guild.pal_count); // only pals slotted into the base container
        assert!(!guild.loaded);
        assert_eq!(vec![guild_id], order);
    }

    #[test]
    fn test_player_summary_fields_without_file_parsing() {
        let characters = vec![
            player_character_entry(PLAYER_ONE, "Tester", 9),
            {
                // No NickName property → fallback name
                let mut save_parameter = Properties::default();
                save_parameter.insert("IsPlayer", Property::Bool(true));
                character_entry(PLAYER_TWO, PLAYER_TWO, save_parameter)
            },
            pal_character_entry(PLAYER_ONE, "aaaaaaaa-0000-0000-0000-000000000001", None),
        ];
        let tail = guild(1, "G", PLAYER_ONE, &[(PLAYER_ONE, 0, "Tester")]);
        let groups = vec![guild_entry(GUILD_ID, tail)];

        let guild_map = build_player_guild_map(&groups);
        let owner_counts = build_pal_owner_counts(&characters);
        let players = collect_player_entries(&characters);

        assert_eq!(2, players.len());
        let (first_uid, first_parameters) = &players[0];
        assert_eq!(PLAYER_ONE, first_uid.to_string());
        let first = build_player_summary(
            *first_uid,
            first_parameters,
            &guild_map,
            &owner_counts,
            None,
        );
        assert_eq!("Tester", first.nickname);
        assert_eq!(Some(9), first.level);
        assert_eq!(Some(GUILD_ID.parse().unwrap()), first.guild_id);
        assert_eq!(1, first.pal_count);
        assert!(first.last_online_time.is_none());

        let (second_uid, second_parameters) = &players[1];
        let second = build_player_summary(
            *second_uid,
            second_parameters,
            &guild_map,
            &owner_counts,
            None,
        );
        assert_eq!("Player (22222222)", second.nickname);
        assert_eq!(None, second.level);
        assert_eq!(0, second.pal_count);
    }
}

#[cfg(test)]
mod extraction_tests {
    use super::*;
    use std::sync::{Arc, Mutex};
    use crate::ue::{MapEntry, Properties, Property, StructValue};

    const PLAYER_ONE: &str = "11111111-1111-1111-1111-111111111111";

    fn fguid(text: &str) -> crate::ue::FGuid {
        serde_json::from_value(serde_json::Value::String(text.to_string())).unwrap()
    }

    fn guid_property(text: &str) -> Property {
        Property::Struct(StructValue::Guid(fguid(text)))
    }

    fn well_formed_pal_entry(owner_uid: &str, instance_id: &str) -> MapEntry {
        let mut save_parameter = Properties::default();
        save_parameter.insert("OwnerPlayerUId", guid_property(owner_uid));
        let mut object = Properties::default();
        object.insert(
            "SaveParameter",
            Property::Struct(StructValue::Struct(save_parameter)),
        );
        let character_data = crate::ue::games::palworld::PalCharacterData {
            object,
            unknown_bytes: [0; 4],
            group_id: crate::ue::FGuid::nil(),
            trailing_bytes: [0; 4],
        };
        let mut value_properties = Properties::default();
        value_properties.insert(
            "RawData",
            Property::Struct(StructValue::Game(crate::ue::PalStruct::CharacterData(character_data))),
        );
        let mut key_properties = Properties::default();
        key_properties.insert(
            "PlayerUId",
            guid_property("00000000-0000-0000-0000-000000000000"),
        );
        key_properties.insert("InstanceId", guid_property(instance_id));
        MapEntry {
            key: Property::Struct(StructValue::Struct(key_properties)),
            value: Property::Struct(StructValue::Struct(value_properties)),
        }
    }

    #[test]
    fn test_malformed_entries_are_skipped_without_panicking() {
        // A value that isn't a struct at all, and a struct whose `RawData` is
        // the wrong variant. Neither may panic.
        let not_a_struct_at_all = MapEntry {
            key: guid_property(PLAYER_ONE),
            value: Property::Bool(true),
        };
        let mut wrong_raw_data_variant_value = Properties::default();
        wrong_raw_data_variant_value.insert("RawData", Property::Bool(false));
        let wrong_raw_data_variant = MapEntry {
            key: guid_property(PLAYER_ONE),
            value: Property::Struct(StructValue::Struct(wrong_raw_data_variant_value)),
        };
        let good_pal = well_formed_pal_entry(PLAYER_ONE, "aaaaaaaa-0000-0000-0000-000000000001");

        let entries = vec![not_a_struct_at_all, wrong_raw_data_variant, good_pal];

        assert!(save_parameter(&entries[0]).is_none());
        assert!(save_parameter(&entries[1]).is_none());
        assert!(collect_player_entries(&entries).is_empty());

        let owner_counts = build_pal_owner_counts(&entries);
        assert_eq!(
            1,
            owner_counts
                .get(&PLAYER_ONE.parse::<Uuid>().unwrap())
                .copied()
                .unwrap_or(0)
        );
    }

    #[test]
    fn test_zero_ticks_yields_no_last_online_time() {
        let mut properties = Properties::default();
        properties.insert("Timestamp", Property::Struct(StructValue::DateTime(0)));
        assert!(last_online_time_from_root(&properties).is_none());
    }

    #[test]
    fn test_nonzero_ticks_yields_a_last_online_time() {
        let mut properties = Properties::default();
        properties.insert(
            "Timestamp",
            Property::Struct(StructValue::DateTime(638400000000000000)),
        );
        assert!(last_online_time_from_root(&properties).is_some());
    }

    #[test]
    fn test_missing_timestamp_yields_no_last_online_time() {
        assert!(last_online_time_from_root(&Properties::default()).is_none());
    }

    fn minimal_uesave_save(properties: crate::ue::Properties) -> crate::ue::Save {
        crate::ue::Save {
            header: crate::ue::Header {
                magic: 0,
                save_game_version: 0,
                package_version: crate::ue::PackageVersion { ue4: 0, ue5: None },
                engine_version_major: 0,
                engine_version_minor: 0,
                engine_version_patch: 0,
                engine_version_build: 0,
                engine_version: String::new(),
                custom_version: None,
            },
            schemas: crate::ue::PropertySchemas::default(),
            root: crate::ue::Root {
                save_game_type: String::new(),
                properties,
            },
            extra: Vec::new(),
        }
    }

    /// Empty but present required maps — enough for `extract_summaries` to run
    /// end to end without a real save file.
    fn minimal_empty_session() -> SaveSession {
        let mut world_save_data = Properties::default();
        world_save_data.insert("CharacterSaveParameterMap", Property::Map(Vec::new()));
        world_save_data.insert("GroupSaveDataMap", Property::Map(Vec::new()));
        let mut root_properties = Properties::default();
        root_properties.insert(
            "worldSaveData",
            Property::Struct(StructValue::Struct(world_save_data)),
        );

        let mut session = SaveSession::new_for_tests(
            crate::session::SaveKind::InMemory,
            minimal_uesave_save(root_properties),
        );
        session.world_name = "Test".to_string();
        session.save_id = "test".to_string();
        session
    }

    #[test]
    fn test_extract_summaries_emits_progress_messages_in_order_for_an_empty_save() {
        let mut session = minimal_empty_session();
        let log = Arc::new(Mutex::new(Vec::<String>::new()));
        let log_clone = Arc::clone(&log);
        let progress: ProgressSink = Arc::new(move |message: &str| {
            log_clone.lock().unwrap().push(message.to_string());
        });

        extract_summaries(&mut session, &progress).unwrap();

        assert!(session.player_summaries.is_empty());
        assert!(session.guild_summaries.is_empty());
        assert_eq!(
            vec![
                "Extracting player summaries...".to_string(),
                "Extracting guild summaries...".to_string(),
            ],
            *log.lock().unwrap()
        );
    }

    fn player_character_entry_with_file_ref(player_uid: &str, nickname: &str) -> MapEntry {
        let mut save_parameter = Properties::default();
        save_parameter.insert("IsPlayer", Property::Bool(true));
        save_parameter.insert("NickName", Property::Str(nickname.to_string()));
        let mut object = Properties::default();
        object.insert(
            "SaveParameter",
            Property::Struct(StructValue::Struct(save_parameter)),
        );
        let character_data = crate::ue::games::palworld::PalCharacterData {
            object,
            unknown_bytes: [0; 4],
            group_id: crate::ue::FGuid::nil(),
            trailing_bytes: [0; 4],
        };
        let mut value_properties = Properties::default();
        value_properties.insert(
            "RawData",
            Property::Struct(StructValue::Game(crate::ue::PalStruct::CharacterData(character_data))),
        );
        let mut key_properties = Properties::default();
        key_properties.insert("PlayerUId", guid_property(player_uid));
        key_properties.insert("InstanceId", guid_property(player_uid));
        MapEntry {
            key: Property::Struct(StructValue::Struct(key_properties)),
            value: Property::Struct(StructValue::Struct(value_properties)),
        }
    }

    /// The two players are laid out HIGH-then-LOW, deliberately the opposite
    /// of `Uuid`'s `Ord`, so encounter order and sorted order are genuinely
    /// different sequences and the assertions below are non-vacuous.
    #[test]
    fn test_extract_summaries_records_gvas_insertion_order_not_uuid_sorted_order() {
        const HIGH_UUID: &str = "ffffffff-ffff-ffff-ffff-ffffffffffff";
        const LOW_UUID: &str = "00000000-0000-0000-0000-000000000001";
        let high: Uuid = HIGH_UUID.parse().unwrap();
        let low: Uuid = LOW_UUID.parse().unwrap();

        let mut world_save_data = Properties::default();
        world_save_data.insert(
            "CharacterSaveParameterMap",
            Property::Map(vec![
                player_character_entry_with_file_ref(HIGH_UUID, "High"),
                player_character_entry_with_file_ref(LOW_UUID, "Low"),
            ]),
        );
        world_save_data.insert("GroupSaveDataMap", Property::Map(Vec::new()));
        let mut root_properties = Properties::default();
        root_properties.insert(
            "worldSaveData",
            Property::Struct(StructValue::Struct(world_save_data)),
        );

        let mut player_file_refs = BTreeMap::new();
        player_file_refs.insert(
            high,
            crate::session::PlayerFileData::Bytes {
                sav: None,
                dps: None,
            },
        );
        player_file_refs.insert(
            low,
            crate::session::PlayerFileData::Bytes {
                sav: None,
                dps: None,
            },
        );

        let mut session = minimal_empty_session();
        session.level = minimal_uesave_save(root_properties);
        session.player_file_refs = player_file_refs;

        extract_summaries(&mut session, &crate::progress::null_progress()).unwrap();

        assert_eq!(
            vec![high, low],
            session.player_summary_order,
            "player_summary_order must preserve GVAS encounter order (HIGH before LOW)"
        );
        assert_eq!(
            vec![low, high],
            session.player_summaries.keys().copied().collect::<Vec<_>>(),
            "the BTreeMap itself must still be Uuid-sorted (LOW before HIGH) -- \
             this is what makes the check above non-vacuous"
        );
    }
}
