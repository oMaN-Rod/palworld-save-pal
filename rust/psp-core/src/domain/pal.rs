//! Pal read side — port of `game/pal.py`'s `Pal` getters (Level.sav pals and
//! DPS-array pals) and `SummariesMixin.get_pal_summaries`
//! (`game/mixins/summaries.py`). Every default below is copied from the
//! Python property it ports; see each field's own comment for the exact
//! source. `Rank` defaults to `0` in the full `Pal` dump but `1` in
//! `PalSummary` — both are correct, not a typo (see
//! `pal_summaries`/`read_save_parameter_dto`).
//!
//! Untrusted input: a malformed pal entry (missing `SaveParameter`, wrong-
//! typed `RawData`, ...) is skipped, never a panic — matching Python's own
//! `PalObjects.get_nested`/`try/except (KeyError, TypeError): continue`
//! guards throughout `game/mixins/loading.py` and `summaries.py`.

use std::collections::HashSet;

use uesave::{MapEntry, Properties, Property, PropertyKey, StructValue};

use crate::dto::ordered_map::OrderedMap;
use crate::dto::pal::{format_character_key, PalDto, PalGender, WORK_SUITABILITIES};
use crate::dto::summary::PalSummary;
use crate::error::CoreError;
use crate::gamedata::GameData;
use crate::props;
use crate::session::SaveSession;

use super::world;

/// Look up a top-level property inside a pal/player `SaveParameter` bag by
/// name — every accessor in this module reads through this one function so
/// a missing key uniformly resolves to `None` rather than panicking.
pub(crate) fn param<'a>(save_parameter: &'a Properties, name: &str) -> Option<&'a Property> {
    save_parameter.0.get(&PropertyKey::from(name))
}

/// Port of `game/utils.py::get_pal_data`'s backing set: every key in
/// `data/json/pals.json`, used by `format_character_key` to decide whether a
/// `BOSS_`-prefixed id names a real "boss variant is its own catalog entry"
/// pal (keep the prefix) or an ordinary pal spawned as a boss (strip it).
pub fn known_pal_keys(game_data: &GameData) -> HashSet<String> {
    game_data
        .get("pals")
        .and_then(|value| value.as_object())
        .map(|object| object.keys().cloned().collect())
        .unwrap_or_default()
}

/// `PAL_SICK_TYPES` (`game/pal.py`), minus `HungerType`/`SanityValue`: `Pal.
/// is_sick` checks membership of exactly these three keys
/// (`any(t in self._save_parameter for t in PAL_SICK_TYPES if t not in
/// ["HungerType", "SanityValue"])`), and is unconditionally `False` for DPS
/// pals.
const SICK_MARKERS: [&str; 3] = ["PalReviveTimer", "PhysicalHealth", "WorkerSick"];

