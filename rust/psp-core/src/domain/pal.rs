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

use uesave::{MapEntry, Properties, Property, PropertyKey, StructValue, ValueVec};

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

/// Port of `game/utils.py::get_pal_data`: resolves a pal's static
/// `pals.json` entry from an already-computed `character_key`
/// (case-insensitive against the real key casing, matching Python's
/// `PALS_KEY_MAP = {k.lower(): k for k in PAL_DATA.keys()}` lookup table).
/// `None` for an empty key (`if not character_key: return None`) or an
/// unrecognized one (`if not key: return None`). Shared by `max_hp_for`
/// (`Pal.max_hp`'s `self.pal_data`) and `read_save_parameter_dto`'s
/// `stomach` NaN/Infinity guard (`Pal.stomach`'s `_set_max_stomach`), both
/// of which port a Python property that reads `self.pal_data` the same way.
fn pal_data_for<'a>(character_key: &str, game_data: &'a GameData) -> Option<&'a serde_json::Value> {
    if character_key.is_empty() {
        return None;
    }
    game_data
        .get("pals")
        .and_then(|value| value.as_object())
        .and_then(|pals_json| {
            pals_json
                .iter()
                .find(|(key, _)| key.to_lowercase() == character_key)
                .map(|(_, value)| value)
        })
}

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
            // Deliberate divergence from Python: `WorkSuitability.from_value`
            // (game/enum.py) returns `None` (no fallback variant) for an
            // unrecognized bare name, and `Pal.work_suitability`'s
            // pydantic-validated return type is `Dict[WorkSuitability, int]`
            // -- assigning a `None` key there fails model validation, i.e.
            // an unrecognized WorkSuitability name would crash Python's read
            // path outright rather than silently drop the entry. This port
            // skips just that one entry instead (never panics on untrusted
            // save data), matching the "malformed input is skipped, not
            // fatal" rule documented at the top of this file.
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

    // `character_key` (Pal.character_key / Pal.pal_data): computed once up
    // front because `stomach`'s NaN/Infinity guard below needs it to resolve
    // `pal_data["max_full_stomach"]`, the same `self.pal_data` lookup
    // (`get_pal_data(self.character_key)`) Python's `_set_max_stomach` uses.
    let character_key = format_character_key(&character_id, &known_pal_keys(game_data));

    // `stomach` (Pal.stomach): 150.0 when FullStomach is absent. Python has
    // an explicit "artifact bug fix" (game/pal.py Pal.stomach) for corrupted
    // saves seen in the wild: `if not isinstance(stomach, float) or
    // math.isnan(stomach): return self._set_max_stomach()`. A present
    // FullStomach that decodes to a non-finite f32 (NaN observed in
    // practice; Infinity guarded for the same reason) must not leak onto the
    // wire -- `serde_json` has no NaN/Infinity literal and would silently
    // downgrade it to JSON `null` -- so it falls back through the same chain
    // `_set_max_stomach()` does: the pal's own `pals.json` `max_full_stomach`
    // if it has one, else a flat 300.0. A missing/wrong-typed FullStomach
    // property (as_f32 -> None) is a different Python branch ("FullStomach"
    // not in save_parameter) and keeps the existing 150.0 default, not this
    // fallback.
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
        stomach,
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
    // `is_boss`/`is_lucky` here are the same local variables computed above
    // (lines 104-108) that `dto.is_boss`/`dto.is_lucky` were just set from --
    // never stale on this read path, unlike `apply_pal_dto`'s caller-supplied
    // DTO (see `max_hp_for`'s doc comment).
    dto.max_hp = max_hp_for(&dto, is_boss || is_lucky, game_data);
    dto
}

