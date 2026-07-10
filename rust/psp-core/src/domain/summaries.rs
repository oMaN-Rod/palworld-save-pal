//! Player/guild summary extraction — port of
//! `palworld_save_pal/game/mixins/summaries.py`'s sequential path. Python
//! switches to a thread pool above two players purely for throughput; that
//! path's map-insertion order is nondeterministic, which is exactly why the
//! parity harness (Task 10) restricts itself to <=2-player saves. This port
//! never parallelizes summary extraction, so it has no such gap to avoid.

use std::collections::{BTreeMap, HashMap};

use crate::dto::summary::{ticks_to_datetime, GuildSummary, IsoDateTime, PlayerSummary};
use crate::error::CoreError;
use crate::palbin::{self, GuildRawTail};
use crate::progress::ProgressSink;
use crate::props;
use crate::session::{parse_palworld_save, SaveSession};
use uuid::Uuid;

const GROUP_TYPE_GUILD: &str = "EPalGroupType::Guild";

/// `entry.value.RawData(.PalCharacterData).object.SaveParameter` — the
/// property bag every other accessor in this module reads from. Mirrors the
/// `try/except (KeyError, TypeError): continue` guard Python wraps around
/// this same chain (`_categorize_character_entries`, `_count_guild_base_pals`
/// in `mixins/summaries.py`): a missing or mistyped link anywhere in the
/// chain returns `None` rather than panicking — the character entry it
/// belongs to is simply skipped by the caller.
pub(crate) fn save_parameter(entry: &uesave::MapEntry) -> Option<&uesave::Properties> {
    let value_properties = props::struct_properties(&entry.value)?;
    let raw_data = props::get(value_properties, &["RawData"])?;
    let uesave::Property::Struct(uesave::StructValue::PalCharacterData(character_data)) = raw_data
    else {
        return None;
    };
    props::get(&character_data.object, &["SaveParameter"]).and_then(props::struct_properties)
}

/// Port of `SaveManager._is_player`, taking the already-resolved save
/// parameter bag rather than the raw map entry: `IsPlayer` bool, defaulting
/// to `false` when the property is absent.
pub(crate) fn is_player_entry(save_parameter: &uesave::Properties) -> bool {
    props::get(save_parameter, &["IsPlayer"])
        .and_then(props::as_bool)
        .unwrap_or(false)
}

/// `entry.key.PlayerUId` as a `Uuid`, if present and well-typed.
fn player_uid_from_key(entry: &uesave::MapEntry) -> Option<Uuid> {
    props::get_in(&entry.key, &["PlayerUId"]).and_then(props::as_uuid)
}

/// Locates a `GroupSaveDataMap` entry's decoded guild tail, for an entry
/// whose `GroupType` is `EPalGroupType::Guild` and whose `RawData` decodes
/// cleanly. Deliberately does **not** filter out a nil guild id: this
/// mirrors `_build_player_guild_index`, which only `continue`s when
/// `guild_id` extraction itself fails (`if not guild_id`) — a resolved nil
/// UUID is still a truthy Python `UUID` object and is kept. The *stricter*
/// nil check Python applies in `_extract_guild_summaries`
/// (`if not guild_id or is_empty_uuid(guild_id)`) lives only in
/// `build_guild_summaries`, not here — see that function.
fn guild_tail_entry(entry: &uesave::MapEntry) -> Option<(Uuid, GuildRawTail)> {
    let value_properties = props::struct_properties(&entry.value)?;
    let group_type = props::get(value_properties, &["GroupType"]).and_then(props::as_enum)?;
    if group_type != GROUP_TYPE_GUILD {
        return None;
    }
    let guild_id = props::as_uuid(&entry.key)?;
    let raw_data = props::get(value_properties, &["RawData"])?;
    let uesave::Property::Struct(uesave::StructValue::PalGroupData(group_data)) = raw_data else {
        return None;
    };
    let tail = palbin::parse_guild_raw_tail(&group_data.remaining_data).ok()?;
    Some((guild_id, tail))
}

/// Port of `_build_player_guild_index`.
pub(crate) fn build_player_guild_map(group_entries: &[uesave::MapEntry]) -> HashMap<Uuid, Uuid> {
    let mut player_guild_map = HashMap::new();
    for entry in group_entries {
        let Some((guild_id, tail)) = guild_tail_entry(entry) else {
            continue;
        };
        for player in &tail.players {
            player_guild_map.insert(player.player_uid, guild_id);
        }
    }
    player_guild_map
}

