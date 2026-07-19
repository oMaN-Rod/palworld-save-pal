//! Pal read/write side for Level.sav pals and DPS-array pals.
//!
//! Save data is untrusted: a malformed pal entry (missing `SaveParameter`,
//! wrong-typed `RawData`, ...) is skipped, never a panic.

use std::collections::HashSet;

use crate::ue::{MapEntry, Properties, Property, PropertyKey, StructValue, ValueVec};

use crate::dto::ordered_map::OrderedMap;
use crate::dto::pal::{format_character_key, PalDto, PalGender, WORK_SUITABILITIES};
use crate::dto::summary::PalSummary;
use crate::error::CoreError;
use crate::gamedata::GameData;
use crate::props;
use crate::session::SaveSession;

use super::world;

/// Look up a top-level property inside a pal/player `SaveParameter` bag.
pub(crate) fn param<'a>(save_parameter: &'a Properties, name: &str) -> Option<&'a Property> {
    save_parameter.0.get(&PropertyKey::from(name))
}

/// Every key in `pals.json`, used by `format_character_key` to decide whether
/// a `BOSS_`-prefixed id names a pal that is its own catalog entry (keep the
/// prefix) or an ordinary pal spawned as a boss (strip it). Borrowed from
/// `GameData`'s memoized lookup: this is called once per pal decoded, so it
/// must not rebuild the set.
pub fn known_pal_keys(game_data: &GameData) -> &HashSet<String> {
    &game_data.pal_lookup().keys
}

/// The sickness markers `is_sick` checks for. `HungerType`/`SanityValue` are
/// deliberately excluded: they are normal state, not illness.
const SICK_MARKERS: [&str; 3] = ["PalReviveTimer", "PhysicalHealth", "WorkerSick"];

/// Resolves a pal's static `pals.json` entry from a `character_key`, matching
/// case-insensitively against the real key casing.
fn pal_data_for<'a>(character_key: &str, game_data: &'a GameData) -> Option<&'a serde_json::Value> {
    if character_key.is_empty() {
        return None;
    }
    let canonical = game_data
        .pal_lookup()
        .lower_to_canonical
        .get(character_key)?;
    game_data
        .get("pals")
        .and_then(|value| value.as_object())
        .and_then(|pals_json| pals_json.get(canonical))
}

/// Builds the full `PalDto` from a resolved `SaveParameter` bag, for both
/// Level.sav pals (`is_dps: false`) and GPS/DPS-array pals (`is_dps: true`).
pub fn read_save_parameter_dto(
    save_parameter: &Properties,
    instance_id: uuid::Uuid,
    is_dps: bool,
    game_data: &GameData,
) -> PalDto {
    // A character-map entry with no CharacterID is corrupt; "" keeps the
    // downstream prefix checks (boss/predator/tower) total.
    let character_id = param(save_parameter, "CharacterID")
        .and_then(props::as_str)
        .unwrap_or("")
        .to_string();

    let is_lucky = param(save_parameter, "IsRarePal")
        .and_then(props::as_bool)
        .unwrap_or(false);
    // A lucky pal is never also reported as a boss.
    let is_boss = character_id.to_uppercase().starts_with("BOSS_") && !is_lucky;

    // Gender is absent for pals that never had one assigned; Female is the
    // contract's default.
    let gender = param(save_parameter, "Gender")
        .and_then(props::as_str)
        .map(PalGender::from_prefixed)
        .unwrap_or(PalGender::Female);

    // Saves spell the slot key either "SlotID" or "SlotId"; the all-caps
    // spelling wins when both are present.
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
            // An unrecognized work-suitability name is dropped rather than
            // surfaced: untrusted save data must not fail the whole read.
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

    // Saves spell the health key either "Hp" or the older "HP"; "Hp" wins.
    let hp = param(save_parameter, "Hp")
        .or_else(|| param(save_parameter, "HP"))
        .and_then(props::fixed_point64)
        .unwrap_or(0);

    let nickname = param(save_parameter, "NickName")
        .and_then(props::as_str)
        .map(str::to_string);
    // Only DPS pals carry a filtered nickname.
    let filtered_nickname = if is_dps {
        param(save_parameter, "FilteredNickName")
            .and_then(props::as_str)
            .map(str::to_string)
    } else {
        None
    };

    let character_key = format_character_key(&character_id, known_pal_keys(game_data));

    // Corrupted saves carry a non-finite FullStomach (NaN seen in the wild).
    // It must not reach the wire: `serde_json` has no NaN/Infinity literal
    // and would emit `null`. Fall back to the species' max, else 300.0. An
    // *absent* FullStomach is a different case and keeps the 150.0 default.
    let raw_stomach = param(save_parameter, "FullStomach")
        .and_then(props::as_f32)
        .unwrap_or(150.0) as f64;
    let stomach = if raw_stomach.is_finite() {
        raw_stomach
    } else {
        pal_data_for(&character_key, game_data)
            .and_then(|pal_data| pal_data.pointer("/max_full_stomach"))
            .and_then(|value| value.as_f64())
            .unwrap_or(300.0)
    };

    let mut dto = PalDto {
        instance_id,
        owner_uid: param(save_parameter, "OwnerPlayerUId").and_then(props::as_uuid),
        character_key,
        is_lucky: Some(is_lucky),
        is_boss: Some(is_boss),
        is_predator: character_id.starts_with("PREDATOR_"),
        gender,
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
        talent_hp: param(save_parameter, "Talent_HP")
            .and_then(props::as_byte_number)
            .unwrap_or(0) as i64,
        talent_shot: param(save_parameter, "Talent_Shot")
            .and_then(props::as_byte_number)
            .unwrap_or(0) as i64,
        talent_defense: param(save_parameter, "Talent_Defense")
            .and_then(props::as_byte_number)
            .unwrap_or(0) as i64,
        // The full dump defaults an absent Rank to 0; `pal_summaries` defaults
        // it to 1. Both are contract, not a typo.
        rank: param(save_parameter, "Rank")
            .and_then(props::as_byte_number)
            .unwrap_or(0) as i64,
        level: param(save_parameter, "Level")
            .and_then(props::as_byte_number)
            .unwrap_or(1) as i64,
        exp: param(save_parameter, "Exp")
            .and_then(props::as_i64)
            .unwrap_or(0),
        nickname,
        filtered_nickname,
        is_tower: character_id.starts_with("GYM_"),
        storage_id,
        stomach,
        storage_slot,
        learned_skills: param(save_parameter, "MasteredWaza")
            .and_then(props::enum_values)
            .cloned()
            .unwrap_or_default(),
        active_skills: param(save_parameter, "EquipWaza")
            .and_then(props::enum_values)
            .cloned()
            .unwrap_or_default(),
        passive_skills: param(save_parameter, "PassiveSkillList")
            .and_then(props::name_values)
            .cloned()
            .unwrap_or_default(),
        hp,
        max_hp: 0,      // filled below: max_hp_for reads other dto fields
        group_id: None, // filled by pal_dto_from_entry from PalCharacterData.group_id
        sanity: param(save_parameter, "SanityValue")
            .and_then(props::as_f32)
            .unwrap_or(100.0) as f64,
        work_suitability,
        // DPS pals are never sick.
        is_sick: !is_dps
            && SICK_MARKERS
                .iter()
                .any(|marker| param(save_parameter, marker).is_some()),
        friendship_point: param(save_parameter, "FriendshipPoint")
            .and_then(props::as_i32)
            .unwrap_or(0) as i64,
        character_id,
    };
    dto.max_hp = max_hp_for(&dto, is_boss || is_lucky, game_data);
    dto
}

/// Computed max HP, falling back to `dto.hp` for a pal with no `scaling.hp`
/// entry in `pals.json`.
///
/// `boosted` (boss or lucky, worth a 1.2x multiplier) is a parameter rather
/// than read off `dto`: on the write path `dto.is_boss` may be stale and
/// `dto.is_lucky` may be `None` ("leave `IsRarePal` alone", not "false"), so
/// only the caller can resolve it against the save's current state.
pub fn max_hp_for(dto: &PalDto, boosted: bool, game_data: &GameData) -> i64 {
    let keys = known_pal_keys(game_data);
    let pal_key = format_character_key(&dto.character_id, keys);
    let Some(pal_data) = pal_data_for(&pal_key, game_data) else {
        return dto.hp;
    };
    let Some(hp_scaling) = pal_data.pointer("/scaling/hp").and_then(|v| v.as_f64()) else {
        return dto.hp;
    };
    let condenser_bonus = (dto.rank as f64 - 1.0) * 0.05;
    let hp_iv = dto.talent_hp as f64 * 0.3 / 100.0;
    let hp_soul_bonus = dto.rank_hp as f64 * 0.03;
    let alpha_scaling = if boosted { 1.2 } else { 1.0 };
    let base = (500.0
        + 5.0 * dto.level as f64
        + hp_scaling * 0.5 * dto.level as f64 * (1.0 + hp_iv) * alpha_scaling)
        .floor();
    ((base * (1.0 + condenser_bonus) * (1.0 + hp_soul_bonus)).floor() as i64) * 1000
}

/// Reads a `CharacterSaveParameterMap` entry. `None` when the entry isn't
/// shaped like a pal (no `InstanceId`/`SaveParameter`/`PalCharacterData`).
pub fn pal_dto_from_entry(entry: &MapEntry, game_data: &GameData) -> Option<PalDto> {
    let instance_id = world::entry_instance_id(entry)?;
    let save_parameter = world::entry_save_parameter(entry)?;
    let mut dto = read_save_parameter_dto(save_parameter, instance_id, false, game_data);
    let character_data = world::entry_character_data(entry)?;
    // A nil guid means "no guild", not guild zero.
    let group_id = props::guid_to_uuid(&character_data.group_id);
    dto.group_id = (group_id != props::EMPTY_UUID).then_some(group_id);
    Some(dto)
}

/// Reads a GPS/DPS `SaveParameterArray` element: a struct with a
/// `SaveParameter` property and an `InstanceId` struct holding an inner
/// `InstanceId` guid -- no `RawData`/`PalCharacterData` wrapper, unlike
/// Level.sav pals. `None` when the slot isn't shaped this way.
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