/// Port of `Pal.max_hp` (`game/pal.py`): falls back to `dto.hp` when the pal
/// isn't recognized or has no `scaling.hp` entry in `pals.json` -- the same
/// fallback Python's `if not self.character_key or not self.pal_data:
/// return self.hp` / `if not hp_scaling: return self.hp` apply.
///
/// `boosted` is the caller-computed `self.is_boss or self.is_lucky`
/// (`game/pal.py` `Pal.max_hp`'s `alpha_scaling` condition). By boolean
/// absorption (`is_boss = character_id.upper().startswith("BOSS_") and not
/// is_lucky`), `is_boss or is_lucky` simplifies to `character_id.upper().
/// startswith("BOSS_") or is_lucky` -- see this module's `apply_pal_dto` doc
/// comment. This function deliberately does NOT read `dto.is_boss`/
/// `dto.is_lucky` itself: on the write path (`apply_pal_dto`), `dto.is_boss`
/// is caller-supplied DTO input that can be stale (echoed back by a client
/// after `character_id` changed -- the exact hazard `apply_pal_dto`'s own
/// boss-prefix fix addresses two lines away), and `dto.is_lucky` can be
/// `None` (meaning "leave `IsRarePal` untouched", not "false"), so neither
/// field reflects the save's actual current state at the point `Hp` is
/// computed. Making the caller pass the already-resolved `boosted` boolean,
/// rather than accepting an `is_boss`/`is_lucky` the caller could get wrong,
/// makes a stale value structurally impossible to feed into this function.
pub fn max_hp_for(dto: &PalDto, boosted: bool, game_data: &GameData) -> i64 {
    let keys = known_pal_keys(game_data);
    let pal_key = format_character_key(&dto.character_id, &keys);
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

        // `gender` (summaries.py get_pal_summaries): `None` unless "Gender"
        // is present AND its decoded value is truthy -- `gender = None;
        // if "Gender" in save_parameter: raw_gender = ...; if raw_gender:
        // gender = PalGender.from_value(raw_gender).value`. An empty
        // decoded string is falsy in Python and leaves gender `None`; unlike
        // the full `Pal.gender` dump (which always runs a present value
        // through `from_value`, defaulting even an empty string to Female),
        // summaries treat an empty string the same as an absent property.
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

// ============================================================================
// Write side (Task 6) — port of `Pal.update_from`/`Pal.heal`/
// `PalObjects.PalSaveParameter` (`game/pal.py`, `game/pal_objects.py`).
// ============================================================================

/// `PAL_SICK_TYPES` (`game/pal.py`) verbatim -- all five markers `Pal.heal`
/// removes. Distinct from `SICK_MARKERS` above (three of these five, which
/// is what `Pal.is_sick` actually checks membership of).
const PAL_SICK_TYPES: [&str; 5] = [
    "PalReviveTimer",
    "PhysicalHealth",
    "WorkerSick",
    "HungerType",
    "SanityValue",
];

/// Sets `name` to `property` when `Some`, removes it entirely when `None` --
/// the "remove on default" shape every clamped/optional `Pal` setter in
/// `game/pal.py` shares (`safe_remove(self._save_parameter, ...)` vs. a plain
/// assignment). Name-only removal via `PropertyKey::from(name)` (index `0`)
/// matches this module's own `param` lookup convention; every property this
/// port ever inserts is itself created with `PropertyKey::from` (also index
/// `0`), and every real save property this port has read so far resolves the
/// same way (`param`'s existing, already save-verified behavior).
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

/// Port of `Pal._set_max_stomach`'s lookup (`game/pal.py`): a recognized
/// pal's `pals.json` `max_full_stomach`, else the flat `300.0` default.
/// Shares `pal_data_for` with `max_hp_for`/`read_save_parameter_dto`'s own
/// NaN/Infinity guard rather than re-deriving the `pals.json` lookup a third
/// time (the brief's version of this function duplicated that lookup
/// inline; using the existing private helper is the same behavior with one
/// fewer copy of it in this module).
pub fn max_stomach_for(character_id: &str, game_data: &GameData) -> f64 {
    let keys = known_pal_keys(game_data);
    let pal_key = format_character_key(character_id, &keys);
    pal_data_for(&pal_key, game_data)
        .and_then(|pal_data| pal_data.pointer("/max_full_stomach"))
        .and_then(|value| value.as_f64())
        .unwrap_or(300.0)
}

/// Port of `Pal.heal` (`game/pal.py`): removes every `PAL_SICK_TYPES`
/// marker, then sets `SanityValue = 100.0` and `FullStomach` to the pal's
/// max (`_set_max_stomach`).
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

/// Port of `Pal.update_from` (`game/pal.py`). Applies every writable field
/// from `dto` onto an existing pal/player `SaveParameter` bag, following
/// Python's exact remove-on-default / skip-on-`None` semantics per field
/// (each block below cites the Python setter it ports). Two deliberate
/// narrowings vs. the full Python method, both load-bearing:
///
/// - **`group_id` is not applied here.** Python's `group_id.setter` writes
///   into `PalCharacterData.group_id` -- a sibling of `SaveParameter`, not a
///   property inside it -- which this function's `&mut Properties` signature
///   cannot reach. The caller (Task 9, which owns the full `MapEntry`) must
///   apply `dto.group_id` to `PalCharacterData.group_id` directly.
/// - **`dto.is_boss` is never read.** Python's own `update_from` puts
///   `"is_boss"` in `skip_properties` (it has no setter -- `is_boss` is a
///   read-only `@computed_field`), and the boss-prefix decision at the end
///   of the method uses `self.is_boss or self.is_lucky`, where `self.is_boss`
///   is RE-DERIVED from the just-updated `character_id`/`is_lucky`
///   (`character_id.upper().startswith("BOSS_") and not is_lucky`), not read
///   from the incoming DTO at all. Algebraically `(A and not B) or B`
///   simplifies to `A or B`, so the actual boss-decision Python computes is
///   `character_id.upper().startswith("BOSS_") || is_lucky` -- never
///   `dto.is_boss`. A version of this function that reads `dto.is_boss`
///   directly here is wrong: a stale `is_boss=true` echoed back by a client
///   for an already-non-boss `character_id` would incorrectly re-add the
///   `BOSS_` prefix. Fixed per this task's "Python source wins over the
///   brief" rule -- see this task's report.
///
/// Also diverges from Python in one more place, deliberately: `Rank_HP`/
/// `Rank_Attack`/`Rank_Defence`/`Rank_CraftSpeed`/`Level`/`Talent_HP`/
/// `Talent_Shot`/`Talent_Defense` are saturated to `0..=255` before being
/// written as a `Byte`. Python's setters for these do NOT clamp (only
/// `rank`'s setter does, via `min(value, 255)`) -- an out-of-range value
/// would raise an unhandled `struct.error` in Python at actual
/// byte-serialization time. This port saturates instead, matching the
/// project's "never panic on malformed/adversarial input" policy (this is
/// about untrusted DTO input from the API, not save-file bytes, but the same
/// policy applies). `FriendshipPoint` and `storage_slot`'s `SlotIndex` are
/// saturated to `i32::MIN..=i32::MAX` for the identical reason -- both are
/// plain UE `IntProperty`s built from a `PalDto` `i64` field, so a bare
/// `as i32` would silently wrap instead of matching Python's would-be crash.
/// `exp`'s `Int64Property` needs no such clamp: `PalDto::exp` is already
/// `i64`, the exact width `Int64Property` stores, so no narrowing cast (and
/// therefore no overflow) is possible there at all.
pub fn apply_pal_dto(
    save_parameter: &mut Properties,
    dto: &crate::dto::pal::PalDto,
    is_dps: bool,
    game_data: &GameData,
) {
    // OwnerPlayerUId (Pal.owner_uid setter): skipped entirely when None,
    // matching `update_from`'s `if value is None: continue`.
    if let Some(owner_uid) = dto.owner_uid {
        save_parameter.insert("OwnerPlayerUId", props::guid_property(owner_uid));
    }
    // CharacterID (Pal.character_id setter): character_id is required on
    // PalDto, always applied.
    save_parameter.insert("CharacterID", props::name_property(&dto.character_id));

    // IsRarePal (Pal.is_lucky setter): skipped entirely when None (matching
    // `update_from`'s None-skip) -- "absent" is NOT the same as "false".
    if let Some(is_lucky) = dto.is_lucky {
        if is_lucky {
            save_parameter.insert("IsRarePal", props::bool_property(true));
        } else {
            save_parameter
                .0
                .shift_remove(&PropertyKey::from("IsRarePal"));
        }
    }

    // Gender (Pal.gender setter): required field, always applied.
    save_parameter.insert("Gender", props::enum_property(&dto.gender.prefixed()));

    // Rank_HP/Rank_Attack/Rank_Defence/Rank_CraftSpeed (Pal.rank_hp/
    // rank_attack/rank_defense/rank_craftspeed setters): remove on 0.
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

    // Talent_HP/Talent_Shot/Talent_Defense (Pal.talent_* setters): always
    // applied unconditionally, no removal branch.
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

    // Rank (Pal.rank setter): `value = min(value, 255)` then remove-on-0.
    set_or_remove(
        save_parameter,
        "Rank",
        (dto.rank != 0).then(|| props::byte_property(dto.rank.clamp(0, 255) as u8)),
    );
    // Level (Pal.level setter): remove when <= 1.
    set_or_remove(
        save_parameter,
        "Level",
        (dto.level > 1).then(|| props::byte_property(dto.level.clamp(0, 255) as u8)),
    );
    // Exp (Pal.exp setter): remove on 0.
    set_or_remove(
        save_parameter,
        "Exp",
        (dto.exp != 0).then(|| props::int64_property(dto.exp)),
    );

    // NickName (Pal.nickname setter): skipped entirely when None.
    if let Some(nickname) = &dto.nickname {
        save_parameter.insert("NickName", props::str_property(nickname));
    }
    // FilteredNickName (Pal.filtered_nickname setter): only ever written for
    // DPS pals, and only when present -- matches Python's setter, which
    // no-ops internally for non-DPS pals regardless of whether the loop
    // even reaches it.
    if is_dps {
        if let Some(filtered) = &dto.filtered_nickname {
            save_parameter.insert("FilteredNickName", props::str_property(filtered));
        }
    }

    // FullStomach (Pal.stomach setter): always applied here; heal() below
    // (non-DPS only) unconditionally overwrites it again with the pal's max,
    // matching Python's own redundant write-then-overwrite.
    save_parameter.insert("FullStomach", props::float_property(dto.stomach as f32));

    // storage_slot (Pal.storage_slot/storage_id setters): PARITY-BUG-1.
    // Python's storage_id setter and storage_slot setter are byte-for-byte
    // identical (`self._save_parameter[slot_id_key] =
    // PalObjects.PalCharacterSlotId(self.storage_id, value)`) and are applied
    // in PalDTO's field declaration order -- storage_id before storage_slot.
    // storage_id's setter therefore rebuilds the slot struct from *its own
    // getter's* (i.e. the OLD, unchanged) container id plus `value` (a UUID,
    // mis-typed into the int-shaped SlotIndex slot) -- a transient, invalid
    // intermediate state, immediately overwritten by storage_slot's setter,
    // which runs next and rebuilds the SAME struct from the SAME old
    // container id plus the real (int) new slot index. The only ever
    // *observable* effect of applying both fields is: ContainerId never
    // changes, only SlotIndex does. This port reproduces exactly that net
    // effect directly (mutate SlotIndex in place on the existing, already
    // schema-registered struct; leave ContainerId untouched) rather than
    // replaying Python's transient invalid intermediate write, which is
    // never itself serialized to disk.
    //
    // Real save data spells this property "SlotId" (mixed case) on every
    // pal this port has read (11/11 in tests/fixtures/saves/world1); Python's
    // own `PalObjects.PalCharacterSlotId` constructor always writes "SlotID"
    // (all-caps). Both this function and the read side (`read_save_parameter_dto`)
    // check "SlotID" first, falling back to "SlotId", matching
    // `Pal.storage_slot`'s own key-preference exactly. Neither key present
    // (never observed in real save data) is a silent no-op here, rather than
    // Python's "construct a new struct whose ContainerId is a None-valued
    // Guid" -- a pathological path this port declines to replicate, since it
    // would require modeling an invalid null-valued Guid property, and
    // cannot arise from calling `apply_pal_dto` on any entry this port's own
    // read side ever produces.
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
        // Saturate rather than wrap: `SlotIndex` is a plain UE `IntProperty`
        // (i32); Python's `PalObjects.PalCharacterSlotId` would raise an
        // unhandled `struct.error` on an out-of-i32-range value rather than
        // silently wrapping it, so a bare `as i32` here would produce a
        // *different* wrong value than Python's crash -- matching the same
        // "saturate untrusted DTO input rather than wrap" policy already
        // applied to `Rank`/`Level`/`Talent_*`/`Rank_*` above.
        slot_struct.insert(
            "SlotIndex",
            props::int_property(dto.storage_slot.clamp(i32::MIN as i64, i32::MAX as i64) as i32),
        );
    }

    // MasteredWaza (Pal.learned_skills setter): remove when empty.
    set_or_remove(
        save_parameter,
        "MasteredWaza",
        (!dto.learned_skills.is_empty())
            .then(|| props::enum_array_property(dto.learned_skills.clone())),
    );
    // EquipWaza/PassiveSkillList (Pal.active_skills/passive_skills setters):
    // always applied unconditionally, no removal branch.
    save_parameter.insert(
        "EquipWaza",
        props::enum_array_property(dto.active_skills.clone()),
    );
    save_parameter.insert(
        "PassiveSkillList",
        props::name_array_property(dto.passive_skills.clone()),
    );

    // SanityValue (Pal.sanity setter): always applied here; heal() below
    // (non-DPS only) unconditionally overwrites it to 100.0 afterward,
    // matching Python's own write-then-overwrite.
    save_parameter.insert("SanityValue", props::float_property(dto.sanity as f32));

    // GotWorkSuitabilityAddRankList (Pal.work_suitability setter): drop
    // zero-rank entries; remove the whole property when nothing remains.
    // Also drops any key that isn't one of the 13 known WorkSuitability
    // names: Python's wire layer (pydantic's `Dict[WorkSuitability, int]`
    // validation on `PalDTO`) rejects an unrecognized key before it ever
    // reaches `update_from`; `PalDto.work_suitability` has no such upstream
    // guarantee (it's a plain `OrderedMap<String, i64>`), so this port
    // applies the same defensive filter Task 5's read side already applies
    // to untrusted *save* data (`read_save_parameter_dto`'s work_suitability
    // loop) here, to untrusted *DTO input* instead -- never write an
    // unrecognized `EPalWorkSuitability::` variant string into the save.
    let non_zero_known: Vec<(&String, &i64)> = dto
        .work_suitability
        .iter()
        .filter(|(name, rank)| **rank != 0 && WORK_SUITABILITIES.contains(&name.as_str()))
        .collect();
    if non_zero_known.is_empty() {
        save_parameter
            .0
            .shift_remove(&PropertyKey::from("GotWorkSuitabilityAddRankList"));
    } else {
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

    // FriendshipPoint (Pal.friendship_point setter): always applied. Saturate
    // rather than wrap -- same rationale as `storage_slot` above: a plain UE
    // `IntProperty` (i32), and Python's `PalObjects.IntProperty` would raise
    // on an out-of-range value rather than wrap.
    save_parameter.insert(
        "FriendshipPoint",
        props::int_property(dto.friendship_point.clamp(i32::MIN as i64, i32::MAX as i64) as i32),
    );

    // Tail of update_from (game/pal.py): `self.hp = self.max_hp` -- recomputed
    // from the state just written above, so dto.hp's own value (itself
    // written and then immediately superseded here, matching Python's
    // redundant write-then-overwrite via the "hp" key in the generic
    // setattr loop) never survives. Then heal() for non-DPS pals, then
    // boss-prefix formatting.
    //
    // `self.max_hp`'s `alpha_scaling` reads `self.is_boss or self.is_lucky`
    // (game/pal.py), and BOTH are live computed properties re-read from the
    // save's ACTUAL current state at this exact point in `update_from`, not
    // from the incoming DTO: `self.is_lucky` reads directly off whatever
    // "IsRarePal" now holds in `_save_parameter` -- set moments ago if
    // `dto.is_lucky` was `Some`, or left exactly as it already was if `None`
    // (the is_lucky-None-skip fix above) -- and `self.is_boss` reads
    // `self.character_id`, just set to `dto.character_id`. Reading
    // `dto.is_boss`/`dto.is_lucky` directly here instead (as the brief's
    // reference code did) is wrong on two counts: `dto.is_boss` is
    // caller-supplied and can be stale (the same hazard the boss-prefix fix
    // below addresses for `CharacterID`, missed here for `Hp`), and
    // `dto.is_lucky` can be `None` -- which means "leave `IsRarePal`
    // untouched", not "false" -- so `dto.is_lucky.unwrap_or(false)` would
    // silently disagree with a pal that is actually still lucky from before
    // this call. Reading `IsRarePal` back off `save_parameter` (post-write)
    // is the only way to match Python's live-getter re-read exactly.
    let current_is_lucky = param(save_parameter, "IsRarePal")
        .and_then(props::as_bool)
        .unwrap_or(false);
    let boosted = dto.character_id.to_uppercase().starts_with("BOSS_") || current_is_lucky;
    let max_hp = max_hp_for(dto, boosted, game_data);
    save_parameter.insert("Hp", props::fixed_point64_property(max_hp));
    // Legacy spelling cleanup: proactively removes a stale "HP" key whenever
    // "Hp" is rewritten. Python only ever migrates "HP" -> "Hp" as a side
    // effect of *reading* `Pal.hp`'s getter, which `update_from`'s own
    // `self.hp = self.max_hp` (a setter call) does not trigger for a
    // recognized pal (see this task's report) -- so Python does not reliably
    // clean up a stale "HP" key here. This is a deliberate, strictly safer
    // divergence (never destroys data: "HP" is always redundant with the
    // "Hp" just written above) with zero real-save impact observed (0/11
    // pals in tests/fixtures/saves/world1 carry the legacy "HP" spelling at
    // all).
    save_parameter.0.shift_remove(&PropertyKey::from("HP"));
    if !is_dps {
        heal_save_parameter(save_parameter, &dto.character_id, game_data);
    }

    // _format_boss_character_id (game/pal.py) -- see this function's own doc
    // comment for why `should_be_boss` is derived from `character_id`/
    // `is_lucky`, never `dto.is_boss`.
    let current_id = dto.character_id.clone();
    let should_be_boss =
        current_id.to_uppercase().starts_with("BOSS_") || dto.is_lucky.unwrap_or(false);
    let has_prefix = current_id.starts_with("BOSS_"); // case-sensitive, matching Python's `_format_boss_character_id`
    if should_be_boss && !has_prefix {
        save_parameter.insert(
            "CharacterID",
            props::name_property(&format!("BOSS_{current_id}")),
        );
    } else if !should_be_boss && has_prefix {
        save_parameter.insert("CharacterID", props::name_property(&current_id[5..]));
    }
}

/// `PalObjects.StatusNames` (`pal_objects.py`) -- Japanese status keys; six
/// entries, includes capture rate.
pub const STATUS_NAMES: [&str; 6] = [
    "最大HP",
    "最大SP",
    "攻撃力",
    "所持重量",
    "捕獲率",
    "作業速度",
];
/// `PalObjects.ExStatusNames` (`pal_objects.py`) -- five entries, no capture
/// rate.
pub const EX_STATUS_NAMES: [&str; 5] = ["最大HP", "最大SP", "攻撃力", "所持重量", "作業速度"];

/// `PalObjects.GetStatusPointList` (`pal_objects.py`): one `{StatusName,
/// StatusPoint: 0}` struct per status name.
fn status_point_structs(names: &[&str]) -> Property {
    let mut values = Vec::new();
    for status_name in names {
        let mut status_props = Properties::default();
        status_props.insert("StatusName", props::name_property(status_name));
        status_props.insert("StatusPoint", props::int_property(0));
        values.push(StructValue::Struct(status_props));
    }
    Property::Array(ValueVec::Struct(values))
}

/// `PalObjects.TIME` (`pal_objects.py`): a fixed UE tick count (not "now"),
/// used verbatim by `PalObjects.PalSaveParameter` for a freshly created pal's
/// `OwnedTime`. `uesave`'s `StructValue::DateTime` is a bare tick-count
/// `u64` alias, so a wrong value here (this port previously wrote a literal
/// `0`, which decodes to `0001-01-01`) compiles silently but writes a wrong
/// "owned since" timestamp into the save for every freshly created pal.
const PAL_OWNED_TIME_TICKS: u64 = 638_486_453_957_560_000;

/// `PalObjects.PalSaveParameter`'s literal `CustomVersionData` byte payload
/// (`pal_objects.py`) -- opaque UE custom-version-guid metadata, unrelated to
/// any game-specific `RawData` codec. Every real pal entry this port has
/// read carries a `CustomVersionData` sibling of `RawData` at the character-
/// map entry's value level (verified against `tests/fixtures/saves/world1`);
/// the brief's own reference implementation of `new_pal_entry` omitted it
/// despite its own checkpoint note flagging the need to carry it -- added
/// here as a fixed literal (matching Python's own hardcoded list exactly)
/// since this function's signature (fixed per this task's brief, "use
/// verbatim") has no template/existing-entry parameter to clone it from.
const CUSTOM_VERSION_DATA: [u8; 24] = [
    1, 0, 0, 0, 108, 246, 252, 15, 153, 72, 144, 17, 248, 156, 96, 177, 94, 71, 70, 74, 1, 0, 0, 0,
];

/// Port of `PalObjects.PalSaveParameter` (`pal_objects.py`) -- returns a
/// complete new `CharacterSaveParameterMap` entry for a freshly created pal.
/// `nickname` here is always used as given (Python's own
/// `nickname = nickname or character_id` default-to-species-name fallback is
/// the caller's job -- `new_pal_entry` takes `nickname: &str`, not
/// `Option<&str>`, so there is no "unset" state to default here).
///
/// Does NOT insert the returned entry into any map, and does NOT register
/// the write schemas a freshly serialized copy of it would need beyond what
/// `ensure_pal_property_schemas` covers (see that function's own doc
/// comment) -- both are Task 9's responsibility (the actual pal-creation
/// CRUD operation, which owns the `SaveSession`/`uesave::Save` this entry
/// gets inserted into).
pub fn new_pal_entry(
    character_id: &str,
    instance_id: uuid::Uuid,
    owner_uid: uuid::Uuid,
    container_id: uuid::Uuid,
    slot_index: i32,
    group_id: Option<uuid::Uuid>,
    nickname: &str,
) -> MapEntry {
    let mut save_parameter = Properties::default();
    save_parameter.insert("CharacterID", props::name_property(character_id));
    save_parameter.insert("Gender", props::enum_property("EPalGenderType::Female"));
    save_parameter.insert("Level", props::byte_property(1));
    save_parameter.insert("Exp", props::int64_property(0));
    save_parameter.insert("NickName", props::str_property(nickname));
    save_parameter.insert("EquipWaza", props::enum_array_property(vec![]));
    save_parameter.insert("MasteredWaza", props::enum_array_property(vec![]));
    // "Hp" (not Python's literal "HP"): every real save this port has read
    // uses "Hp" (0/11 world1 pals carry the legacy "HP" spelling), and this
    // port's own read side prioritizes "Hp" too. Python's `PalSaveParameter`
    // constructor literally writes "HP", a legacy-spelling quirk that
    // self-heals the moment any code reads `Pal.hp`'s getter (which migrates
    // "HP" -> "Hp" as a side effect) -- in practice, on essentially every
    // real code path a newly created pal's DTO gets serialized back to the
    // client at least once before the save is written, which triggers that
    // migration. This is a real, if extremely narrow, Python quirk not on
    // the PARITY-BUG list; reported rather than silently reproduced, since
    // reproducing it would mean this port's own freshly created pals are the
    // ONLY entries in the entire save tree spelled "HP" -- inconsistent with
    // every other pal in the file, for no behavioral benefit (both spellings
    // read back identically through this port's own `Hp`-then-`HP` fallback).
    save_parameter.insert("Hp", props::fixed_point64_property(545_000));
    save_parameter.insert("Talent_HP", props::byte_property(50));
    save_parameter.insert("Talent_Shot", props::byte_property(50));
    save_parameter.insert("Talent_Defense", props::byte_property(50));
    save_parameter.insert("FullStomach", props::float_property(300.0));
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
    // "SlotID" (all-caps), matching Python's `PalObjects.PalCharacterSlotId`
    // constructor exactly -- unlike "Hp"/"HP" above, Python's getters
    // (`storage_slot`/`storage_id`) check "SlotID" *first*, so this spelling
    // is read back correctly with no migration needed; this port's own read
    // side does the same (see `read_save_parameter_dto`).
    save_parameter.insert("SlotID", Property::Struct(StructValue::Struct(slot_struct)));

    save_parameter.insert("GotStatusPointList", status_point_structs(&STATUS_NAMES));
    save_parameter.insert(
        "GotExStatusPointList",
        status_point_structs(&EX_STATUS_NAMES),
    );
    save_parameter.insert(
        "LastJumpedLocation",
        Property::Struct(StructValue::Vector(uesave::Vector {
            x: uesave::Double(0.0),
            y: uesave::Double(0.0),
            z: uesave::Double(7088.5),
        })),
    );

    let mut object_props = Properties::default();
    object_props.insert(
        "SaveParameter",
        Property::Struct(StructValue::Struct(save_parameter)),
    );

    let character_data = uesave::games::palworld::PalCharacterData {
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
        Property::Struct(StructValue::PalCharacterData(character_data)),
    );
    value_props.insert(
        "CustomVersionData",
        Property::Array(ValueVec::Byte(uesave::ByteArray::Byte(
            CUSTOM_VERSION_DATA.to_vec(),
        ))),
    );

    MapEntry {
        key: Property::Struct(StructValue::Struct(key_props)),
        value: Property::Struct(StructValue::Struct(value_props)),
    }
}

/// Registers write schemas for every property `apply_pal_dto`/
/// `heal_save_parameter` can newly introduce on a pal that has never carried
/// it before -- `uesave` refuses to write a property at a path with no
/// recorded schema (see `props::ensure_schema`'s own doc comment). Every
/// scalar tag shape and the two array tag shapes below were verified
/// directly against schemas `uesave` itself recorded while parsing
/// `tests/fixtures/saves/world1`/`world2` (dumped via a temporary debug test,
/// since deleted -- see this task's report for the exact shapes observed).
///
/// Extends the brief's 6-entry list with `Rank_HP`/`Rank_Attack`/
/// `Rank_Defence`/`Rank_CraftSpeed` (conditionally written by
/// `apply_pal_dto`, exactly like `Rank`, but omitted from the brief's list),
/// `MasteredWaza` (present on ZERO of the 11 world1 pals and ZERO schemas
/// anywhere in either fixture save -- confirmed empirically, not assumed;
/// its `Array(Enum)` tag shape is `EquipWaza`'s, which is structurally
/// identical: both are `PalObjects.ArrayPropertyValues(ArrayType.
/// ENUM_PROPERTY, ...)` in Python), and `GotWorkSuitabilityAddRankList`
/// (present on all 11 world1 pals but not guaranteed universal, so still
/// registered defensively; its `Array(Struct("PalWorkSuitabilityInfo"))`
/// shape, plus its two nested per-field schemas, were read directly off a
/// real pal rather than guessed).
///
/// The brief's own `PropertyTagDataPartial::Bool(false)` for `IsRarePal`
/// does not compile: `PropertyTagDataPartial` (unlike its `_Full` sibling
/// used only during actual parsing) has no `Bool` variant at all --
/// `uesave`'s own `PropertyTagDataFull::into_partial` collapses `Bool(_)`
/// into `Other(PropertyType::BoolProperty)` (verified in `uesave/src/
/// lib.rs`, and the reverse `into_full` maps `Other(BoolProperty)` back to a
/// real `Bool(value)` at write time), so that's the correct shape here.
///
/// Does NOT cover every property `new_pal_entry` introduces (`OwnedTime`,
/// `SlotID`, `GotStatusPointList`, `LastJumpedLocation`, ...) -- those only
/// need a registered schema once a newly built entry is actually inserted
/// into the world tree and serialized, which is Task 9's concern; this task
/// never inserts a `new_pal_entry` result into any map.
pub fn ensure_pal_property_schemas(level: &mut uesave::Save) {
    use uesave::{PropertyTagDataPartial, PropertyTagPartial, PropertyType, StructType};

    let Some(prefix) = props::schema_prefix_ending_with(level, "SaveParameter.CharacterID") else {
        return;
    };
    let tag = |data: PropertyTagDataPartial| PropertyTagPartial { id: None, data };
    let path = |name: &str| format!("{prefix}SaveParameter.{name}");

    let scalar_entries: [(&str, PropertyTagDataPartial); 10] = [
        (
            "IsRarePal",
            PropertyTagDataPartial::Other(PropertyType::BoolProperty),
        ),
        (
            "FriendshipPoint",
            PropertyTagDataPartial::Other(PropertyType::IntProperty),
        ),
        (
            "Exp",
            PropertyTagDataPartial::Other(PropertyType::Int64Property),
        ),
        ("Rank", PropertyTagDataPartial::Byte(None)),
        ("Level", PropertyTagDataPartial::Byte(None)),
        (
            "SanityValue",
            PropertyTagDataPartial::Other(PropertyType::FloatProperty),
        ),
        ("Rank_HP", PropertyTagDataPartial::Byte(None)),
        ("Rank_Attack", PropertyTagDataPartial::Byte(None)),
        ("Rank_Defence", PropertyTagDataPartial::Byte(None)),
        ("Rank_CraftSpeed", PropertyTagDataPartial::Byte(None)),
    ];
    for (name, data) in scalar_entries {
        props::ensure_schema(level, path(name), tag(data));
    }

    props::ensure_schema(
        level,
        path("MasteredWaza"),
        tag(PropertyTagDataPartial::Array(Box::new(
            PropertyTagDataPartial::Enum(String::new(), None),
        ))),
    );

    props::ensure_schema(
        level,
        path("GotWorkSuitabilityAddRankList"),
        tag(PropertyTagDataPartial::Array(Box::new(
            PropertyTagDataPartial::Struct {
                struct_type: StructType::Struct(Some("PalWorkSuitabilityInfo".to_string())),
                id: uesave::FGuid::nil(),
            },
        ))),
    );
    props::ensure_schema(
        level,
        path("GotWorkSuitabilityAddRankList.WorkSuitability"),
        tag(PropertyTagDataPartial::Enum(
            "EPalWorkSuitability".to_string(),
            None,
        )),
    );
    props::ensure_schema(
        level,
        path("GotWorkSuitabilityAddRankList.Rank"),
        tag(PropertyTagDataPartial::Other(PropertyType::IntProperty)),
    );
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
        assert_eq!(max_hp_for(&dto, false, &data), 12345);
    }

    #[test]
    fn read_save_parameter_dto_stomach_guards_against_nan_using_pal_data_fallback() {
        // Python's "artifact bug fix" (game/pal.py Pal.stomach): a present
        // but NaN FullStomach falls back through `_set_max_stomach()` --
        // `pal_data["max_full_stomach"]` when the pal is recognized, else
        // 300.0. "Alpaca" has `max_full_stomach: 225` in the real
        // data/json/pals.json (verified via `.venv` Python).
        let data = game_data();
        let mut save_parameter = Properties::default();
        save_parameter.insert("CharacterID", Property::Name("Alpaca".to_string()));
        save_parameter.insert("FullStomach", Property::Float(uesave::Float(f32::NAN)));
        let instance_id = uuid::Uuid::nil();

        let dto = read_save_parameter_dto(&save_parameter, instance_id, false, &data);

        assert_eq!(
            dto.stomach, 225.0,
            "NaN FullStomach on a recognized pal must fall back to pals.json's max_full_stomach"
        );

        // The wire-visible consequence: serde_json has no NaN literal and
        // would otherwise silently downgrade this field to JSON `null`.
        let serialized = serde_json::to_value(&dto).unwrap();
        assert_eq!(
            serialized["stomach"],
            serde_json::json!(225.0),
            "a NaN FullStomach must never reach the wire as null"
        );
    }

    #[test]
    fn read_save_parameter_dto_stomach_guards_against_infinity_using_flat_default_for_an_unrecognized_pal(
    ) {
        // Same guard, but for an unrecognized character_key (no pals.json
        // entry at all) and Infinity rather than NaN -- both are non-finite,
        // and Python's `math.isnan` alone would miss Infinity, so the Rust
        // guard checks `is_finite()` instead of a NaN-only check.
        let data = game_data();
        let mut save_parameter = Properties::default();
        save_parameter.insert(
            "CharacterID",
            Property::Name("TotallyMadeUpCreature".to_string()),
        );
        save_parameter.insert("FullStomach", Property::Float(uesave::Float(f32::INFINITY)));
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
        // A missing FullStomach key is a *different* Python branch ("FullStomach"
        // not in save_parameter -> 150.0 directly) from a present-but-invalid
        // value (-> _set_max_stomach()). Recognized pal ("Alpaca", whose
        // max_full_stomach is 225, not 150) proves the two branches aren't
        // conflated.
        let data = game_data();
        let mut save_parameter = Properties::default();
        save_parameter.insert("CharacterID", Property::Name("Alpaca".to_string()));
        let instance_id = uuid::Uuid::nil();

        let dto = read_save_parameter_dto(&save_parameter, instance_id, false, &data);

        assert_eq!(dto.stomach, 150.0);
    }
}