/// Port of `Pal`'s full computed-field dump (`game/pal.py`), applied to an
/// already-resolved `SaveParameter` property bag. Shared by both call sites
/// that own such a bag: `pal_dto_from_entry` (Level.sav pals, `is_dps:
/// false`) and `pal_dto_from_dps_slot` (GPS/DPS-array pals, `is_dps: true`).
pub fn read_save_parameter_dto(
    save_parameter: &Properties,
    instance_id: uuid::Uuid,
    is_dps: bool,
    game_data: &GameData,
) -> PalDto {
    // `character_id` (game/pal.py Pal.character_id): "" when CharacterID is
    // absent. Python's own getter returns `None` here and several other
    // properties (`is_boss`, `is_predator`, `is_tower`) call `.upper()`/
    // `.startswith()` on it unconditionally -- a `None` would raise in
    // Python too. A missing CharacterID on a real character-map entry is
    // pathological (no game data ever writes one), so this only matters for
    // adversarial/corrupted input; "" keeps every downstream prefix check
    // total (never panics) while still producing the same answer ("no boss
    // prefix", "no predator prefix", ...) Python's crash path would never
    // let you observe anyway.
    let character_id = param(save_parameter, "CharacterID")
        .and_then(props::as_str)
        .unwrap_or("")
        .to_string();

    // `is_lucky` (Pal.is_lucky): false when IsRarePal absent.
    let is_lucky = param(save_parameter, "IsRarePal")
        .and_then(props::as_bool)
        .unwrap_or(false);
    // `is_boss` (Pal.is_boss): character_id.upper().startswith("BOSS_") and not is_lucky.
    let is_boss = character_id.to_uppercase().starts_with("BOSS_") && !is_lucky;

    // `gender` (Pal.gender): defaults to Female when Gender absent
    // (PalGender.FEMALE.prefixed() is fed through the same from_value parse
    // Python applies to a present value).
    let gender = param(save_parameter, "Gender")
        .and_then(props::as_str)
        .map(PalGender::from_prefixed)
        .unwrap_or(PalGender::Female);

    // `storage_slot`/`storage_id` (Pal.storage_slot/storage_id): both check
    // "SlotID" first, falling back to "SlotId" (game/pal.py: `slot_id_key =
    // "SlotID" if "SlotID" in self._save_parameter else "SlotId"`).
    let slot_property = param(save_parameter, "SlotID").or_else(|| param(save_parameter, "SlotId"));
    let (storage_id, storage_slot) = slot_property
        .and_then(props::struct_props)
        .map(|slot| {
            let container_id = slot
                .0
                .get(&PropertyKey::from("ContainerId"))
                .and_then(props::struct_props)
                .and_then(|container| container.0.get(&PropertyKey::from("ID")))
                .and_then(props::as_uuid)
                .unwrap_or(props::EMPTY_UUID);
            let index = slot
                .0
                .get(&PropertyKey::from("SlotIndex"))
                .and_then(props::as_i32)
                .unwrap_or(0) as i64;
            (container_id, index)
        })
        .unwrap_or((props::EMPTY_UUID, 0));

    // `work_suitability` (Pal.work_suitability): {} when
    // GotWorkSuitabilityAddRankList absent; otherwise one entry per element
    // whose WorkSuitability enum value is a recognized bare name.
    let mut work_suitability: OrderedMap<String, i64> = OrderedMap::new();
    if let Some(rank_list) =
        param(save_parameter, "GotWorkSuitabilityAddRankList").and_then(props::struct_values)
    {
        for rank_entry in rank_list {
            let StructValue::Struct(rank_props) = rank_entry else {
                continue;
            };
            let Some(work_name) = rank_props
                .0
                .get(&PropertyKey::from("WorkSuitability"))
                .and_then(props::as_str)
            else {
                continue;
            };
            let bare = work_name.trim_start_matches("EPalWorkSuitability::");
            if !WORK_SUITABILITIES.contains(&bare) {
                continue;
            }
            let rank = rank_props
                .0
                .get(&PropertyKey::from("Rank"))
                .and_then(props::as_i32)
                .unwrap_or(0) as i64;
            work_suitability.insert(bare.to_string(), rank);
        }
    }

    // `hp` (Pal.hp): checks "Hp" first; Python additionally migrates a
    // legacy "HP" key into "Hp" as a side effect of reading it. This port
    // never mutates the save tree from a read accessor, so it simply reads
    // whichever of the two is present, "Hp" taking priority -- the same
    // precedence Python's migrate-then-read produces.
    let hp = param(save_parameter, "Hp")
        .or_else(|| param(save_parameter, "HP"))
        .and_then(props::fixed_point64)
        .unwrap_or(0);

    // `nickname` (Pal.nickname): None when NickName absent, for every pal.
    let nickname = param(save_parameter, "NickName")
        .and_then(props::as_str)
        .map(str::to_string);
    // `filtered_nickname` (Pal.filtered_nickname): only ever populated for
    // DPS pals, and only when FilteredNickName is present.
    let filtered_nickname = if is_dps {
        param(save_parameter, "FilteredNickName")
            .and_then(props::as_str)
            .map(str::to_string)
    } else {
        None
    };

    let mut dto = PalDto {
        instance_id,
        owner_uid: param(save_parameter, "OwnerPlayerUId").and_then(props::as_uuid),
        character_key: format_character_key(&character_id, &known_pal_keys(game_data)),
        is_lucky: Some(is_lucky),
        is_boss: Some(is_boss),
        // `is_predator` (Pal.is_predator): startswith("PREDATOR_") if character_id else False.
        is_predator: character_id.starts_with("PREDATOR_"),
        gender,
        // Rank_HP/Rank_Attack/Rank_Defence/Rank_CraftSpeed (Pal.rank_hp/rank_attack/
        // rank_defense/rank_craftspeed): 0 when absent.
        rank_hp: param(save_parameter, "Rank_HP")
            .and_then(props::as_byte_number)
            .unwrap_or(0) as i64,
        rank_attack: param(save_parameter, "Rank_Attack")
            .and_then(props::as_byte_number)
            .unwrap_or(0) as i64,
        rank_defense: param(save_parameter, "Rank_Defence")
            .and_then(props::as_byte_number)
            .unwrap_or(0) as i64,
        rank_craftspeed: param(save_parameter, "Rank_CraftSpeed")
            .and_then(props::as_byte_number)
            .unwrap_or(0) as i64,
        // Talent_HP/Talent_Shot/Talent_Defense (Pal.talent_hp/talent_shot/talent_defense): 0 when absent.
        talent_hp: param(save_parameter, "Talent_HP")
            .and_then(props::as_byte_number)
            .unwrap_or(0) as i64,
        talent_shot: param(save_parameter, "Talent_Shot")
            .and_then(props::as_byte_number)
            .unwrap_or(0) as i64,
        talent_defense: param(save_parameter, "Talent_Defense")
            .and_then(props::as_byte_number)
            .unwrap_or(0) as i64,
        // Rank (Pal.rank): 0 when absent -- the full dump's default, NOT
        // the same as PalSummary's (1); see pal_summaries below.
        rank: param(save_parameter, "Rank")
            .and_then(props::as_byte_number)
            .unwrap_or(0) as i64,
        // Level (Pal.level): 1 when absent.
        level: param(save_parameter, "Level")
            .and_then(props::as_byte_number)
            .unwrap_or(1) as i64,
        // Exp (Pal.exp): 0 when absent.
        exp: param(save_parameter, "Exp")
            .and_then(props::as_i64)
            .unwrap_or(0),
        nickname,
        filtered_nickname,
        // is_tower (Pal.is_tower): startswith("GYM_") if character_id else False.
        is_tower: character_id.starts_with("GYM_"),
        storage_id,
        // FullStomach (Pal.stomach): 150.0 when absent.
        stomach: param(save_parameter, "FullStomach")
            .and_then(props::as_f32)
            .unwrap_or(150.0) as f64,
        storage_slot,
        // MasteredWaza (Pal.learned_skills): [] when absent.
        learned_skills: param(save_parameter, "MasteredWaza")
            .and_then(props::enum_values)
            .cloned()
            .unwrap_or_default(),
        // EquipWaza (Pal.active_skills): [] when absent.
        active_skills: param(save_parameter, "EquipWaza")
            .and_then(props::enum_values)
            .cloned()
            .unwrap_or_default(),
        // PassiveSkillList (Pal.passive_skills): [] when absent.
        passive_skills: param(save_parameter, "PassiveSkillList")
            .and_then(props::name_values)
            .cloned()
            .unwrap_or_default(),
        hp,
        max_hp: 0, // filled below, after `dto` exists (max_hp_for reads other dto fields)
        group_id: None, // filled by pal_dto_from_entry from PalCharacterData.group_id
        // SanityValue (Pal.sanity): 100.0 when absent.
        sanity: param(save_parameter, "SanityValue")
            .and_then(props::as_f32)
            .unwrap_or(100.0) as f64,
        work_suitability,
        // is_sick (Pal.is_sick): always false for DPS pals; otherwise true
        // iff any of the three SICK_MARKERS keys is present.
        is_sick: !is_dps
            && SICK_MARKERS
                .iter()
                .any(|marker| param(save_parameter, marker).is_some()),
        // FriendshipPoint (Pal.friendship_point): 0 when absent.
        friendship_point: param(save_parameter, "FriendshipPoint")
            .and_then(props::as_i32)
            .unwrap_or(0) as i64,
        character_id,
    };
    dto.max_hp = max_hp_for(&dto, game_data);
    dto
}