/// Lightweight summaries of every non-player pal in Level.sav.
///
/// Summary defaults differ from the full dump in one place: an absent `Rank`
/// is reported as 1 here, 0 there.
pub fn pal_summaries(
    session: &SaveSession,
    game_data: &GameData,
) -> Result<Vec<PalSummary>, CoreError> {
    // container_id -> (guild_id, base_id), from BaseCampSaveData's
    // WorkerDirector. Empty when the world has no bases.
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

        // Base membership keys off "SlotId" only -- no "SlotID" fallback,
        // unlike the full dump's storage_id/storage_slot.
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

        // Summaries report gender as `None` for both an absent and an empty
        // Gender value; the full dump defaults either to Female.
        let gender = param(save_parameter, "Gender")
            .and_then(props::as_str)
            .filter(|raw| !raw.is_empty())
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
            character_key: format_character_key(&character_id, keys),
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

/// Every marker `heal_save_parameter` clears. A superset of `SICK_MARKERS`,
/// which is the narrower set that makes a pal *report* as sick.
const PAL_SICK_TYPES: [&str; 5] = [
    "PalReviveTimer",
    "PhysicalHealth",
    "WorkerSick",
    "HungerType",
    "SanityValue",
];

/// Sets `name`, or removes it entirely when `None` -- a pal property at its
/// default value is absent from the save, not written as zero.
fn set_or_remove(save_parameter: &mut Properties, name: &str, property: Option<Property>) {
    match property {
        Some(value) => {
            save_parameter.insert(name, value);
        }
        None => {
            save_parameter.0.shift_remove(&PropertyKey::from(name));
        }
    }
}

/// A recognized species' `max_full_stomach`, else the flat 300.0 default.
pub fn max_stomach_for(character_id: &str, game_data: &GameData) -> f64 {
    let keys = known_pal_keys(game_data);
    let pal_key = format_character_key(character_id, keys);
    pal_data_for(&pal_key, game_data)
        .and_then(|pal_data| pal_data.pointer("/max_full_stomach"))
        .and_then(|value| value.as_f64())
        .unwrap_or(300.0)
}

/// Clears every sickness marker and restores sanity and stomach to full.
pub fn heal_save_parameter(
    save_parameter: &mut Properties,
    character_id: &str,
    game_data: &GameData,
) {
    for marker in PAL_SICK_TYPES {
        save_parameter.0.shift_remove(&PropertyKey::from(marker));
    }
    save_parameter.insert("SanityValue", props::float_property(100.0));
    save_parameter.insert(
        "FullStomach",
        props::float_property(max_stomach_for(character_id, game_data) as f32),
    );
}

/// Applies every writable field of `dto` onto an existing pal/player
/// `SaveParameter` bag.
///
/// - `group_id` is NOT applied: it lives in `PalCharacterData`, a sibling of
///   `SaveParameter` this signature cannot reach. Callers owning the whole
///   `MapEntry` must write it themselves.
/// - `dto.is_boss` is never read. It is caller-supplied and can be stale
///   (echoed back by a client after `character_id` changed), which would
///   wrongly re-add the `BOSS_` prefix. Boss-ness is re-derived below from
///   `character_id` and `is_lucky`.
/// - Byte- and i32-width fields are saturated, never wrapped: DTO input is
///   untrusted and a wrapped value would silently corrupt the save.
pub fn apply_pal_dto(
    save_parameter: &mut Properties,
    dto: &crate::dto::pal::PalDto,
    is_dps: bool,
    game_data: &GameData,
) {
    if let Some(owner_uid) = dto.owner_uid {
        save_parameter.insert("OwnerPlayerUId", props::guid_property(owner_uid));
    }
    save_parameter.insert("CharacterID", props::name_property(&dto.character_id));

    // `is_lucky: None` means "leave IsRarePal untouched", not "false".
    if let Some(is_lucky) = dto.is_lucky {
        if is_lucky {
            save_parameter.insert("IsRarePal", props::bool_property(true));
        } else {
            save_parameter
                .0
                .shift_remove(&PropertyKey::from("IsRarePal"));
        }
    }

    save_parameter.insert("Gender", props::enum_property(&dto.gender.prefixed()));

    // Soul-upgrade ranks are absent from the save at 0, not written as 0.
    for (name, value) in [
        ("Rank_HP", dto.rank_hp),
        ("Rank_Attack", dto.rank_attack),
        ("Rank_Defence", dto.rank_defense),
        ("Rank_CraftSpeed", dto.rank_craftspeed),
    ] {
        set_or_remove(
            save_parameter,
            name,
            (value != 0).then(|| props::byte_property(value.clamp(0, 255) as u8)),
        );
    }

    save_parameter.insert(
        "Talent_HP",
        props::byte_property(dto.talent_hp.clamp(0, 255) as u8),
    );
    save_parameter.insert(
        "Talent_Shot",
        props::byte_property(dto.talent_shot.clamp(0, 255) as u8),
    );
    save_parameter.insert(
        "Talent_Defense",
        props::byte_property(dto.talent_defense.clamp(0, 255) as u8),
    );

    set_or_remove(
        save_parameter,
        "Rank",
        (dto.rank != 0).then(|| props::byte_property(dto.rank.clamp(0, 255) as u8)),
    );
    // Level 1 is the default and is stored as an absent property.
    set_or_remove(
        save_parameter,
        "Level",
        (dto.level > 1).then(|| props::byte_property(dto.level.clamp(0, 255) as u8)),
    );
    set_or_remove(
        save_parameter,
        "Exp",
        (dto.exp != 0).then(|| props::int64_property(dto.exp)),
    );

    if let Some(nickname) = &dto.nickname {
        save_parameter.insert("NickName", props::str_property(nickname));
    }
    // Only DPS pals carry a filtered nickname.
    if is_dps {
        if let Some(filtered) = &dto.filtered_nickname {
            save_parameter.insert("FilteredNickName", props::str_property(filtered));
        }
    }

    // Overwritten again by heal() below for non-DPS pals.
    save_parameter.insert("FullStomach", props::float_property(dto.stomach as f32));

    // Storage moves are SlotIndex-only: `dto.storage_id` is inert, a pal's
    // ContainerId is never reassigned here (moving a pal between containers
    // goes through `move_pal`). Real saves spell the key "SlotId", but pals
    // this module creates spell it "SlotID", so both are accepted; a pal with
    // neither key is left untouched.
    let slot_key = if save_parameter.0.contains_key(&PropertyKey::from("SlotID")) {
        "SlotID"
    } else {
        "SlotId"
    };
    if let Some(slot_struct) = save_parameter
        .0
        .get_mut(&PropertyKey::from(slot_key))
        .and_then(props::struct_props_mut)
    {
        slot_struct.insert(
            "SlotIndex",
            props::int_property(dto.storage_slot.clamp(i32::MIN as i64, i32::MAX as i64) as i32),
        );
    }

    set_or_remove(
        save_parameter,
        "MasteredWaza",
        (!dto.learned_skills.is_empty())
            .then(|| props::enum_array_property(dto.learned_skills.clone())),
    );
    save_parameter.insert(
        "EquipWaza",
        props::enum_array_property(dto.active_skills.clone()),
    );
    save_parameter.insert(
        "PassiveSkillList",
        props::name_array_property(dto.passive_skills.clone()),
    );

    // Overwritten again by heal() below for non-DPS pals.
    save_parameter.insert("SanityValue", props::float_property(dto.sanity as f32));

    // Zero-rank and unrecognized work-suitability keys are dropped:
    // `work_suitability` is an unvalidated `String` map on the DTO, and an
    // unknown `EPalWorkSuitability::` variant must never reach the save.
    let non_zero_known: Vec<(&String, &i64)> = dto
        .work_suitability
        .iter()
        .filter(|(name, rank)| **rank != 0 && WORK_SUITABILITIES.contains(&name.as_str()))
        .collect();
    // Remove-then-append, not in-place insert: the property must end up at the
    // END of the bag. An `IndexMap` insert would keep its original position and
    // change the byte layout of the resaved file.
    save_parameter
        .0
        .shift_remove(&PropertyKey::from("GotWorkSuitabilityAddRankList"));
    if !non_zero_known.is_empty() {
        let mut rank_structs = Vec::new();
        for (work_name, rank) in non_zero_known {
            let mut rank_props = Properties::default();
            rank_props.insert(
                "WorkSuitability",
                props::enum_property(&format!("EPalWorkSuitability::{work_name}")),
            );
            rank_props.insert("Rank", props::int_property(*rank as i32));
            rank_structs.push(StructValue::Struct(rank_props));
        }
        save_parameter.insert(
            "GotWorkSuitabilityAddRankList",
            Property::Array(ValueVec::Struct(rank_structs)),
        );
    }

    save_parameter.insert(
        "FriendshipPoint",
        props::int_property(dto.friendship_point.clamp(i32::MIN as i64, i32::MAX as i64) as i32),
    );

    // Hp is always recomputed from the state just written, so `dto.hp` never
    // survives this call. `IsRarePal` is read back off the save rather than
    // from the DTO: `dto.is_lucky: None` leaves the save's existing flag in
    // place, and the boost multiplier must reflect that flag, not the DTO.
    let current_is_lucky = param(save_parameter, "IsRarePal")
        .and_then(props::as_bool)
        .unwrap_or(false);
    let boosted = dto.character_id.to_uppercase().starts_with("BOSS_") || current_is_lucky;
    let max_hp = max_hp_for(dto, boosted, game_data);
    save_parameter.insert("Hp", props::fixed_point64_property(max_hp));
    // Drop the older "HP" spelling: it is now redundant with the "Hp" written
    // above, and leaving both would let the stale one win on the next read.
    save_parameter.0.shift_remove(&PropertyKey::from("HP"));
    if !is_dps {
        heal_save_parameter(save_parameter, &dto.character_id, game_data);
    }

    // Boss-ness is re-derived here, never taken from `dto.is_boss`.
    let current_id = dto.character_id.clone();
    let should_be_boss =
        current_id.to_uppercase().starts_with("BOSS_") || dto.is_lucky.unwrap_or(false);
    let has_prefix = current_id.starts_with("BOSS_"); // the stored prefix is case-sensitive
    if should_be_boss && !has_prefix {
        save_parameter.insert(
            "CharacterID",
            props::name_property(&format!("BOSS_{current_id}")),
        );
    } else if !should_be_boss && has_prefix {
        save_parameter.insert("CharacterID", props::name_property(&current_id[5..]));
    }
}

/// Status keys as the save spells them (Japanese); includes capture rate.
pub const STATUS_NAMES: [&str; 6] = [
    "最大HP",
    "最大SP",
    "攻撃力",
    "所持重量",
    "捕獲率",
    "作業速度",
];
/// Same as `STATUS_NAMES` without capture rate.
pub const EX_STATUS_NAMES: [&str; 5] = ["最大HP", "最大SP", "攻撃力", "所持重量", "作業速度"];

/// One zeroed `{StatusName, StatusPoint}` struct per status name.
pub(crate) fn status_point_structs(names: &[&str]) -> Property {
    let mut values = Vec::new();
    for status_name in names {
        let mut status_props = Properties::default();
        status_props.insert("StatusName", props::name_property(status_name));
        status_props.insert("StatusPoint", props::int_property(0));
        values.push(StructValue::Struct(status_props));
    }
    Property::Array(ValueVec::Struct(values))
}

/// `OwnedTime` for a freshly created pal: a fixed UE tick count, not "now".
/// `uesave`'s `StructValue::DateTime` is a bare `u64` of ticks, so a wrong
/// value here compiles fine and silently writes a bogus "owned since" date.
const PAL_OWNED_TIME_TICKS: u64 = 638_486_453_957_560_000;

/// Opaque UE custom-version-guid metadata. Every real character-map entry
/// carries this as a sibling of `RawData`, so a new pal entry must too.
const CUSTOM_VERSION_DATA: [u8; 24] = [
    1, 0, 0, 0, 108, 246, 252, 15, 153, 72, 144, 17, 248, 156, 96, 177, 94, 71, 70, 74, 1, 0, 0, 0,
];

/// Builds a complete `CharacterSaveParameterMap` entry for a freshly created
/// pal. The caller must insert it into the map and call
/// `ensure_pal_property_schemas`.
#[allow(clippy::too_many_arguments)]
pub fn new_pal_entry(
    character_id: &str,
    instance_id: uuid::Uuid,
    owner_uid: uuid::Uuid,
    container_id: uuid::Uuid,
    slot_index: i32,
    group_id: Option<uuid::Uuid>,
    nickname: &str,
    game_data: &GameData,
) -> MapEntry {
    let mut save_parameter = Properties::default();
    save_parameter.insert("CharacterID", props::name_property(character_id));
    save_parameter.insert("Gender", props::enum_property("EPalGenderType::Female"));
    save_parameter.insert("Level", props::byte_property(1));
    save_parameter.insert("Exp", props::int64_property(0));
    save_parameter.insert("NickName", props::str_property(nickname));
    save_parameter.insert("EquipWaza", props::enum_array_property(vec![]));
    save_parameter.insert("MasteredWaza", props::enum_array_property(vec![]));
    // Placeholder HP for a brand-new pal; real saves spell this key "Hp".
    save_parameter.insert("Hp", props::fixed_point64_property(545_000));
    save_parameter.insert("Talent_HP", props::byte_property(50));
    save_parameter.insert("Talent_Shot", props::byte_property(50));
    save_parameter.insert("Talent_Defense", props::byte_property(50));
    // A new pal starts with a full stomach, which is species-specific.
    save_parameter.insert(
        "FullStomach",
        props::float_property(max_stomach_for(character_id, game_data) as f32),
    );
    save_parameter.insert("PassiveSkillList", props::name_array_property(vec![]));
    save_parameter.insert(
        "OwnedTime",
        Property::Struct(StructValue::DateTime(PAL_OWNED_TIME_TICKS)),
    );
    save_parameter.insert("OwnerPlayerUId", props::guid_property(owner_uid));
    save_parameter.insert(
        "OldOwnerPlayerUIds",
        Property::Array(ValueVec::Struct(vec![StructValue::Guid(
            props::uuid_to_guid(owner_uid),
        )])),
    );

    let mut container_struct = Properties::default();
    container_struct.insert("ID", props::guid_property(container_id));
    let mut slot_struct = Properties::default();
    slot_struct.insert(
        "ContainerId",
        Property::Struct(StructValue::Struct(container_struct)),
    );
    slot_struct.insert("SlotIndex", props::int_property(slot_index));
    // All-caps "SlotID"; readers check this spelling before "SlotId".
    save_parameter.insert("SlotID", Property::Struct(StructValue::Struct(slot_struct)));

    save_parameter.insert("GotStatusPointList", status_point_structs(&STATUS_NAMES));
    save_parameter.insert(
        "GotExStatusPointList",
        status_point_structs(&EX_STATUS_NAMES),
    );
    save_parameter.insert(
        "LastJumpedLocation",
        Property::Struct(StructValue::Vector(crate::ue::Vector {
            x: crate::ue::Double(0.0),
            y: crate::ue::Double(0.0),
            z: crate::ue::Double(7088.5),
        })),
    );

    let mut object_props = Properties::default();
    object_props.insert(
        "SaveParameter",
        Property::Struct(StructValue::Struct(save_parameter)),
    );

    let character_data = crate::ue::games::palworld::PalCharacterData {
        object: object_props,
        unknown_bytes: [0, 0, 0, 0],
        group_id: props::uuid_to_guid(group_id.unwrap_or(props::EMPTY_UUID)),
        trailing_bytes: [0, 0, 0, 0],
    };

    let mut key_props = Properties::default();
    key_props.insert("PlayerUId", props::guid_property(props::EMPTY_UUID));
    key_props.insert("InstanceId", props::guid_property(instance_id));
    key_props.insert("DebugName", props::str_property(""));

    let mut value_props = Properties::default();
    value_props.insert(
        "RawData",
        Property::Struct(StructValue::Game(crate::ue::PalStruct::CharacterData(character_data))),
    );
    value_props.insert(
        "CustomVersionData",
        Property::Array(ValueVec::Byte(crate::ue::ByteArray::Byte(
            CUSTOM_VERSION_DATA.to_vec(),
        ))),
    );

    MapEntry {
        key: Property::Struct(StructValue::Struct(key_props)),
        value: Property::Struct(StructValue::Struct(value_props)),
    }
}

pub const LEVEL_SAVE_PARAMETER_PREFIX: &str =
    "worldSaveData.CharacterSaveParameterMap.RawData.SaveParameter";
/// `_dps.sav` and `GlobalPalStorage.sav` share one slot layout.
pub const SLOT_SAVE_PARAMETER_PREFIX: &str = "SaveParameterArray.SaveParameter";

/// Every `SaveParameter` property this app can write, with the tag `uesave` records
/// for it when reading a save that has it.
///
/// A property at its default value is absent from a Palworld save, and `uesave`
/// schemas only what it read, so a write must not depend on what the file happened
/// to carry: every name is registered up front rather than discovered.
///
/// `IsRarePal`/`IsPlayer` are `Other(BoolProperty)` because `PropertyTagDataPartial`
/// has no `Bool`; `uesave` maps the two onto each other. Nested struct leaves are
/// paths of their own (`Hp` is not enough -- `Hp.Value` too), as are array element
/// fields, which no save records while the array is empty.
fn save_parameter_schemas() -> Vec<(String, crate::ue::PropertyTagDataPartial)> {
    use crate::ue::{PropertyTagDataPartial as Data, PropertyType, StructType};

    let byte = || Data::Byte(None);
    let other = |t: PropertyType| Data::Other(t);
    // Resolve the name the way the READER does, rather than hand-picking a
    // `StructType` variant: a Palworld game struct becomes a `StructType::Game`,
    // anything else a plain named struct. Guessing wrong makes uesave write the
    // payload with the wrong codec, and the save no longer parses back.
    let named_struct = |name: &str| Data::Struct {
        struct_type: crate::ue::struct_type_for(name),
        id: crate::ue::FGuid::nil(),
    };
    let plain_struct = |struct_type: StructType| Data::Struct {
        struct_type,
        id: crate::ue::FGuid::nil(),
    };
    let struct_array = |name: &str| Data::Array(Box::new(named_struct(name)));

    let mut entries: Vec<(String, Data)> = vec![
        ("CharacterID".into(), other(PropertyType::NameProperty)),
        ("Gender".into(), Data::Enum("EPalGenderType".into(), None)),
        ("IsRarePal".into(), other(PropertyType::BoolProperty)),
        ("IsPlayer".into(), other(PropertyType::BoolProperty)),
        ("Level".into(), byte()),
        ("Rank".into(), byte()),
        ("Rank_HP".into(), byte()),
        ("Rank_Attack".into(), byte()),
        ("Rank_Defence".into(), byte()),
        ("Rank_CraftSpeed".into(), byte()),
        ("Talent_HP".into(), byte()),
        ("Talent_Shot".into(), byte()),
        ("Talent_Defense".into(), byte()),
        ("Exp".into(), other(PropertyType::Int64Property)),
        ("FriendshipPoint".into(), other(PropertyType::IntProperty)),
        ("SanityValue".into(), other(PropertyType::FloatProperty)),
        ("FullStomach".into(), other(PropertyType::FloatProperty)),
        ("NickName".into(), other(PropertyType::StrProperty)),
        ("FilteredNickName".into(), other(PropertyType::StrProperty)),
        // Written as "Hp" on every path, even on a save that spells it "HP".
        ("Hp".into(), named_struct("FixedPoint64")),
        ("Hp.Value".into(), other(PropertyType::Int64Property)),
        ("OwnedTime".into(), plain_struct(StructType::DateTime)),
        ("OwnerPlayerUId".into(), plain_struct(StructType::Guid)),
        (
            "OldOwnerPlayerUIds".into(),
            Data::Array(Box::new(plain_struct(StructType::Guid))),
        ),
        (
            "LastJumpedLocation".into(),
            plain_struct(StructType::Vector),
        ),
        (
            "EquipWaza".into(),
            Data::Array(Box::new(Data::Enum(String::new(), None))),
        ),
        (
            "MasteredWaza".into(),
            Data::Array(Box::new(Data::Enum(String::new(), None))),
        ),
        (
            "PassiveSkillList".into(),
            Data::Array(Box::new(other(PropertyType::NameProperty))),
        ),
        (
            "GotWorkSuitabilityAddRankList".into(),
            struct_array("PalWorkSuitabilityInfo"),
        ),
        (
            "GotWorkSuitabilityAddRankList.WorkSuitability".into(),
            Data::Enum("EPalWorkSuitability".into(), None),
        ),
        (
            "GotWorkSuitabilityAddRankList.Rank".into(),
            other(PropertyType::IntProperty),
        ),
        // An egg's SaveParameter carries this; a pal's may not.
        (
            "FoodRegeneEffectInfo".into(),
            named_struct("PalFoodRegeneInfo"),
        ),
        (
            "FoodRegeneEffectInfo.EffectTime".into(),
            other(PropertyType::IntProperty),
        ),
    ];

    // Both lists share the element struct; their rows are created from scratch.
    for list in ["GotStatusPointList", "GotExStatusPointList"] {
        entries.push((list.into(), struct_array("PalGotStatusPoint")));
        entries.push((
            format!("{list}.StatusName"),
            other(PropertyType::NameProperty),
        ));
        entries.push((
            format!("{list}.StatusPoint"),
            other(PropertyType::IntProperty),
        ));
    }

    // Real saves spell the slot key "SlotId"; `new_pal_entry` writes "SlotID".
    for slot in ["SlotId", "SlotID"] {
        entries.push((slot.into(), named_struct("PalCharacterSlotId")));
        entries.push((
            format!("{slot}.ContainerId"),
            named_struct("PalContainerId"),
        ));
        entries.push((
            format!("{slot}.ContainerId.ID"),
            plain_struct(StructType::Guid),
        ));
        entries.push((
            format!("{slot}.SlotIndex"),
            other(PropertyType::IntProperty),
        ));
    }

    entries
}

/// Never overwrites a tag the real save recorded, so a file that already carries a
/// property keeps its own on-disk shape.
pub fn ensure_save_parameter_schemas(save: &mut crate::ue::Save, prefix: &str) {
    for (name, data) in save_parameter_schemas() {
        props::ensure_schema(
            save,
            format!("{prefix}.{name}"),
            crate::ue::PropertyTagPartial { id: None, data },
        );
    }
}

pub fn ensure_pal_property_schemas(level: &mut crate::ue::Save) {
    ensure_save_parameter_schemas(level, LEVEL_SAVE_PARAMETER_PREFIX);
}

pub fn ensure_slot_pal_schemas(save: &mut crate::ue::Save) {
    ensure_save_parameter_schemas(save, SLOT_SAVE_PARAMETER_PREFIX);
}

/// Guards every op that needs a loaded player. The message text is part of
/// the wire contract (handlers surface it verbatim), hence `CoreError::Other`
/// rather than `CoreError::PlayerNotFound`, whose `Display` differs.
fn require_loaded_player(session: &SaveSession, player_id: uuid::Uuid) -> Result<(), CoreError> {
    if session.loaded_players.contains_key(&player_id) {
        Ok(())
    } else {
        Err(CoreError::Other(format!(
            "Player {player_id} not found in the save file."
        )))
    }
}

/// The player's (pal box, party) container ids, read off the player's own
/// `.sav` rather than Level.sav.
fn player_container_ids(
    session: &SaveSession,
    player_id: uuid::Uuid,
) -> Result<(uuid::Uuid, uuid::Uuid), CoreError> {
    let loaded = session.loaded_players.get(&player_id).ok_or_else(|| {
        CoreError::Other(format!("Player {player_id} not found in the save file."))
    })?;
    let save_data = super::player::save_data_props(&loaded.sav)?;
    let pal_box_id = super::player::container_id_from(save_data, "PalStorageContainerId")
        .ok_or_else(|| CoreError::Parse("PalStorageContainerId missing".into()))?;
    let party_id = super::player::container_id_from(save_data, "OtomoCharacterContainerId")
        .ok_or_else(|| CoreError::Parse("OtomoCharacterContainerId missing".into()))?;
    Ok((pal_box_id, party_id))
}

/// `CharacterContainerSaveData` key.ID -> entry position. Built check-then-
/// build rather than via an entry closure, which would borrow `session.level`
/// inside a `&mut session.caches` borrow.
fn container_entry_index(
    session: &mut SaveSession,
    container_id: uuid::Uuid,
) -> Result<Option<usize>, CoreError> {
    if session.caches.character_container_index.is_none() {
        session.caches.character_container_index =
            Some(world::build_character_container_index(&session.level));
    }
    Ok(session
        .caches
        .character_container_index
        .as_ref()
        .expect("just built")
        .get(&container_id)
        .copied())
}

/// Registers a pal with its guild. `individual_character_handle_ids` is a
/// typed `PalGroupData` field decoded natively by `uesave`, not part of the
/// guild tail's raw blob, so no re-encoding is needed.
fn append_guild_handle(
    session: &mut SaveSession,
    guild_id: uuid::Uuid,
    instance_id: uuid::Uuid,
) -> Result<(), CoreError> {
    let Some(entry_index) = super::guild::guild_entry_index(session, guild_id)? else {
        return Ok(());
    };
    let entries = world::group_map_mut(&mut session.level)?;
    if let Some(group_data) = super::guild_tail::entry_group_data_mut(&mut entries[entry_index]) {
        group_data
            .individual_character_handle_ids
            .push(crate::ue::games::palworld::PalInstanceId {
                guid: props::uuid_to_guid(props::EMPTY_UUID),
                instance_id: props::uuid_to_guid(instance_id),
            });
    }
    Ok(())
}

/// Removes every guild handle matching `target_id` on EITHER of its two id
/// fields -- a handle can carry the target id in `guid` rather than
/// `instance_id`.
fn remove_guild_handle(
    session: &mut SaveSession,
    guild_id: uuid::Uuid,
    target_id: uuid::Uuid,
) -> Result<(), CoreError> {
    let Some(entry_index) = super::guild::guild_entry_index(session, guild_id)? else {
        return Ok(());
    };
    let entries = world::group_map_mut(&mut session.level)?;
    if let Some(group_data) = super::guild_tail::entry_group_data_mut(&mut entries[entry_index]) {
        group_data.individual_character_handle_ids.retain(|handle| {
            props::guid_to_uuid(&handle.instance_id) != target_id
                && props::guid_to_uuid(&handle.guid) != target_id
        });
    }
    Ok(())
}

/// Removes the `CharacterSaveParameterMap` entry for `pal_id`. Caches are
/// invalidated only when an entry actually moved; a `pal_id` that was never
/// present is a no-op. Performs NO ownership check -- callers must scope.
pub fn delete_pal_entry(session: &mut SaveSession, pal_id: uuid::Uuid) {
    if let Ok(entries) = world::character_map_mut(&mut session.level) {
        if let Some(position) = entries
            .iter()
            .position(|entry| world::entry_instance_id(entry) == Some(pal_id))
        {
            entries.remove(position);
            session.invalidate_performance_caches();
        }
    }
}

/// Creates a pal in the player's pal box or party.
///
/// The mutated container is the pal box only when `container_id` names it,
/// and the party otherwise -- but the caller's raw `container_id` is still
/// what gets written into the new pal's own `SlotID.ContainerId`, so an
/// unrecognized id lands the pal in the party while labelling it otherwise.
pub fn add_player_pal(
    session: &mut SaveSession,
    game_data: &GameData,
    player_id: uuid::Uuid,
    character_id: &str,
    nickname: &str,
    container_id: uuid::Uuid,
    storage_slot: Option<i32>,
) -> Result<Option<PalDto>, CoreError> {
    require_loaded_player(session, player_id)?;
    let (pal_box_id, party_id) = player_container_ids(session, player_id)?;
    let target_container_id = if container_id == pal_box_id {
        pal_box_id
    } else {
        party_id
    };
    let Some(entry_index) = container_entry_index(session, target_container_id)? else {
        return Ok(None);
    };
    let new_pal_id = uuid::Uuid::new_v4();
    let Some(slot_index) = super::containers::character_container_add_pal(
        &mut session.level,
        entry_index,
        new_pal_id,
        storage_slot,
    )?
    else {
        return Ok(None); // container full
    };
    let guild_id = super::guild::find_player_guild_id(session, player_id)?;
    let entry = new_pal_entry(
        character_id,
        new_pal_id,
        player_id,
        container_id,
        slot_index,
        guild_id,
        nickname,
        game_data,
    );
    // A freshly added pal keeps `new_pal_entry`'s placeholder Hp; it is not
    // recomputed to max_hp here.
    ensure_pal_property_schemas(&mut session.level);
    world::character_map_mut(&mut session.level)?.push(entry);
    if let Some(guild) = guild_id {
        append_guild_handle(session, guild, new_pal_id)?;
    }
    session.invalidate_performance_caches();
    let entries = world::character_map(&session.level)?;
    Ok(entries
        .last()
        .and_then(|e| pal_dto_from_entry(e, game_data)))
}

/// Creates a pal in the player's box from a full DTO, preserving every
/// stat/talent/skill it carries (the destination of a pal import/export).
/// Container and slot placement follow `add_player_pal`.
///
/// The new pal is created owned by `player_id`, but a `dto.owner_uid` of
/// `Some` overwrites that, so an imported pal keeps the owner it was stored
/// with.
pub fn add_player_pal_from_dto(
    session: &mut SaveSession,
    game_data: &GameData,
    player_id: uuid::Uuid,
    pal_dto: &PalDto,
    container_id: uuid::Uuid,
    storage_slot: Option<i32>,
) -> Result<Option<PalDto>, CoreError> {
    require_loaded_player(session, player_id)?;
    let (pal_box_id, party_id) = player_container_ids(session, player_id)?;
    let target_container_id = if container_id == pal_box_id {
        pal_box_id
    } else {
        party_id
    };
    let Some(entry_index) = container_entry_index(session, target_container_id)? else {
        return Ok(None);
    };
    let new_pal_id = uuid::Uuid::new_v4();
    let Some(slot_index) = super::containers::character_container_add_pal(
        &mut session.level,
        entry_index,
        new_pal_id,
        storage_slot,
    )?
    else {
        return Ok(None); // container full
    };
    let guild_id = super::guild::find_player_guild_id(session, player_id)?;
    let mut entry = new_pal_entry(
        &pal_dto.character_id,
        new_pal_id,
        player_id,
        container_id,
        slot_index,
        guild_id,
        pal_dto.nickname.as_deref().unwrap_or(&pal_dto.character_id),
        game_data,
    );
    let mut incoming = pal_dto.clone();
    incoming.instance_id = new_pal_id;
    incoming.storage_id = container_id; // inert: apply_pal_dto never moves ContainerId
    incoming.storage_slot = slot_index as i64;
    if let Some(save_parameter) = world::entry_save_parameter_mut(&mut entry) {
        apply_pal_dto(save_parameter, &incoming, false, game_data);
    }
    ensure_pal_property_schemas(&mut session.level);
    world::character_map_mut(&mut session.level)?.push(entry);
    if let Some(guild) = guild_id {
        append_guild_handle(session, guild, new_pal_id)?;
    }
    session.invalidate_performance_caches();
    let entries = world::character_map(&session.level)?;
    Ok(entries
        .last()
        .and_then(|e| pal_dto_from_entry(e, game_data)))
}

/// Creates a pal in a guild base's worker container.
///
/// A base pal has no player owner, but `OwnerPlayerUId` is still written --
/// as the nil guid, not removed. The game accepts this and it is what the
/// save format carries for base pals.
pub fn add_guild_pal(
    session: &mut SaveSession,
    game_data: &GameData,
    guild_id: uuid::Uuid,
    base_id: uuid::Uuid,
    character_id: &str,
    nickname: &str,
    storage_slot: Option<i32>,
) -> Result<Option<PalDto>, CoreError> {
    if !session.loaded_guilds.contains(&guild_id) {
        return Err(CoreError::Other(format!(
            "Guild {guild_id} not found in the save file."
        )));
    }
    let base_camp_entries = world::base_camp_map(&session.level)?
        .map(|entries| entries.as_slice())
        .unwrap_or(&[]);
    let Some((base_guild, worker_container_id)) = base_camp_entries
        .iter()
        .find(|entry| props::as_uuid(&entry.key) == Some(base_id))
        .and_then(super::guild::base_guild_and_container)
    else {
        return Err(CoreError::Other(format!(
            "Base {base_id} not found in the guild {guild_id}."
        )));
    };
    let Some(entry_index) = container_entry_index(session, worker_container_id)? else {
        return Ok(None);
    };
    let new_pal_id = uuid::Uuid::new_v4();
    let Some(slot_index) = super::containers::character_container_add_pal(
        &mut session.level,
        entry_index,
        new_pal_id,
        storage_slot,
    )?
    else {
        return Ok(None);
    };
    let entry = new_pal_entry(
        character_id,
        new_pal_id,
        props::EMPTY_UUID,
        worker_container_id,
        slot_index,
        Some(base_guild),
        nickname,
        game_data,
    );
    // Keeps `new_pal_entry`'s placeholder Hp, as `add_player_pal` does.
    ensure_pal_property_schemas(&mut session.level);
    world::character_map_mut(&mut session.level)?.push(entry);
    append_guild_handle(session, guild_id, new_pal_id)?;
    session.invalidate_performance_caches();
    let entries = world::character_map(&session.level)?;
    Ok(entries
        .last()
        .and_then(|e| pal_dto_from_entry(e, game_data)))
}

/// Clones one of a player's own pals into their pal box.
///
/// Two quirks of this operation, both deliberate:
/// - Slot index 0 is treated as "no free slot" and the clone is refused.
/// - A refused clone (slot 0, or a source pal this player does not own)
///   leaves the reserved slot behind in the pal box, referencing a pal id
///   that never gets created. Callers depend on that state; do not clean it.
pub fn clone_pal(
    session: &mut SaveSession,
    game_data: &GameData,
    dto: &PalDto,
) -> Result<Option<PalDto>, CoreError> {
    // The literal "None" in this message is part of the wire contract.
    let owner_id = match dto.owner_uid {
        Some(id) => id,
        None => {
            return Err(CoreError::Other(
                "Player None not found in the save file.".to_string(),
            ))
        }
    };
    require_loaded_player(session, owner_id)?;
    let (pal_box_id, _) = player_container_ids(session, owner_id)?;
    let Some(container_index) = container_entry_index(session, pal_box_id)? else {
        return Ok(None);
    };
    let new_pal_id = uuid::Uuid::new_v4();
    let Some(slot_index) = super::containers::character_container_add_pal(
        &mut session.level,
        container_index,
        new_pal_id,
        None,
    )?
    else {
        return Ok(None); // pal box has no free slot at all
    };
    if slot_index == 0 {
        return Ok(None); // slot 0 is refused -- see this function's doc comment
    }
    let source_entry = {
        let entries = world::character_map(&session.level)?;
        entries
            .iter()
            .find(|entry| {
                world::entry_instance_id(entry) == Some(dto.instance_id)
                    && world::entry_save_parameter(entry)
                        .and_then(|params| param(params, "OwnerPlayerUId").and_then(props::as_uuid))
                        == Some(owner_id)
            })
            .cloned()
    };
    let Some(mut cloned_entry) = source_entry else {
        return Ok(None);
    };
    // A clone gets a fresh instance id and a nil key PlayerUId.
    if let Some(key_props) = props::struct_props_mut(&mut cloned_entry.key) {
        key_props.insert("InstanceId", props::guid_property(new_pal_id));
        key_props.insert("PlayerUId", props::guid_property(props::EMPTY_UUID));
    }
    let nickname = dto
        .nickname
        .clone()
        .unwrap_or_else(|| dto.character_id.clone());
    if let Some(save_parameter) = world::entry_save_parameter_mut(&mut cloned_entry) {
        save_parameter.insert("NickName", props::str_property(&nickname));
        // SlotIndex only; the clone inherits the source pal's ContainerId.
        let slot_key = if save_parameter.0.contains_key(&PropertyKey::from("SlotID")) {
            "SlotID"
        } else {
            "SlotId"
        };
        if let Some(slot_struct) = save_parameter
            .0
            .get_mut(&PropertyKey::from(slot_key))
            .and_then(props::struct_props_mut)
        {
            slot_struct.insert("SlotIndex", props::int_property(slot_index));
        }
    }
    world::character_map_mut(&mut session.level)?.push(cloned_entry);
    if let Some(guild_id) = super::guild::find_player_guild_id(session, owner_id)? {
        append_guild_handle(session, guild_id, new_pal_id)?;
    }
    session.invalidate_performance_caches();
    let entries = world::character_map(&session.level)?;
    Ok(entries
        .last()
        .and_then(|e| pal_dto_from_entry(e, game_data)))
}

/// Clones a pal within a guild base's worker container.
///
/// Unlike `clone_pal`, slot index 0 is a valid destination here. The source
/// pal must already be a member of this base's container.
pub fn clone_guild_pal(
    session: &mut SaveSession,
    game_data: &GameData,
    guild_id: uuid::Uuid,
    base_id: uuid::Uuid,
    dto: &PalDto,
) -> Result<Option<PalDto>, CoreError> {
    if !session.loaded_guilds.contains(&guild_id) {
        return Err(CoreError::Other(format!(
            "Base {base_id} not found in the guild {guild_id}."
        )));
    }
    let base_camp_entries = world::base_camp_map(&session.level)?
        .map(|entries| entries.as_slice())
        .unwrap_or(&[]);
    let Some((_, worker_container_id)) = base_camp_entries
        .iter()
        .find(|entry| props::as_uuid(&entry.key) == Some(base_id))
        .and_then(super::guild::base_guild_and_container)
    else {
        return Err(CoreError::Other(format!(
            "Base {base_id} not found in the guild {guild_id}."
        )));
    };
    let Some(container_index) = container_entry_index(session, worker_container_id)? else {
        return Ok(None);
    };
    let new_pal_id = uuid::Uuid::new_v4();
    let Some(slot_index) = super::containers::character_container_add_pal(
        &mut session.level,
        container_index,
        new_pal_id,
        None,
    )?
    else {
        return Ok(None);
    };
    let source_entry = {
        let entries = world::character_map(&session.level)?;
        entries
            .iter()
            .find(|entry| {
                world::entry_instance_id(entry) == Some(dto.instance_id)
                    && world::entry_save_parameter(entry)
                        .map(super::guild::base_container_membership)
                        == Some(Some(worker_container_id))
            })
            .cloned()
    };
    let Some(mut cloned_entry) = source_entry else {
        return Ok(None);
    };
    if let Some(key_props) = props::struct_props_mut(&mut cloned_entry.key) {
        key_props.insert("InstanceId", props::guid_property(new_pal_id));
        key_props.insert("PlayerUId", props::guid_property(props::EMPTY_UUID));
    }
    let nickname = dto
        .nickname
        .clone()
        .unwrap_or_else(|| dto.character_id.clone());
    if let Some(save_parameter) = world::entry_save_parameter_mut(&mut cloned_entry) {
        save_parameter.insert("NickName", props::str_property(&nickname));
        // `OwnerPlayerUId` is left on the clone, as `add_guild_pal` leaves it.
        let slot_key = if save_parameter.0.contains_key(&PropertyKey::from("SlotID")) {
            "SlotID"
        } else {
            "SlotId"
        };
        if let Some(slot_struct) = save_parameter
            .0
            .get_mut(&PropertyKey::from(slot_key))
            .and_then(props::struct_props_mut)
        {
            slot_struct.insert("SlotIndex", props::int_property(slot_index));
        }
    }
    world::character_map_mut(&mut session.level)?.push(cloned_entry);
    append_guild_handle(session, guild_id, new_pal_id)?;
    session.invalidate_performance_caches();
    let entries = world::character_map(&session.level)?;
    Ok(entries
        .last()
        .and_then(|e| pal_dto_from_entry(e, game_data)))
}

/// Moves one of a player's own pals between their pal box and party.
///
/// Ownership is checked BEFORE any container is touched: a bogus `pal_id`
/// must not leave a phantom slot reserved for a pal that was never there.
pub fn move_pal(
    session: &mut SaveSession,
    game_data: &GameData,
    player_id: uuid::Uuid,
    pal_id: uuid::Uuid,
    container_id: uuid::Uuid,
) -> Result<Option<PalDto>, CoreError> {
    require_loaded_player(session, player_id)?;
    let owns_pal = world::character_map(&session.level)?.iter().any(|entry| {
        !world::entry_is_player(entry)
            && world::entry_instance_id(entry) == Some(pal_id)
            && world::entry_save_parameter(entry)
                .and_then(|params| param(params, "OwnerPlayerUId").and_then(props::as_uuid))
                == Some(player_id)
    });
    if !owns_pal {
        return Err(CoreError::PalNotFound(pal_id));
    }
    let (pal_box_id, party_id) = player_container_ids(session, player_id)?;
    let (source_id, target_id) = if container_id == pal_box_id {
        (party_id, pal_box_id)
    } else if container_id == party_id {
        (pal_box_id, party_id)
    } else {
        return Ok(None); // container id belongs to neither the box nor the party
    };
    let Some(target_index) = container_entry_index(session, target_id)? else {
        return Ok(None);
    };
    let Some(source_index) = container_entry_index(session, source_id)? else {
        return Ok(None);
    };
    let Some(slot_index) = super::containers::character_container_add_pal(
        &mut session.level,
        target_index,
        pal_id,
        None,
    )?
    else {
        return Ok(None); // target full -> handler warning "Pal container is full"
    };
    super::containers::character_container_remove_pal(&mut session.level, source_index, pal_id)?;
    // The pal's own ContainerId is left as-is; only SlotIndex is rewritten.
    let entries = world::character_map_mut(&mut session.level)?;
    if let Some(entry) = entries
        .iter_mut()
        .find(|entry| world::entry_instance_id(entry) == Some(pal_id))
    {
        if let Some(save_parameter) = world::entry_save_parameter_mut(entry) {
            let slot_key = if save_parameter.0.contains_key(&PropertyKey::from("SlotID")) {
                "SlotID"
            } else {
                "SlotId"
            };
            if let Some(slot_struct) = save_parameter
                .0
                .get_mut(&PropertyKey::from(slot_key))
                .and_then(props::struct_props_mut)
            {
                slot_struct.insert("SlotIndex", props::int_property(slot_index));
            }
        }
    }
    let entries = world::character_map(&session.level)?;
    Ok(entries
        .iter()
        .find(|entry| world::entry_instance_id(entry) == Some(pal_id))
        .and_then(|entry| pal_dto_from_entry(entry, game_data)))
}

/// A player owns a pal iff its `OwnerPlayerUId` matches.
fn pal_owned_by_player(
    session: &SaveSession,
    pal_id: uuid::Uuid,
    player_id: uuid::Uuid,
) -> Result<bool, CoreError> {
    Ok(world::character_map(&session.level)?.iter().any(|entry| {
        !world::entry_is_player(entry)
            && world::entry_instance_id(entry) == Some(pal_id)
            && world::entry_save_parameter(entry)
                .and_then(|params| param(params, "OwnerPlayerUId").and_then(props::as_uuid))
                == Some(player_id)
    }))
}

/// Whether `pal_id` occupies a slot in the container at `container_index`,
/// per the container's own `Slots` array.
///
/// Membership is checked against the container rather than against the pal's
/// own `SlotId` property (`guild::base_container_membership`): a pal created
/// this session spells that key "SlotID" and would not be recognized, which
/// would wrongly forbid deleting a pal right after adding it. The `Slots`
/// array still correctly rejects a pal belonging to a different base.
fn pal_in_character_container(
    level: &crate::ue::Save,
    container_index: usize,
    pal_id: uuid::Uuid,
) -> bool {
    super::containers::read_character_container(level, container_index)
        .map(|view| view.slots.iter().any(|slot| slot.pal_id == Some(pal_id)))
        .unwrap_or(false)
}

/// Deletes pals owned by `player_id`: from both containers (removal from a
/// container that never held the pal is a no-op), from the guild handles, then
/// the `CharacterSaveParameterMap` entry itself.
///
/// Ownership is checked BEFORE any mutation and a pal this player does not own
/// is a hard `PalNotFound`, not a skip: `delete_pal_entry` matches on instance
/// id alone, so without this guard one player could delete another's pal.
pub fn delete_player_pals(
    session: &mut SaveSession,
    player_id: uuid::Uuid,
    pal_ids: &[uuid::Uuid],
) -> Result<(), CoreError> {
    require_loaded_player(session, player_id)?;
    let (pal_box_id, party_id) = player_container_ids(session, player_id)?;
    let guild_id = super::guild::find_player_guild_id(session, player_id)?;
    for pal_id in pal_ids {
        if !pal_owned_by_player(session, *pal_id, player_id)? {
            return Err(CoreError::PalNotFound(*pal_id));
        }
        for container_id in [pal_box_id, party_id] {
            if let Some(container_index) = container_entry_index(session, container_id)? {
                super::containers::character_container_remove_pal(
                    &mut session.level,
                    container_index,
                    *pal_id,
                )?;
            }
        }
        if let Some(guild) = guild_id {
            remove_guild_handle(session, guild, *pal_id)?;
        }
        delete_pal_entry(session, *pal_id);
    }
    Ok(())
}

/// Deletes pals from a guild base's worker container.
///
/// The "Base ... not found in the guild ..." error fires when the GUILD
/// doesn't resolve; an unresolvable `base_id` within a loaded guild is
/// tolerated (guild handle and character-map entry are still removed).
///
/// Base membership is checked BEFORE any mutation and a non-member pal is a
/// hard `PalNotFound`, for the same reason as `delete_player_pals`.
pub fn delete_guild_pals(
    session: &mut SaveSession,
    guild_id: uuid::Uuid,
    base_id: uuid::Uuid,
    pal_ids: &[uuid::Uuid],
) -> Result<(), CoreError> {
    if !session.loaded_guilds.contains(&guild_id) {
        return Err(CoreError::Other(format!(
            "Base {base_id} not found in the guild {guild_id}."
        )));
    }
    let worker_container_id = world::base_camp_map(&session.level)?
        .map(|entries| entries.as_slice())
        .unwrap_or(&[])
        .iter()
        .find(|entry| props::as_uuid(&entry.key) == Some(base_id))
        .and_then(super::guild::base_guild_and_container)
        .map(|(_, container)| container);
    for pal_id in pal_ids {
        if let Some(container_id) = worker_container_id {
            if let Some(container_index) = container_entry_index(session, container_id)? {
                if !pal_in_character_container(&session.level, container_index, *pal_id) {
                    return Err(CoreError::PalNotFound(*pal_id));
                }
                super::containers::character_container_remove_pal(
                    &mut session.level,
                    container_index,
                    *pal_id,
                )?;
            }
        }
        remove_guild_handle(session, guild_id, *pal_id)?;
        delete_pal_entry(session, *pal_id);
    }
    Ok(())
}

/// Heals each pal in `pal_ids`. A pal id that doesn't resolve is skipped, not
/// an error. Player entries are never touched.
pub fn heal_pals(
    session: &mut SaveSession,
    game_data: &GameData,
    pal_ids: &[uuid::Uuid],
) -> Result<(), CoreError> {
    let target_ids: HashSet<uuid::Uuid> = pal_ids.iter().copied().collect();
    let entries = world::character_map_mut(&mut session.level)?;
    for entry in entries.iter_mut() {
        if world::entry_is_player(entry) {
            continue;
        }
        let Some(instance_id) = world::entry_instance_id(entry) else {
            continue;
        };
        if !target_ids.contains(&instance_id) {
            continue;
        }
        let character_id = world::entry_save_parameter(entry)
            .and_then(|params| param(params, "CharacterID").and_then(props::as_str))
            .unwrap_or("")
            .to_string();
        if let Some(save_parameter) = world::entry_save_parameter_mut(entry) {
            heal_save_parameter(save_parameter, &character_id, game_data);
        }
    }
    Ok(())
}

/// Heals every pal owned by `player_id`.
pub fn heal_all_player_pals(
    session: &mut SaveSession,
    game_data: &GameData,
    player_id: uuid::Uuid,
) -> Result<(), CoreError> {
    require_loaded_player(session, player_id)?;
    let owned_ids: Vec<uuid::Uuid> = world::character_map(&session.level)?
        .iter()
        .filter(|entry| !world::entry_is_player(entry))
        .filter(|entry| {
            world::entry_save_parameter(entry)
                .and_then(|params| param(params, "OwnerPlayerUId").and_then(props::as_uuid))
                == Some(player_id)
        })
        .filter_map(world::entry_instance_id)
        .collect();
    heal_pals(session, game_data, &owned_ids)
}

/// Heals every pal in a base's worker container. Membership uses
/// `guild::base_container_membership`'s SlotId-only rule, not the
/// SlotId-or-SlotID fallback the read path uses for storage ids.
pub fn heal_all_base_pals(
    session: &mut SaveSession,
    game_data: &GameData,
    guild_id: uuid::Uuid,
    base_id: uuid::Uuid,
) -> Result<(), CoreError> {
    if !session.loaded_guilds.contains(&guild_id) {
        return Err(CoreError::Other(format!(
            "Base {base_id} not found in the guild {guild_id}."
        )));
    }
    let base_camp_entries = world::base_camp_map(&session.level)?
        .map(|entries| entries.as_slice())
        .unwrap_or(&[]);
    let Some((_, worker_container_id)) = base_camp_entries
        .iter()
        .find(|entry| props::as_uuid(&entry.key) == Some(base_id))
        .and_then(super::guild::base_guild_and_container)
    else {
        return Err(CoreError::Other(format!(
            "Base {base_id} not found in the guild {guild_id}."
        )));
    };
    let base_pal_ids: Vec<uuid::Uuid> = world::character_map(&session.level)?
        .iter()
        .filter(|entry| !world::entry_is_player(entry))
        .filter_map(|entry| {
            let save_parameter = world::entry_save_parameter(entry)?;
            if super::guild::base_container_membership(save_parameter) == Some(worker_container_id)
            {
                world::entry_instance_id(entry)
            } else {
                None
            }
        })
        .collect();
    heal_pals(session, game_data, &base_pal_ids)
}

// DPS ops read and write `SaveParameterArray` in the player's own `_dps.sav`,
// never `Level.sav`, so none of them invalidate `session.caches`: nothing in
// `WorldCaches` indexes DPS data.

fn dps_slots_mut(loaded: &mut crate::session::LoadedPlayer) -> Option<&mut Vec<StructValue>> {
    let dps_save = loaded.dps.as_mut()?;
    props::struct_values_mut(
        dps_save
            .root
            .properties
            .0
            .get_mut(&PropertyKey::from("SaveParameterArray"))?,
    )
}

/// The first slot whose `CharacterID` is absent or the literal `"None"` --
/// DPS slots are recycled in place, never removed, so that is what "empty"
/// looks like.
fn first_empty_dps_slot(slots: &[StructValue]) -> Option<usize> {
    slots.iter().position(|slot| {
        let StructValue::Struct(slot_props) = slot else {
            return false;
        };
        let Some(save_parameter) = slot_props
            .0
            .get(&PropertyKey::from("SaveParameter"))
            .and_then(props::struct_props)
        else {
            return true;
        };
        match param(save_parameter, "CharacterID").and_then(props::as_str) {
            None => true,
            Some("None") => true,
            Some(_) => false,
        }
    })
}

/// Empties a DPS/GPS slot's `SaveParameter` bag in place. Shared with
/// `domain::gps`, whose slots have the identical layout.
///
/// - `IsRarePal` is deliberately left in place: a recycled slot keeps the
///   previous occupant's luck flag.
/// - The status-point lists are cleared in place rather than reinserted, so
///   the arrays keep the tag metadata they were parsed with.
pub(crate) fn reset_dps_save_parameter(save_parameter: &mut Properties) {
    save_parameter.insert("CharacterID", props::name_property("None"));
    save_parameter.insert("NickName", props::str_property(""));
    save_parameter.insert("FilteredNickName", props::str_property(""));
    save_parameter.insert("OwnerPlayerUId", props::guid_property(props::EMPTY_UUID));
    save_parameter.insert("Hp", props::fixed_point64_property(0));
    for name in [
        "Exp",
        "Level",
        "Rank",
        "Rank_HP",
        "Rank_Attack",
        "Rank_Defence",
        "Rank_CraftSpeed",
        "MasteredWaza",
        "GotWorkSuitabilityAddRankList",
    ] {
        save_parameter.0.shift_remove(&PropertyKey::from(name));
    }
    save_parameter.insert("Talent_HP", props::byte_property(0));
    save_parameter.insert("Talent_Shot", props::byte_property(0));
    save_parameter.insert("Talent_Defense", props::byte_property(0));
    save_parameter.insert("EquipWaza", props::enum_array_property(vec![]));
    save_parameter.insert("PassiveSkillList", props::name_array_property(vec![]));
    // SlotIndex -1 marks the slot vacant; ContainerId is left as it was.
    let slot_key = if save_parameter.0.contains_key(&PropertyKey::from("SlotID")) {
        "SlotID"
    } else {
        "SlotId"
    };
    if let Some(slot_struct) = save_parameter
        .0
        .get_mut(&PropertyKey::from(slot_key))
        .and_then(props::struct_props_mut)
    {
        slot_struct.insert("SlotIndex", props::int_property(-1));
    }
    for list_name in ["GotStatusPointList", "GotExStatusPointList"] {
        if let Some(values) = save_parameter
            .0
            .get_mut(&PropertyKey::from(list_name))
            .and_then(props::struct_values_mut)
        {
            values.clear();
        }
    }
}

/// Creates a pal in a player's Dimensional Palbox slot. `Ok(None)` when every
/// slot is occupied.
pub fn add_player_dps_pal(
    session: &mut SaveSession,
    game_data: &GameData,
    player_id: uuid::Uuid,
    character_id: &str,
    nickname: &str,
    storage_slot: Option<i32>,
) -> Result<Option<(i32, PalDto)>, CoreError> {
    require_loaded_player(session, player_id)?;
    let loaded = session.loaded_players.get_mut(&player_id).expect("checked");
    let Some(slots) = dps_slots_mut(loaded) else {
        return Ok(None);
    };
    let slot_index = match storage_slot {
        Some(requested) if requested >= 0 && (requested as usize) < slots.len() => {
            requested as usize
        }
        Some(_) => return Ok(None),
        None => match first_empty_dps_slot(slots) {
            Some(found) => found,
            None => return Ok(None),
        },
    };
    let new_instance_id = uuid::Uuid::new_v4();
    {
        let StructValue::Struct(slot_props) = &mut slots[slot_index] else {
            return Ok(None);
        };
        if let Some(id_struct) = slot_props
            .0
            .get_mut(&PropertyKey::from("InstanceId"))
            .and_then(props::struct_props_mut)
        {
            id_struct.insert("InstanceId", props::guid_property(new_instance_id));
        }
        let Some(save_parameter) = slot_props
            .0
            .get_mut(&PropertyKey::from("SaveParameter"))
            .and_then(props::struct_props_mut)
        else {
            return Ok(None);
        };
        // FullStomach is sized from the slot's PREVIOUS occupant, not the new
        // species -- so a recycled slot can start off with the wrong species'
        // max. This is the contract; nothing rewrites it afterwards.
        let previous_character_id = param(save_parameter, "CharacterID")
            .and_then(props::as_str)
            .unwrap_or("")
            .to_string();
        reset_dps_save_parameter(save_parameter);
        save_parameter.insert(
            "FullStomach",
            props::float_property(max_stomach_for(&previous_character_id, game_data) as f32),
        );
        save_parameter.insert("OwnerPlayerUId", props::guid_property(player_id));
        save_parameter.insert("CharacterID", props::name_property(character_id));
        save_parameter.insert("NickName", props::str_property(nickname));
        save_parameter.insert("FilteredNickName", props::str_property(nickname));
        save_parameter.insert("Gender", props::enum_property("EPalGenderType::Female"));
        let slot_key = if save_parameter.0.contains_key(&PropertyKey::from("SlotID")) {
            "SlotID"
        } else {
            "SlotId"
        };
        if let Some(slot_struct) = save_parameter
            .0
            .get_mut(&PropertyKey::from(slot_key))
            .and_then(props::struct_props_mut)
        {
            slot_struct.insert("SlotIndex", props::int_property(0));
        }
        save_parameter.insert("GotStatusPointList", status_point_structs(&STATUS_NAMES));
        save_parameter.insert(
            "GotExStatusPointList",
            status_point_structs(&EX_STATUS_NAMES),
        );
        let dto = read_save_parameter_dto(save_parameter, new_instance_id, true, game_data);
        let boosted = dto.is_boss.unwrap_or(false) || dto.is_lucky.unwrap_or(false);
        save_parameter.insert(
            "Hp",
            props::fixed_point64_property(max_hp_for(&dto, boosted, game_data)),
        );
    }
    let loaded = session.loaded_players.get(&player_id).expect("checked");
    let dps_save = loaded.dps.as_ref().expect("checked");
    let slots = props::struct_values(
        dps_save
            .root
            .properties
            .0
            .get(&PropertyKey::from("SaveParameterArray"))
            .unwrap(),
    )
    .unwrap();
    Ok(pal_dto_from_dps_slot(&slots[slot_index], game_data).map(|dto| (slot_index as i32, dto)))
}

/// Creates a pal in a player's DPS slot from a full DTO (a pal import). The
/// pal is always reowned to the destination player, unlike the GPS variant,
/// which clears the owner. `apply_pal_dto` writes Hp, so none is written here.
pub fn add_player_dps_pal_from_dto(
    session: &mut SaveSession,
    game_data: &GameData,
    player_id: uuid::Uuid,
    pal_dto: &PalDto,
    storage_slot: Option<i32>,
) -> Result<Option<(i32, PalDto)>, CoreError> {
    require_loaded_player(session, player_id)?;
    let (pal_box_id, _party_id) = player_container_ids(session, player_id)?;
    let loaded = session.loaded_players.get_mut(&player_id).expect("checked");
    let Some(slots) = dps_slots_mut(loaded) else {
        return Ok(None);
    };
    let slot_index = match storage_slot {
        Some(requested) if requested >= 0 && (requested as usize) < slots.len() => {
            requested as usize
        }
        Some(_) => return Ok(None),
        None => match first_empty_dps_slot(slots) {
            Some(found) => found,
            None => return Ok(None),
        },
    };
    let new_instance_id = uuid::Uuid::new_v4();
    let mut incoming = pal_dto.clone();
    incoming.owner_uid = Some(player_id);
    incoming.instance_id = new_instance_id;
    incoming.storage_id = pal_box_id; // inert: apply_pal_dto never moves ContainerId
    incoming.storage_slot = 0;
    {
        let StructValue::Struct(slot_props) = &mut slots[slot_index] else {
            return Ok(None);
        };
        if let Some(id_struct) = slot_props
            .0
            .get_mut(&PropertyKey::from("InstanceId"))
            .and_then(props::struct_props_mut)
        {
            id_struct.insert("InstanceId", props::guid_property(new_instance_id));
        }
        let Some(save_parameter) = slot_props
            .0
            .get_mut(&PropertyKey::from("SaveParameter"))
            .and_then(props::struct_props_mut)
        else {
            return Ok(None);
        };
        apply_pal_dto(save_parameter, &incoming, true, game_data);
        save_parameter.insert("GotStatusPointList", status_point_structs(&STATUS_NAMES));
        save_parameter.insert(
            "GotExStatusPointList",
            status_point_structs(&EX_STATUS_NAMES),
        );
    }
    let loaded = session.loaded_players.get(&player_id).expect("checked");
    let dps_save = loaded.dps.as_ref().expect("checked");
    let slots = props::struct_values(
        dps_save
            .root
            .properties
            .0
            .get(&PropertyKey::from("SaveParameterArray"))
            .unwrap(),
    )
    .unwrap();
    Ok(pal_dto_from_dps_slot(&slots[slot_index], game_data).map(|dto| (slot_index as i32, dto)))
}

/// Clones a DPS pal into the owner's first empty DPS slot.
pub fn clone_dps_pal(
    session: &mut SaveSession,
    game_data: &GameData,
    dto: &PalDto,
) -> Result<Option<(i32, PalDto)>, CoreError> {
    let owner_id = match dto.owner_uid {
        Some(id) => id,
        None => {
            return Err(CoreError::Other(
                "Player None not found in the save file.".to_string(),
            ))
        }
    };
    require_loaded_player(session, owner_id)?;
    let loaded = session.loaded_players.get_mut(&owner_id).expect("checked");
    let Some(slots) = dps_slots_mut(loaded) else {
        return Ok(None);
    };
    let Some(slot_index) = first_empty_dps_slot(slots) else {
        return Ok(None);
    };
    let new_instance_id = uuid::Uuid::new_v4();
    {
        let StructValue::Struct(slot_props) = &mut slots[slot_index] else {
            return Ok(None);
        };
        if let Some(id_struct) = slot_props
            .0
            .get_mut(&PropertyKey::from("InstanceId"))
            .and_then(props::struct_props_mut)
        {
            id_struct.insert("InstanceId", props::guid_property(new_instance_id));
        }
        let Some(save_parameter) = slot_props
            .0
            .get_mut(&PropertyKey::from("SaveParameter"))
            .and_then(props::struct_props_mut)
        else {
            return Ok(None);
        };
        apply_pal_dto(save_parameter, dto, true, game_data);
        save_parameter.insert("GotStatusPointList", status_point_structs(&STATUS_NAMES));
        save_parameter.insert(
            "GotExStatusPointList",
            status_point_structs(&EX_STATUS_NAMES),
        );
        let reread = read_save_parameter_dto(save_parameter, new_instance_id, true, game_data);
        let boosted = reread.is_boss.unwrap_or(false) || reread.is_lucky.unwrap_or(false);
        save_parameter.insert(
            "Hp",
            props::fixed_point64_property(max_hp_for(&reread, boosted, game_data)),
        );
    }
    let loaded = session.loaded_players.get(&owner_id).expect("checked");
    let dps_save = loaded.dps.as_ref().expect("checked");
    let slots = props::struct_values(
        dps_save
            .root
            .properties
            .0
            .get(&PropertyKey::from("SaveParameterArray"))
            .unwrap(),
    )
    .unwrap();
    Ok(pal_dto_from_dps_slot(&slots[slot_index], game_data).map(|dto| (slot_index as i32, dto)))
}

/// Empties the given DPS slots in place: the slot's outer `InstanceId` is
/// nilled and its `SaveParameter` bag is reset.
pub fn delete_player_dps_pals(
    session: &mut SaveSession,
    _game_data: &GameData,
    player_id: uuid::Uuid,
    pal_indexes: &[i32],
) -> Result<(), CoreError> {
    require_loaded_player(session, player_id)?;
    let loaded = session.loaded_players.get_mut(&player_id).expect("checked");
    let Some(slots) = dps_slots_mut(loaded) else {
        return Ok(());
    };
    let mut sorted_indexes: Vec<i32> = pal_indexes.to_vec();
    sorted_indexes.sort_unstable_by(|a, b| b.cmp(a));
    for index in sorted_indexes {
        if index < 0 {
            continue;
        }
        let Some(StructValue::Struct(slot_props)) = slots.get_mut(index as usize) else {
            continue;
        };
        if let Some(id_struct) = slot_props
            .0
            .get_mut(&PropertyKey::from("InstanceId"))
            .and_then(props::struct_props_mut)
        {
            id_struct.insert("InstanceId", props::guid_property(props::EMPTY_UUID));
        }
        if let Some(save_parameter) = slot_props
            .0
            .get_mut(&PropertyKey::from("SaveParameter"))
            .and_then(props::struct_props_mut)
        {
            reset_dps_save_parameter(save_parameter);
        }
    }
    Ok(())
}

/// Applies each DTO in `modified_pals` onto its `CharacterSaveParameterMap`
/// entry. Deliberately NOT ownership-scoped: an edit addresses a pal by id
/// regardless of who owns it.
///
/// Never invalidates `session.caches`: entries are mutated in place, never
/// inserted or removed, so every cached position still resolves.
pub fn update_pals(
    session: &mut SaveSession,
    game_data: &GameData,
    modified_pals: &OrderedMap<uuid::Uuid, PalDto>,
    progress: &crate::progress::ProgressSink,
) -> Result<(), CoreError> {
    for (pal_id, dto) in modified_pals.iter() {
        let display_name = dto
            .nickname
            .clone()
            .unwrap_or_else(|| dto.character_id.clone());
        progress(&format!("Updating pal {display_name}"));
        let entries = world::character_map_mut(&mut session.level)?;
        let Some(entry) = entries
            .iter_mut()
            .find(|entry| world::entry_instance_id(entry) == Some(*pal_id))
        else {
            return Err(CoreError::PalNotFound(*pal_id));
        };
        if let Some(save_parameter) = world::entry_save_parameter_mut(entry) {
            apply_pal_dto(save_parameter, dto, false, game_data);
        }
    }
    ensure_pal_property_schemas(&mut session.level);
    progress("Saving changes to file");
    Ok(())
}

/// Applies each DTO in `modified_dps_pals` onto the owning player's DPS slot.
/// A DTO whose owner, DPS file, or slot index doesn't resolve is skipped.
pub fn update_dps_pals(
    session: &mut SaveSession,
    game_data: &GameData,
    modified_dps_pals: &OrderedMap<i32, PalDto>,
    progress: &crate::progress::ProgressSink,
) -> Result<(), CoreError> {
    for (slot_index, dto) in modified_dps_pals.iter() {
        let display_name = dto
            .nickname
            .clone()
            .unwrap_or_else(|| dto.character_id.clone());
        progress(&format!("Updating DPS pal {display_name}"));
        let Some(owner_id) = dto.owner_uid else {
            continue;
        };
        if !session.loaded_players.contains_key(&owner_id) {
            continue;
        }
        let loaded = session.loaded_players.get_mut(&owner_id).expect("checked");
        let Some(slots) = dps_slots_mut(loaded) else {
            continue;
        };
        let Some(StructValue::Struct(slot_props)) = slots.get_mut(*slot_index as usize) else {
            continue;
        };
        if let Some(save_parameter) = slot_props
            .0
            .get_mut(&PropertyKey::from("SaveParameter"))
            .and_then(props::struct_props_mut)
        {
            apply_pal_dto(save_parameter, dto, true, game_data);
        }
    }
    progress("Saving changes to file");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ue::games::palworld::PalCharacterData;
    use crate::ue::{Byte, Properties, Property, StructValue};

    fn game_data() -> GameData {
        let json_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../data/json");
        GameData::load(&json_dir).expect("data dir")
    }

    fn fguid(text: &str) -> crate::ue::FGuid {
        serde_json::from_value(serde_json::Value::String(text.to_string())).unwrap()
    }

    fn guid_property(text: &str) -> Property {
        Property::Struct(StructValue::Guid(fguid(text)))
    }

    fn character_entry(
        instance_id: &str,
        save_parameter: Properties,
        group_id: crate::ue::FGuid,
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
            Property::Struct(StructValue::Game(crate::ue::PalStruct::CharacterData(character_data))),
        );

        MapEntry {
            key: Property::Struct(StructValue::Struct(key_properties)),
            value: Property::Struct(StructValue::Struct(value_properties)),
        }
    }

    /// No fixture save carries a `_dps.sav`, so this synthetic session is the
    /// only coverage `update_dps_pals` gets.
    fn session_with_one_dps_slot(
        owner_id: uuid::Uuid,
        character_id: &str,
        instance_id: uuid::Uuid,
    ) -> SaveSession {
        let mut save_parameter = Properties::default();
        save_parameter.insert("CharacterID", props::name_property(character_id));
        save_parameter.insert("Gender", props::enum_property("EPalGenderType::Female"));
        save_parameter.insert("Level", props::byte_property(1));
        let mut id_struct = Properties::default();
        id_struct.insert("InstanceId", props::guid_property(instance_id));
        let mut slot_props = Properties::default();
        slot_props.insert(
            "InstanceId",
            Property::Struct(StructValue::Struct(id_struct)),
        );
        slot_props.insert(
            "SaveParameter",
            Property::Struct(StructValue::Struct(save_parameter)),
        );
        let mut dps_root_properties = Properties::default();
        dps_root_properties.insert(
            "SaveParameterArray",
            Property::Array(ValueVec::Struct(vec![StructValue::Struct(slot_props)])),
        );
        let dps_save = crate::ue::Save {
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
                properties: dps_root_properties,
            },
            extra: Vec::new(),
        };
        let level = crate::ue::Save {
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
                properties: Properties::default(),
            },
            extra: Vec::new(),
        };
        let mut session = SaveSession::new_for_tests(crate::session::SaveKind::InMemory, level);
        session.loaded_players.insert(
            owner_id,
            crate::session::LoadedPlayer {
                uid: owner_id,
                sav: {
                    let mut sav = crate::ue::Save {
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
                            properties: Properties::default(),
                        },
                        extra: Vec::new(),
                    };
                    let mut save_data = Properties::default();
                    sav.root.properties.insert(
                        "SaveData",
                        Property::Struct(StructValue::Struct(std::mem::take(&mut save_data))),
                    );
                    sav
                },
                dps: Some(dps_save),
            },
        );
        session
    }

    #[test]
    fn update_dps_pals_applies_the_dto_onto_the_matching_slot() {
        let data = game_data();
        let owner_id = uuid::Uuid::parse_str("11111111-0000-0000-0000-000000000000").unwrap();
        let instance_id = uuid::Uuid::parse_str("22222222-0000-0000-0000-000000000000").unwrap();
        let mut session = session_with_one_dps_slot(owner_id, "SheepBall", instance_id);

        let mut source = read_save_parameter_dto(
            &{
                let mut props = Properties::default();
                props.insert("CharacterID", props::name_property("SheepBall"));
                props
            },
            instance_id,
            true,
            &data,
        );
        source.owner_uid = Some(owner_id);
        source.nickname = Some("DPS Edited".to_string());
        source.level = 40;
        let mut modified: OrderedMap<i32, PalDto> = OrderedMap::new();
        modified.insert(0, source);

        let captured = std::sync::Arc::new(std::sync::Mutex::new(Vec::<String>::new()));
        let sink = captured.clone();
        let progress: crate::progress::ProgressSink =
            std::sync::Arc::new(move |message: &str| sink.lock().unwrap().push(message.into()));
        update_dps_pals(&mut session, &data, &modified, &progress).unwrap();

        let messages = captured.lock().unwrap();
        assert!(messages[0].starts_with("Updating DPS pal "));
        assert_eq!(
            messages.last().map(String::as_str),
            Some("Saving changes to file")
        );

        let dps_save = session.loaded_players[&owner_id].dps.as_ref().unwrap();
        let slots = props::struct_values(
            dps_save
                .root
                .properties
                .0
                .get(&PropertyKey::from("SaveParameterArray"))
                .unwrap(),
        )
        .unwrap();
        let updated = pal_dto_from_dps_slot(&slots[0], &data).expect("slot still readable");
        assert_eq!(updated.nickname.as_deref(), Some("DPS Edited"));
        assert_eq!(updated.level, 40);
        assert_eq!(updated.character_id, "SheepBall");
    }

    #[test]
    fn update_dps_pals_skips_an_unresolvable_owner_without_panicking() {
        let data = game_data();
        let owner_id = uuid::Uuid::parse_str("33333333-0000-0000-0000-000000000000").unwrap();
        let instance_id = uuid::Uuid::parse_str("44444444-0000-0000-0000-000000000000").unwrap();
        let mut session = session_with_one_dps_slot(owner_id, "SheepBall", instance_id);

        let mut source = read_save_parameter_dto(
            &{
                let mut props = Properties::default();
                props.insert("CharacterID", props::name_property("SheepBall"));
                props
            },
            instance_id,
            true,
            &data,
        );
        source.owner_uid = None; // never resolves to a loaded player
        let mut modified: OrderedMap<i32, PalDto> = OrderedMap::new();
        modified.insert(0, source);

        update_dps_pals(
            &mut session,
            &data,
            &modified,
            &crate::progress::null_progress(),
        )
        .unwrap();
    }

    #[test]
    fn known_pal_keys_loads_the_real_pals_json_key_set() {
        let data = game_data();
        let keys = known_pal_keys(&data);
        // pals.json spells it "Sheepball", not "SheepBall".
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
        only_hunger_and_sanity.insert("SanityValue", Property::Float(crate::ue::Float(50.0)));
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
            crate::ue::FGuid::nil(),
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
        assert!(pal_dto_from_dps_slot(&StructValue::Guid(crate::ue::FGuid::nil()), &data).is_none());
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
        assert_eq!(max_hp_for(&dto, false, &data), 12345);
    }

    #[test]
    fn read_save_parameter_dto_stomach_guards_against_nan_using_pal_data_fallback() {
        // "Anubis" has max_full_stomach 540 in pals.json -- deliberately not
        // Alpaca (150, which now collides with the missing-key flat default)
        // and not 300 (the unrecognized-pal fallback), so this assertion
        // still proves the pal_data lookup ran rather than either default.
        let data = game_data();
        let mut save_parameter = Properties::default();
        save_parameter.insert("CharacterID", Property::Name("Anubis".to_string()));
        save_parameter.insert("FullStomach", Property::Float(crate::ue::Float(f32::NAN)));
        let instance_id = uuid::Uuid::nil();

        let dto = read_save_parameter_dto(&save_parameter, instance_id, false, &data);

        assert_eq!(
            dto.stomach, 540.0,
            "NaN FullStomach on a recognized pal must fall back to pals.json's max_full_stomach"
        );

        // The wire-visible consequence: serde_json has no NaN literal and
        // would otherwise silently downgrade this field to JSON `null`.
        let serialized = serde_json::to_value(&dto).unwrap();
        assert_eq!(
            serialized["stomach"],
            serde_json::json!(540.0),
            "a NaN FullStomach must never reach the wire as null"
        );
    }

    #[test]
    fn read_save_parameter_dto_stomach_guards_against_infinity_using_flat_default_for_an_unrecognized_pal(
    ) {
        // The guard checks `is_finite()`, so Infinity is caught as well as NaN.
        let data = game_data();
        let mut save_parameter = Properties::default();
        save_parameter.insert(
            "CharacterID",
            Property::Name("TotallyMadeUpCreature".to_string()),
        );
        save_parameter.insert("FullStomach", Property::Float(crate::ue::Float(f32::INFINITY)));
        let instance_id = uuid::Uuid::nil();

        let dto = read_save_parameter_dto(&save_parameter, instance_id, false, &data);

        assert_eq!(
            dto.stomach, 300.0,
            "non-finite FullStomach on an unrecognized pal must fall back to the flat 300.0 default"
        );
    }

    #[test]
    fn read_save_parameter_dto_stomach_missing_key_still_defaults_to_150_not_the_pal_data_fallback()
    {
        // An absent FullStomach defaults to 150.0; only a present-but-invalid
        // one falls back to the species max. "Anubis" has max_full_stomach
        // 540 in pals.json, so getting 150.0 here (not 540.0) proves the
        // missing-key path uses the flat default and never consults
        // pal_data at all -- unlike "Alpaca" (150), which would no longer
        // discriminate between the two paths.
        let data = game_data();
        let mut save_parameter = Properties::default();
        save_parameter.insert("CharacterID", Property::Name("Anubis".to_string()));
        let instance_id = uuid::Uuid::nil();

        let dto = read_save_parameter_dto(&save_parameter, instance_id, false, &data);

        assert_eq!(dto.stomach, 150.0);
    }
}