/// Port of the pal-owner-counting half of `_categorize_character_entries`:
/// every non-player character entry's `OwnerPlayerUId`, including the nil
/// UUID — Python's `if owner_uid:` never filters it out, since `UUID`
/// objects (even the nil one) are always truthy.
pub(crate) fn build_pal_owner_counts(character_entries: &[uesave::MapEntry]) -> HashMap<Uuid, i64> {
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

/// Port of the player half of `_categorize_character_entries`: player
/// character entries with a non-nil `PlayerUId`, paired with their save
/// parameter bag.
pub(crate) fn collect_player_entries(
    character_entries: &[uesave::MapEntry],
) -> Vec<(Uuid, &uesave::Properties)> {
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
/// `"Player (<uid8>)"`. Shared by `build_player_summary` and the
/// no-`.sav`-file diagnostic in `extract_summaries` so both derive the same
/// name Python's `_create_player_summary` computes.
fn player_nickname(uid: Uuid, save_parameter: &uesave::Properties) -> String {
    props::get(save_parameter, &["NickName"])
        .and_then(props::as_str)
        .filter(|name| !name.is_empty())
        .map(str::to_owned)
        .unwrap_or_else(|| format!("Player ({})", &uid.to_string()[..8]))
}

/// Port of `_create_player_summary`'s pure part: nickname fallback, level,
/// guild link, pal count. The caller supplies `last_online_time` because
/// parsing the player's own `.sav` is I/O this function has no business
/// doing — that stays a distinct step in `extract_summaries`, matching
/// `_parse_player_gvas_and_timestamp` being a free function Python calls
/// separately from `_create_player_summary`'s own logic.
pub(crate) fn build_player_summary(
    uid: Uuid,
    save_parameter: &uesave::Properties,
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

/// Extracts a player save's `Timestamp` (.NET ticks) as a datetime, applying
/// the zero-tick guard `_parse_player_gvas_and_timestamp` applies *before*
/// calling `ticks_to_datetime` (`if not ticks: return gvas_file, None`):
/// ticks of `0`, and any missing/mistyped `Timestamp` property, both mean
/// "no timestamp". The guard belongs here, the caller — never inside
/// `ticks_to_datetime` itself (see that function's doc comment).
fn last_online_time_from_root(properties: &uesave::Properties) -> Option<chrono::NaiveDateTime> {
    props::get(properties, &["Timestamp"])
        .and_then(props::as_datetime_ticks)
        .filter(|&ticks| ticks != 0)
        .and_then(ticks_to_datetime)
}

/// Port of `_parse_player_gvas_and_timestamp`: parses a player `.sav`,
/// returning the parsed save (for `player_sav_cache`) alongside its
/// `Timestamp`. Any parse failure collapses to `(None, None)`, matching
/// Python's blanket `except Exception: return None, None`.
fn parse_player_save_and_timestamp(
    sav_bytes: &[u8],
) -> (Option<uesave::Save>, Option<chrono::NaiveDateTime>) {
    let Ok(save) = parse_palworld_save(sav_bytes) else {
        return (None, None);
    };
    let last_online_time = last_online_time_from_root(&save.root.properties);
    (Some(save), last_online_time)
}

/// Collects worker-container ids for every base belonging to `guild_id`
/// (the first loop of `_count_guild_base_pals`). A base whose
/// `WorkerDirector` blob fails to decode simply contributes no container
/// id, rather than aborting the whole count.
fn guild_worker_container_ids(base_camp_entries: &[uesave::MapEntry], guild_id: Uuid) -> Vec<Uuid> {
    let mut container_ids = Vec::new();
    for base_entry in base_camp_entries {
        let Some(value_properties) = props::struct_properties(&base_entry.value) else {
            continue;
        };
        let Some(uesave::Property::Struct(uesave::StructValue::PalBaseCamp(camp))) =
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

/// Port of `_extract_guild_summaries`'s `base_count` computation.
fn base_count_for_guild(base_camp_entries: &[uesave::MapEntry], guild_id: Uuid) -> i64 {
    base_camp_entries
        .iter()
        .filter(|base_entry| {
            props::struct_properties(&base_entry.value)
                .and_then(|value_properties| props::get(value_properties, &["RawData"]))
                .and_then(|raw_data| match raw_data {
                    uesave::Property::Struct(uesave::StructValue::PalBaseCamp(camp)) => {
                        Some(props::fguid_to_uuid(&camp.group_id_belong_to))
                    }
                    _ => None,
                })
                == Some(guild_id)
        })
        .count() as i64
}

/// Port of `_count_guild_base_pals`.
fn count_guild_base_pals(
    base_camp_entries: Option<&[uesave::MapEntry]>,
    character_entries: &[uesave::MapEntry],
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

/// Port of `_extract_guild_summaries`. Unlike `build_player_guild_map`,
/// this explicitly filters out the nil guild UUID
/// (`if not guild_id or is_empty_uuid(guild_id): continue`) — no summary is
/// ever built for it.
///
/// Returns the `BTreeMap` alongside a `Vec<Uuid>` recording the order guild
/// ids were actually inserted in (`group_entries`' own order, filtered) —
/// Python's `_guild_summaries` dict preserves exactly this insertion order,
/// and `sync_app_state_handler`'s wire `guilds` array reflects it (see
/// `session.rs`'s `guild_summary_order` doc comment). The map alone cannot
/// answer that question once entries are inserted, since `BTreeMap` always
/// iterates in `Uuid`-sorted order regardless of insertion order.
pub(crate) fn build_guild_summaries(
    group_entries: &[uesave::MapEntry],
    base_camp_entries: Option<&[uesave::MapEntry]>,
    character_entries: &[uesave::MapEntry],
) -> (BTreeMap<Uuid, GuildSummary>, Vec<Uuid>) {
    let mut summaries = BTreeMap::new();
    let mut order = Vec::new();
    for entry in group_entries {
        let Some((guild_id, tail)) = guild_tail_entry(entry) else {
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
                name: tail.guild_name.clone(),
                admin_player_uid: Some(tail.admin_player_uid),
                player_count: tail.players.len() as i64,
                base_count,
                level: Some(i64::from(tail.base_camp_level)),
                pal_count: count_guild_base_pals(base_camp_entries, character_entries, guild_id),
                loaded: false,
            },
        );
    }
    (summaries, order)
}

/// Single entry point: extracts player summaries, then guild summaries, in
/// that order — matching `AppState.process_save_files`'s progress-message
/// sequence (`"Extracting player summaries..."` then
/// `"Extracting guild summaries..."`). `SaveSession::load` (Task 7) calls
/// this once, after building its typed indexes.
///
/// Iteration order throughout is deterministic: `character_entries` and
/// `group_entries` are plain slices walked in save-file order, and the two
/// output maps (`session.player_summaries`, `session.guild_summaries`) are
/// `BTreeMap`s keyed by `Uuid`. The `HashMap`s built along the way
/// (`pal_owner_counts`, `player_guild_map`) are pure lookup tables — never
/// iterated for output — so their unordered iteration never reaches the
/// wire. No parallelism is used, unlike Python's >2-player thread pool.
///
/// The save-file walk order is ALSO recorded verbatim into
/// `session.player_summary_order` / `session.guild_summary_order` — see
/// `session.rs`'s doc comment on those fields for why: Python's
/// `sync_app_state_handler` emits `player_summaries`/`guild_summaries` dict
/// insertion order on the wire, not a `Uuid` sort, and this is where that
/// order is captured before it would otherwise be lost to `BTreeMap`'s
/// sorted iteration.
pub fn extract_summaries(
    session: &mut SaveSession,
    progress: &ProgressSink,
) -> Result<(), CoreError> {
    progress("Extracting player summaries...");

    // `character_entries` and `group_entries` are resolved once and shared by
    // both the player- and guild-summary passes below; both passes only ever
    // borrow `session` immutably (`player_file_refs`, `base_camp_map`), so
    // `session`'s fields are assigned once, together, at the very end.
    let character_entries = session.character_map()?;
    let group_entries = session.group_map()?;

    let pal_owner_counts = build_pal_owner_counts(character_entries);
    let player_guild_map = build_player_guild_map(group_entries);

    let mut player_summaries = BTreeMap::new();
    let mut player_summary_order = Vec::new();
    let mut parsed_player_saves = Vec::new();
    let mut filtered_without_sav_count: usize = 0;
    for (uid, parameters) in collect_player_entries(character_entries) {
        // Mirrors `get_player_summaries`'s filter: in this port, a player
        // entry with no `.sav` file reference never gets a summary at all
        // (the reconciled `SaveSession.player_summaries` holds only the
        // filtered map Python's `AppState` ends up with). Python logs a
        // per-player warning plus an aggregate count for this same filter
        // (`SummariesMixin.get_player_summaries`); this port emits the
        // equivalent diagnostics here, at the point the filter is actually
        // applied.
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
        // Python: `if parsed_gvas is not None and uid not in
        // self._player_gvas_sav_cache: self._player_gvas_sav_cache[uid] =
        // parsed_gvas` — first parse wins, matched by `or_insert`.
        session.player_sav_cache.entry(uid).or_insert(parsed_save);
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::palbin::test_bytes::{guild_tail, shuffle_guid_bytes};
    use uesave::games::palworld::{PalBaseCamp, PalCharacterData, PalGroupData, PalTransform};
    use uesave::{
        ByteArray, Double, MapEntry, Properties, Property, Quat, StructValue, ValueVec, Vector,
    };

    const NIL_UUID: &str = "00000000-0000-0000-0000-000000000000";
    const PLAYER_ONE: &str = "11111111-1111-1111-1111-111111111111";
    const PLAYER_TWO: &str = "22222222-2222-2222-2222-222222222222";
    const GUILD_ID: &str = "33333333-3333-3333-3333-333333333333";
    const BASE_ID: &str = "44444444-4444-4444-4444-444444444444";
    const CONTAINER_ID: &str = "55555555-5555-5555-5555-555555555555";

    fn fguid(text: &str) -> uesave::FGuid {
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
            group_id: uesave::FGuid::nil(),
            trailing_bytes: [0; 4],
        };
        let mut value_properties = Properties::default();
        value_properties.insert(
            "RawData",
            Property::Struct(StructValue::PalCharacterData(character_data)),
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
        save_parameter.insert("Level", Property::Byte(uesave::Byte::Byte(level)));
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

    fn guild_entry(guild_id: &str, tail: Vec<u8>) -> MapEntry {
        let mut value_properties = Properties::default();
        value_properties.insert(
            "GroupType",
            Property::Enum("EPalGroupType::Guild".to_string()),
        );
        let group_data = PalGroupData {
            group_id: fguid(guild_id),
            group_name: String::new(),
            individual_character_handle_ids: vec![],
            remaining_data: tail,
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
            owner_map_object_instance_id: uesave::FGuid::nil(),
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

    #[test]
    fn test_is_player_and_save_parameter() {
        let player = player_character_entry(PLAYER_ONE, "Tester", 9);
        let pal = pal_character_entry(PLAYER_ONE, PLAYER_TWO, None);
        assert!(is_player_entry(save_parameter(&player).unwrap()));
        assert!(!is_player_entry(save_parameter(&pal).unwrap()));
    }

    #[test]
    fn test_build_player_guild_map() {
        let tail = guild_tail(3, "The Guild", PLAYER_ONE, &[(PLAYER_ONE, 0, "Tester")]);
        let groups = vec![guild_entry(GUILD_ID, tail)];
        let map = build_player_guild_map(&groups);
        assert_eq!(
            GUILD_ID,
            map[&PLAYER_ONE.parse::<uuid::Uuid>().unwrap()].to_string()
        );
    }

    #[test]
    fn test_build_guild_summaries_counts() {
        let tail = guild_tail(
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
        // Build the summary parts directly (no SaveSession) to check
        // nickname fallback, level, guild link and pal counts.
        let characters = vec![
            player_character_entry(PLAYER_ONE, "Tester", 9),
            {
                // Player with no NickName property → fallback name
                let mut save_parameter = Properties::default();
                save_parameter.insert("IsPlayer", Property::Bool(true));
                character_entry(PLAYER_TWO, PLAYER_TWO, save_parameter)
            },
            pal_character_entry(PLAYER_ONE, "aaaaaaaa-0000-0000-0000-000000000001", None),
        ];
        let tail = guild_tail(1, "G", PLAYER_ONE, &[(PLAYER_ONE, 0, "Tester")]);
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

/// Coverage beyond the brief's prescribed test module (standing policy:
/// strengthen tests the brief left thin or missing, rather than logging the
/// gap as a footnote): a malformed character entry must be skipped, not
/// panic; a zero-tick `Timestamp` must yield no `last_online_time`; and
/// `extract_summaries`'s progress messages must fire in the documented
/// order even for an empty save.
#[cfg(test)]
mod extraction_tests {
    use super::*;
    use std::sync::{Arc, Mutex};
    use uesave::{MapEntry, Properties, Property, StructValue};

    const PLAYER_ONE: &str = "11111111-1111-1111-1111-111111111111";

    fn fguid(text: &str) -> uesave::FGuid {
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
        let character_data = uesave::games::palworld::PalCharacterData {
            object,
            unknown_bytes: [0; 4],
            group_id: uesave::FGuid::nil(),
            trailing_bytes: [0; 4],
        };
        let mut value_properties = Properties::default();
        value_properties.insert(
            "RawData",
            Property::Struct(StructValue::PalCharacterData(character_data)),
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
        // Untrusted save data: a value that isn't even a struct (so
        // `save_parameter` can't resolve `RawData` at all), and a struct
        // whose `RawData` is present but is the wrong StructValue variant.
        // Neither should panic; both should simply be excluded from the
        // counts and the collected player list.
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

    fn minimal_uesave_save(properties: uesave::Properties) -> uesave::Save {
        uesave::Save {
            header: uesave::Header {
                magic: 0,
                save_game_version: 0,
                package_version: uesave::PackageVersion { ue4: 0, ue5: None },
                engine_version_major: 0,
                engine_version_minor: 0,
                engine_version_patch: 0,
                engine_version_build: 0,
                engine_version: String::new(),
                custom_version: None,
            },
            schemas: uesave::PropertySchemas::default(),
            root: uesave::Root {
                save_game_type: String::new(),
                properties,
            },
            extra: Vec::new(),
        }
    }

    /// A `SaveSession` with empty (but present) required maps — enough for
    /// `extract_summaries` to run end to end without a real save file.
    fn minimal_empty_session() -> SaveSession {
        let mut world_save_data = Properties::default();
        world_save_data.insert("CharacterSaveParameterMap", Property::Map(Vec::new()));
        world_save_data.insert("GroupSaveDataMap", Property::Map(Vec::new()));
        let mut root_properties = Properties::default();
        root_properties.insert(
            "worldSaveData",
            Property::Struct(StructValue::Struct(world_save_data)),
        );

        SaveSession {
            kind: crate::session::SaveKind::InMemory,
            world_name: "Test".to_string(),
            level: minimal_uesave_save(root_properties),
            save_id: "test".to_string(),
            save_type_label: "steam",
            size: 0,
            level_meta: None,
            player_file_refs: BTreeMap::new(),
            player_sav_cache: HashMap::new(),
            player_summaries: BTreeMap::new(),
            guild_summaries: BTreeMap::new(),
            player_summary_order: Vec::new(),
            guild_summary_order: Vec::new(),
            character_index: HashMap::new(),
            item_container_index: HashMap::new(),
            character_container_index: HashMap::new(),
            group_index: HashMap::new(),
            guild_extra_index: HashMap::new(),
            gps_file_path: None,
            gps_loaded: false,
        }
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
        let character_data = uesave::games::palworld::PalCharacterData {
            object,
            unknown_bytes: [0; 4],
            group_id: uesave::FGuid::nil(),
            trailing_bytes: [0; 4],
        };
        let mut value_properties = Properties::default();
        value_properties.insert(
            "RawData",
            Property::Struct(StructValue::PalCharacterData(character_data)),
        );
        let mut key_properties = Properties::default();
        key_properties.insert("PlayerUId", guid_property(player_uid));
        key_properties.insert("InstanceId", guid_property(player_uid));
        MapEntry {
            key: Property::Struct(StructValue::Struct(key_properties)),
            value: Property::Struct(StructValue::Struct(value_properties)),
        }
    }

    /// The test that actually discriminates this fix from the pre-fix
    /// `session.player_summaries.keys()` behavior (which the handler used
    /// before `player_summary_order`/`guild_summary_order` existed): the
    /// two players below are inserted in `CharacterSaveParameterMap` order
    /// HIGH-then-LOW, deliberately the opposite of `Uuid`'s `Ord` — so a
    /// `BTreeMap<Uuid, _>::keys()` read would report LOW-then-HIGH instead.
    /// `extract_summaries` must record `player_summary_order` as the
    /// as-encountered HIGH-then-LOW order, while `player_summaries` itself
    /// stays sorted (LOW-then-HIGH) — proving the two are genuinely
    /// different sequences, not a vacuous check that happens to coincide.
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