/// Port of `Pal.max_hp` (`game/pal.py`): falls back to `dto.hp` when the pal
/// isn't recognized or has no `scaling.hp` entry in `pals.json` -- the same
/// fallback Python's `if not self.character_key or not self.pal_data:
/// return self.hp` / `if not hp_scaling: return self.hp` apply.
pub fn max_hp_for(dto: &PalDto, game_data: &GameData) -> i64 {
    let keys = known_pal_keys(game_data);
    let pal_key = format_character_key(&dto.character_id, &keys);
    if pal_key.is_empty() {
        return dto.hp;
    }
    let Some(pals_json) = game_data.get("pals").and_then(|v| v.as_object()) else {
        return dto.hp;
    };
    let Some(pal_data) = pals_json
        .iter()
        .find(|(key, _)| key.to_lowercase() == pal_key)
        .map(|(_, value)| value)
    else {
        return dto.hp;
    };
    let Some(hp_scaling) = pal_data.pointer("/scaling/hp").and_then(|v| v.as_f64()) else {
        return dto.hp;
    };
    let condenser_bonus = (dto.rank as f64 - 1.0) * 0.05;
    let hp_iv = dto.talent_hp as f64 * 0.3 / 100.0;
    let hp_soul_bonus = dto.rank_hp as f64 * 0.03;
    let alpha_scaling = if dto.is_boss.unwrap_or(false) || dto.is_lucky.unwrap_or(false) {
        1.2
    } else {
        1.0
    };
    let base = (500.0
        + 5.0 * dto.level as f64
        + hp_scaling * 0.5 * dto.level as f64 * (1.0 + hp_iv) * alpha_scaling)
        .floor();
    ((base * (1.0 + condenser_bonus) * (1.0 + hp_soul_bonus)).floor() as i64) * 1000
}

/// Port of `Pal(entry)` for a `CharacterSaveParameterMap` entry
/// (`game/mixins/loading.py`'s `_load_player_pals_only` and friends): `None`
/// when the entry isn't shaped like a pal at all (no resolvable
/// `InstanceId`, no `SaveParameter`, no `PalCharacterData`), matching
/// Python's `PalObjects.get_nested`/`try/except` guards -- the entry is
/// simply skipped by the caller, never a panic.
pub fn pal_dto_from_entry(entry: &MapEntry, game_data: &GameData) -> Option<PalDto> {
    let instance_id = world::entry_instance_id(entry)?;
    let save_parameter = world::entry_save_parameter(entry)?;
    let mut dto = read_save_parameter_dto(save_parameter, instance_id, false, game_data);
    let character_data = world::entry_character_data(entry)?;
    // `group_id` (Pal.group_id): only set when the underlying PalCharacterData
    // group_id is non-nil, matching PalObjects.as_uuid's "nil guid -> None"
    // contract on the read side.
    let group_id = props::guid_to_uuid(&character_data.group_id);
    dto.group_id = (group_id != props::EMPTY_UUID).then_some(group_id);
    Some(dto)
}

/// Port of `Pal(data=entry, dps=True)` for a GPS/DPS `SaveParameterArray`
/// element (`game/pal.py` `Pal.__init__`'s `dps=True` branch, `game/player.py`
/// `_load_dps`): a plain struct with a `"SaveParameter"` property and an
/// `"InstanceId"` struct holding an inner `"InstanceId"` guid -- unlike
/// Level.sav pals, no `RawData`/`PalCharacterData` wrapper. `None` when the
/// slot isn't shaped this way.
pub fn pal_dto_from_dps_slot(slot: &StructValue, game_data: &GameData) -> Option<PalDto> {
    let StructValue::Struct(slot_props) = slot else {
        return None;
    };
    let save_parameter =
        props::struct_props(slot_props.0.get(&PropertyKey::from("SaveParameter"))?)?;
    let instance_id = slot_props
        .0
        .get(&PropertyKey::from("InstanceId"))
        .and_then(props::struct_props)
        .and_then(|inner| inner.0.get(&PropertyKey::from("InstanceId")))
        .and_then(props::as_uuid)?;
    Some(read_save_parameter_dto(
        save_parameter,
        instance_id,
        true,
        game_data,
    ))
}

/// Port of `SummariesMixin.get_pal_summaries` (`game/mixins/summaries.py`).
/// Summary-specific defaults differ from the full `Pal` dump: `level`
/// defaults to 1 (same as the full dump), `rank` defaults to **1** (the full
/// dump defaults to 0 -- see `read_save_parameter_dto`), `stomach` defaults
/// to 150.0 (same as the full dump).
pub fn pal_summaries(
    session: &SaveSession,
    game_data: &GameData,
) -> Result<Vec<PalSummary>, CoreError> {
    // container_id -> (guild_id, base_id), built from BaseCampSaveData's
    // WorkerDirector (summaries.py's `_build_base_container_map`). Absent
    // BaseCampSaveData (no base ever built) yields an empty map, matching
    // Python's `for base in self._base_camp_save_data_map or []`.
    let mut base_container_map = std::collections::HashMap::new();
    if let Some(base_entries) = session.base_camp_map() {
        for base_entry in base_entries {
            let Some(base_id) = props::as_uuid(&base_entry.key) else {
                continue;
            };
            let Some((guild_id, container_id)) = super::guild::base_guild_and_container(base_entry)
            else {
                continue;
            };
            base_container_map.insert(container_id, (guild_id, base_id));
        }
    }

    let keys = known_pal_keys(game_data);
    let mut summaries = Vec::new();
    for entry in world::character_map(&session.level)? {
        if world::entry_is_player(entry) {
            continue;
        }
        let Some(save_parameter) = world::entry_save_parameter(entry) else {
            continue;
        };
        let Some(instance_id) = world::entry_instance_id(entry) else {
            continue;
        };
        let character_id = param(save_parameter, "CharacterID")
            .and_then(props::as_str)
            .unwrap_or("")
            .to_string();
        let owner_uid = param(save_parameter, "OwnerPlayerUId").and_then(props::as_uuid);
        let owner_name = owner_uid
            .and_then(|uid| session.player_summaries.get(&uid))
            .map(|summary| summary.nickname.clone());

        // `slot_id = save_parameter.get("SlotId")` (summaries.py) -- unlike
        // the full dump's storage_id/storage_slot, this checks *only*
        // "SlotId", with no "SlotID" fallback. The brief's version of this
        // function added a "SlotID" fallback here that summaries.py does
        // not have; Python source wins (see this task's report).
        let (guild_id, base_id) = param(save_parameter, "SlotId")
            .and_then(props::struct_props)
            .and_then(|slot| {
                slot.0
                    .get(&PropertyKey::from("ContainerId"))
                    .and_then(props::struct_props)
                    .and_then(|container| container.0.get(&PropertyKey::from("ID")))
                    .and_then(props::as_uuid)
            })
            .and_then(|container_id| base_container_map.get(&container_id).copied())
            .map(|(guild, base)| (Some(guild), Some(base)))
            .unwrap_or((None, None));

        let gender = param(save_parameter, "Gender")
            .and_then(props::as_str)
            .map(|raw| match PalGender::from_prefixed(raw) {
                PalGender::None => "None".to_string(),
                PalGender::Male => "Male".to_string(),
                PalGender::Female => "Female".to_string(),
            });

        let hp = param(save_parameter, "Hp")
            .or_else(|| param(save_parameter, "HP"))
            .and_then(props::fixed_point64)
            .unwrap_or(0);

        summaries.push(PalSummary {
            instance_id,
            character_key: format_character_key(&character_id, &keys),
            character_id,
            nickname: param(save_parameter, "NickName")
                .and_then(props::as_str)
                .map(str::to_string),
            owner_uid,
            owner_name,
            guild_id,
            base_id,
            gender,
            level: param(save_parameter, "Level")
                .and_then(props::as_byte_number)
                .unwrap_or(1) as i64,
            hp,
            stomach: param(save_parameter, "FullStomach")
                .and_then(props::as_f32)
                .unwrap_or(150.0) as f64,
            rank: param(save_parameter, "Rank")
                .and_then(props::as_byte_number)
                .unwrap_or(1) as i64,
            exp: param(save_parameter, "Exp")
                .and_then(props::as_i64)
                .unwrap_or(0),
            talent_hp: param(save_parameter, "Talent_HP")
                .and_then(props::as_byte_number)
                .unwrap_or(0) as i64,
            talent_shot: param(save_parameter, "Talent_Shot")
                .and_then(props::as_byte_number)
                .unwrap_or(0) as i64,
            talent_defense: param(save_parameter, "Talent_Defense")
                .and_then(props::as_byte_number)
                .unwrap_or(0) as i64,
            rank_hp: param(save_parameter, "Rank_HP")
                .and_then(props::as_byte_number)
                .unwrap_or(0) as i64,
            rank_attack: param(save_parameter, "Rank_Attack")
                .and_then(props::as_byte_number)
                .unwrap_or(0) as i64,
            rank_defense: param(save_parameter, "Rank_Defence")
                .and_then(props::as_byte_number)
                .unwrap_or(0) as i64,
            rank_craftspeed: param(save_parameter, "Rank_CraftSpeed")
                .and_then(props::as_byte_number)
                .unwrap_or(0) as i64,
        });
    }
    Ok(summaries)
}

#[cfg(test)]
mod tests {
    use super::*;
    use uesave::games::palworld::PalCharacterData;
    use uesave::{Byte, Properties, Property, StructValue};

    fn game_data() -> GameData {
        let json_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data/json");
        GameData::load(&json_dir).expect("data dir")
    }

    fn fguid(text: &str) -> uesave::FGuid {
        serde_json::from_value(serde_json::Value::String(text.to_string())).unwrap()
    }

    fn guid_property(text: &str) -> Property {
        Property::Struct(StructValue::Guid(fguid(text)))
    }

    fn character_entry(
        instance_id: &str,
        save_parameter: Properties,
        group_id: uesave::FGuid,
    ) -> MapEntry {
        let mut key_properties = Properties::default();
        key_properties.insert(
            "PlayerUId",
            guid_property("00000000-0000-0000-0000-000000000000"),
        );
        key_properties.insert("InstanceId", guid_property(instance_id));

        let mut object = Properties::default();
        object.insert(
            "SaveParameter",
            Property::Struct(StructValue::Struct(save_parameter)),
        );
        let character_data = PalCharacterData {
            object,
            unknown_bytes: [0; 4],
            group_id,
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

    #[test]
    fn known_pal_keys_loads_the_real_pals_json_key_set() {
        let data = game_data();
        let keys = known_pal_keys(&data);
        // Real key casing is "Sheepball" (lowercase second word), not
        // "SheepBall" -- verified against data/json/pals.json directly
        // (`.venv` Python: `[k for k in json.load(...) if 'sheep' in
        // k.lower()]` -> `['Quest_Farmer03_SheepBall', 'Sheepball', ...]`).
        assert!(
            keys.contains("Sheepball"),
            "pals.json must have a Sheepball entry"
        );
    }

    #[test]
    fn read_save_parameter_dto_reads_a_well_formed_pal() {
        let data = game_data();
        let mut save_parameter = Properties::default();
        save_parameter.insert("CharacterID", Property::Name("SheepBall".to_string()));
        save_parameter.insert("Level", Property::Byte(Byte::Byte(12)));
        save_parameter.insert("NickName", Property::Str("Wooly".to_string()));
        let instance_id = uuid::Uuid::parse_str("11111111-2222-3333-4444-555555555555").unwrap();

        let dto = read_save_parameter_dto(&save_parameter, instance_id, false, &data);

        assert_eq!(dto.character_id, "SheepBall");
        assert_eq!(dto.character_key, "sheepball");
        assert_eq!(dto.level, 12);
        assert_eq!(dto.nickname.as_deref(), Some("Wooly"));
        assert_eq!(dto.is_boss, Some(false));
        assert_eq!(dto.gender, PalGender::Female); // default when Gender absent
    }

    #[test]
    fn read_save_parameter_dto_boss_prefixed_character_id_is_boss_unless_lucky() {
        let data = game_data();
        let mut save_parameter = Properties::default();
        save_parameter.insert("CharacterID", Property::Name("BOSS_SheepBall".to_string()));
        let instance_id = uuid::Uuid::nil();

        let boss_dto = read_save_parameter_dto(&save_parameter, instance_id, false, &data);
        assert_eq!(boss_dto.is_boss, Some(true));

        save_parameter.insert("IsRarePal", Property::Bool(true));
        let lucky_dto = read_save_parameter_dto(&save_parameter, instance_id, false, &data);
        assert_eq!(
            lucky_dto.is_boss,
            Some(false),
            "is_boss is false when is_lucky is true"
        );
        assert_eq!(lucky_dto.is_lucky, Some(true));
    }

    #[test]
    fn read_save_parameter_dto_is_sick_ignores_hunger_and_sanity_markers() {
        let data = game_data();
        let instance_id = uuid::Uuid::nil();

        let mut only_hunger_and_sanity = Properties::default();
        only_hunger_and_sanity.insert("HungerType", Property::Bool(true));
        only_hunger_and_sanity.insert("SanityValue", Property::Float(uesave::Float(50.0)));
        let dto = read_save_parameter_dto(&only_hunger_and_sanity, instance_id, false, &data);
        assert!(
            !dto.is_sick,
            "HungerType/SanityValue alone must not mark a pal sick"
        );

        let mut with_real_marker = Properties::default();
        with_real_marker.insert("WorkerSick", Property::Bool(true));
        let sick_dto = read_save_parameter_dto(&with_real_marker, instance_id, false, &data);
        assert!(sick_dto.is_sick);
    }

    #[test]
    fn read_save_parameter_dto_is_sick_always_false_for_dps_pals() {
        let data = game_data();
        let mut save_parameter = Properties::default();
        save_parameter.insert("WorkerSick", Property::Bool(true));
        let instance_id = uuid::Uuid::nil();

        let dto = read_save_parameter_dto(&save_parameter, instance_id, true, &data);
        assert!(!dto.is_sick, "DPS pals are never marked sick");
    }

    #[test]
    fn read_save_parameter_dto_filtered_nickname_only_populated_for_dps() {
        let data = game_data();
        let mut save_parameter = Properties::default();
        save_parameter.insert("FilteredNickName", Property::Str("Clean".to_string()));
        let instance_id = uuid::Uuid::nil();

        let level_dto = read_save_parameter_dto(&save_parameter, instance_id, false, &data);
        assert!(level_dto.filtered_nickname.is_none());

        let dps_dto = read_save_parameter_dto(&save_parameter, instance_id, true, &data);
        assert_eq!(dps_dto.filtered_nickname.as_deref(), Some("Clean"));
    }

    #[test]
    fn read_save_parameter_dto_slot_id_prefers_uppercase_spelling() {
        // game/pal.py: `"SlotID" if "SlotID" in self._save_parameter else "SlotId"`.
        let data = game_data();
        let mut id_properties = Properties::default();
        id_properties.insert("ID", guid_property("aaaaaaaa-0000-0000-0000-000000000001"));
        let mut slot_properties = Properties::default();
        slot_properties.insert(
            "ContainerId",
            Property::Struct(StructValue::Struct(id_properties)),
        );
        slot_properties.insert("SlotIndex", Property::Int(7));
        let mut save_parameter = Properties::default();
        save_parameter.insert(
            "SlotID",
            Property::Struct(StructValue::Struct(slot_properties)),
        );

        let dto = read_save_parameter_dto(&save_parameter, uuid::Uuid::nil(), false, &data);
        assert_eq!(dto.storage_slot, 7);
        assert_eq!(
            dto.storage_id.to_string(),
            "aaaaaaaa-0000-0000-0000-000000000001"
        );
    }

    #[test]
    fn known_pal_keys_returns_empty_set_when_pals_json_is_absent_or_not_an_object() {
        // No pals.json at all: game_data.get("pals") is None.
        let empty_dir = tempfile::tempdir().unwrap();
        let data_without_pals = GameData::load(empty_dir.path()).unwrap();
        assert!(known_pal_keys(&data_without_pals).is_empty());

        // pals.json present but not a JSON object (e.g. corrupted/wrong shape).
        let wrong_shape_dir = tempfile::tempdir().unwrap();
        std::fs::write(wrong_shape_dir.path().join("pals.json"), "[1, 2, 3]").unwrap();
        let data_with_wrong_shape = GameData::load(wrong_shape_dir.path()).unwrap();
        assert!(known_pal_keys(&data_with_wrong_shape).is_empty());
    }

    #[test]
    fn pal_dto_from_entry_resolves_group_id_only_when_non_nil() {
        let data = game_data();
        let mut save_parameter = Properties::default();
        save_parameter.insert("CharacterID", Property::Name("SheepBall".to_string()));

        let nil_group_entry = character_entry(
            "11111111-2222-3333-4444-555555555555",
            save_parameter.clone(),
            uesave::FGuid::nil(),
        );
        let dto = pal_dto_from_entry(&nil_group_entry, &data).unwrap();
        assert!(dto.group_id.is_none());

        let real_group_entry = character_entry(
            "11111111-2222-3333-4444-555555555555",
            save_parameter,
            fguid("99999999-1111-2222-3333-444444444444"),
        );
        let dto = pal_dto_from_entry(&real_group_entry, &data).unwrap();
        assert_eq!(
            dto.group_id.map(|id| id.to_string()),
            Some("99999999-1111-2222-3333-444444444444".to_string())
        );
    }

    #[test]
    fn pal_dto_from_entry_returns_none_when_instance_id_is_unresolvable() {
        let data = game_data();
        let entry = MapEntry {
            key: Property::Bool(true), // not a struct with an InstanceId field
            value: Property::Bool(true),
        };
        assert!(pal_dto_from_entry(&entry, &data).is_none());
    }

    #[test]
    fn pal_dto_from_dps_slot_reads_instance_id_from_nested_struct() {
        let data = game_data();
        let mut save_parameter = Properties::default();
        save_parameter.insert("CharacterID", Property::Name("SheepBall".to_string()));

        let mut inner_instance_id = Properties::default();
        inner_instance_id.insert(
            "InstanceId",
            guid_property("22222222-3333-4444-5555-666666666666"),
        );
        let mut slot_properties = Properties::default();
        slot_properties.insert(
            "SaveParameter",
            Property::Struct(StructValue::Struct(save_parameter)),
        );
        slot_properties.insert(
            "InstanceId",
            Property::Struct(StructValue::Struct(inner_instance_id)),
        );
        let slot = StructValue::Struct(slot_properties);

        let dto = pal_dto_from_dps_slot(&slot, &data).unwrap();
        assert_eq!(
            dto.instance_id.to_string(),
            "22222222-3333-4444-5555-666666666666"
        );
        assert_eq!(dto.character_id, "SheepBall");
    }

    #[test]
    fn pal_dto_from_dps_slot_returns_none_for_a_non_struct_slot() {
        let data = game_data();
        assert!(pal_dto_from_dps_slot(&StructValue::Guid(uesave::FGuid::nil()), &data).is_none());
    }

    #[test]
    fn max_hp_for_falls_back_to_hp_when_character_id_is_unrecognized() {
        let data = game_data();
        let dto = PalDto {
            instance_id: uuid::Uuid::nil(),
            character_id: "TotallyMadeUpCreature".to_string(),
            character_key: String::new(),
            owner_uid: None,
            is_lucky: Some(false),
            is_boss: Some(false),
            is_predator: false,
            is_tower: false,
            gender: PalGender::Female,
            nickname: None,
            filtered_nickname: None,
            group_id: None,
            stomach: 150.0,
            sanity: 100.0,
            hp: 12345,
            level: 10,
            exp: 0,
            rank: 1,
            rank_hp: 0,
            rank_attack: 0,
            rank_defense: 0,
            rank_craftspeed: 0,
            talent_hp: 0,
            talent_shot: 0,
            talent_defense: 0,
            max_hp: 0,
            storage_slot: 0,
            storage_id: props::EMPTY_UUID,
            learned_skills: vec![],
            active_skills: vec![],
            passive_skills: vec![],
            work_suitability: Default::default(),
            is_sick: false,
            friendship_point: 0,
        };
        assert_eq!(max_hp_for(&dto, &data), 12345);
    }
}
