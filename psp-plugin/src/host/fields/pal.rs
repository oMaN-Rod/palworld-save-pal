use std::ffi::c_int;
use std::sync::OnceLock;

use psp_core::domain::pal;
use psp_core::dto::ordered_map::OrderedMap;
use psp_core::dto::pal::{PalDto, PalGender, WORK_SUITABILITIES};
use psp_core::dto::summary::PalSummary;
use psp_core::gamedata::GameData;
use psp_lua_sys::ffi::*;
use uuid::Uuid;

use super::{
    expect_bool, expect_int, expect_list, expect_str, field_value_type_name, ranged_int, read_field_value,
    Access, FieldSpec, FieldValue, FieldWrite, Reader,
};
use crate::context::RunContext;
use crate::host::api_def::{ApiField, ApiType};
use crate::host::handle::{read_handle, HandleKind};
use crate::host::marshal::{arg_string, check_args};
use crate::host::save_read::ensure_pals_snapshot;
use crate::host::{dto_cache, with_context, HostError};
use crate::host_fn;
use crate::manifest::Capability;

fn gender_str(gender: PalGender) -> &'static str {
    match gender {
        PalGender::None => "None",
        PalGender::Male => "Male",
        PalGender::Female => "Female",
    }
}

// --- readers -----------------------------------------------------------

fn read_instance_id(dto: &PalDto) -> FieldValue {
    FieldValue::Str(dto.instance_id.to_string())
}
fn read_character_id(dto: &PalDto) -> FieldValue {
    FieldValue::Str(dto.character_id.clone())
}
fn read_character_key(dto: &PalDto) -> FieldValue {
    FieldValue::Str(dto.character_key.clone())
}
fn read_owner_uid(dto: &PalDto) -> FieldValue {
    dto.owner_uid.map(|u| FieldValue::Str(u.to_string())).unwrap_or(FieldValue::Nil)
}
fn read_nickname(dto: &PalDto) -> FieldValue {
    dto.nickname.clone().map(FieldValue::Str).unwrap_or(FieldValue::Nil)
}
fn read_gender(dto: &PalDto) -> FieldValue {
    FieldValue::Str(gender_str(dto.gender).to_string())
}
fn read_group_id(dto: &PalDto) -> FieldValue {
    dto.group_id.map(|u| FieldValue::Str(u.to_string())).unwrap_or(FieldValue::Nil)
}
fn read_guild_id_from_summary(summary: &PalSummary) -> FieldValue {
    summary.guild_id.map(|u| FieldValue::Str(u.to_string())).unwrap_or(FieldValue::Nil)
}
fn read_base_id_from_summary(summary: &PalSummary) -> FieldValue {
    summary.base_id.map(|u| FieldValue::Str(u.to_string())).unwrap_or(FieldValue::Nil)
}
fn read_stomach(dto: &PalDto) -> FieldValue {
    FieldValue::Float(dto.stomach)
}
fn read_sanity(dto: &PalDto) -> FieldValue {
    FieldValue::Float(dto.sanity)
}
fn read_hp(dto: &PalDto) -> FieldValue {
    FieldValue::Int(dto.hp)
}
fn read_level(dto: &PalDto) -> FieldValue {
    FieldValue::Int(dto.level)
}
fn read_exp(dto: &PalDto) -> FieldValue {
    FieldValue::Int(dto.exp)
}
fn read_rank(dto: &PalDto) -> FieldValue {
    FieldValue::Int(dto.rank)
}
fn read_rank_hp(dto: &PalDto) -> FieldValue {
    FieldValue::Int(dto.rank_hp)
}
fn read_rank_attack(dto: &PalDto) -> FieldValue {
    FieldValue::Int(dto.rank_attack)
}
fn read_rank_defense(dto: &PalDto) -> FieldValue {
    FieldValue::Int(dto.rank_defense)
}
fn read_rank_craftspeed(dto: &PalDto) -> FieldValue {
    FieldValue::Int(dto.rank_craftspeed)
}
fn read_talent_hp(dto: &PalDto) -> FieldValue {
    FieldValue::Int(dto.talent_hp)
}
fn read_talent_shot(dto: &PalDto) -> FieldValue {
    FieldValue::Int(dto.talent_shot)
}
fn read_talent_defense(dto: &PalDto) -> FieldValue {
    FieldValue::Int(dto.talent_defense)
}
fn read_max_hp(dto: &PalDto) -> FieldValue {
    FieldValue::Int(dto.max_hp)
}
fn read_storage_slot(dto: &PalDto) -> FieldValue {
    FieldValue::Int(dto.storage_slot)
}
fn read_storage_id(dto: &PalDto) -> FieldValue {
    FieldValue::Str(dto.storage_id.to_string())
}
fn read_is_boss(dto: &PalDto) -> FieldValue {
    FieldValue::Bool(dto.is_boss.unwrap_or(false))
}
fn read_is_lucky(dto: &PalDto) -> FieldValue {
    FieldValue::Bool(dto.is_lucky.unwrap_or(false))
}
fn read_is_awakened(dto: &PalDto) -> FieldValue {
    FieldValue::Bool(dto.is_awakened.unwrap_or(false))
}
fn read_is_imported(dto: &PalDto) -> FieldValue {
    FieldValue::Bool(dto.is_imported.unwrap_or(false))
}
fn read_is_predator(dto: &PalDto) -> FieldValue {
    FieldValue::Bool(dto.is_predator)
}
fn read_is_tower(dto: &PalDto) -> FieldValue {
    FieldValue::Bool(dto.is_tower)
}
fn read_is_sick(dto: &PalDto) -> FieldValue {
    FieldValue::Bool(dto.is_sick)
}
fn read_friendship_point(dto: &PalDto) -> FieldValue {
    FieldValue::Int(dto.friendship_point)
}
fn read_learned_skills(dto: &PalDto) -> FieldValue {
    FieldValue::List(dto.learned_skills.clone())
}
fn read_active_skills(dto: &PalDto) -> FieldValue {
    FieldValue::List(dto.active_skills.clone())
}
fn read_passive_skills(dto: &PalDto) -> FieldValue {
    FieldValue::List(dto.passive_skills.clone())
}
fn read_work_suitability(dto: &PalDto) -> FieldValue {
    FieldValue::Map(dto.work_suitability.clone())
}

// --- writers -------------------------------------------------------------

fn validate_nickname(_dto: &PalDto, value: &FieldValue) -> Result<(), HostError> {
    expect_str("nickname", value).map(|_| ())
}
fn apply_nickname(dto: &mut PalDto, value: FieldValue) {
    if let FieldValue::Str(text) = value {
        dto.nickname = Some(text);
    }
}

fn validate_gender(_dto: &PalDto, value: &FieldValue) -> Result<(), HostError> {
    let text = expect_str("gender", value)?;
    match text {
        "None" | "Male" | "Female" => Ok(()),
        other => {
            Err(HostError::new(format!("gender must be \"None\", \"Male\" or \"Female\", got {other:?}")))
        }
    }
}
fn apply_gender(dto: &mut PalDto, value: FieldValue) {
    if let FieldValue::Str(text) = value {
        dto.gender = match text.as_str() {
            "None" => PalGender::None,
            "Male" => PalGender::Male,
            _ => PalGender::Female,
        };
    }
}

fn validate_level(_dto: &PalDto, value: &FieldValue) -> Result<(), HostError> {
    ranged_int("level", value, 1, 255).map(|_| ())
}
fn apply_level(dto: &mut PalDto, value: FieldValue) {
    if let FieldValue::Int(v) = value {
        dto.level = v;
    }
}

fn validate_exp(_dto: &PalDto, value: &FieldValue) -> Result<(), HostError> {
    expect_int("exp", value).map(|_| ())
}
fn apply_exp(dto: &mut PalDto, value: FieldValue) {
    if let FieldValue::Int(v) = value {
        dto.exp = v;
    }
}

fn validate_rank(_dto: &PalDto, value: &FieldValue) -> Result<(), HostError> {
    ranged_int("rank", value, 0, 255).map(|_| ())
}
fn apply_rank(dto: &mut PalDto, value: FieldValue) {
    if let FieldValue::Int(v) = value {
        dto.rank = v;
    }
}

fn validate_rank_hp(_dto: &PalDto, value: &FieldValue) -> Result<(), HostError> {
    ranged_int("rank_hp", value, 0, 255).map(|_| ())
}
fn apply_rank_hp(dto: &mut PalDto, value: FieldValue) {
    if let FieldValue::Int(v) = value {
        dto.rank_hp = v;
    }
}

fn validate_rank_attack(_dto: &PalDto, value: &FieldValue) -> Result<(), HostError> {
    ranged_int("rank_attack", value, 0, 255).map(|_| ())
}
fn apply_rank_attack(dto: &mut PalDto, value: FieldValue) {
    if let FieldValue::Int(v) = value {
        dto.rank_attack = v;
    }
}

fn validate_rank_defense(_dto: &PalDto, value: &FieldValue) -> Result<(), HostError> {
    ranged_int("rank_defense", value, 0, 255).map(|_| ())
}
fn apply_rank_defense(dto: &mut PalDto, value: FieldValue) {
    if let FieldValue::Int(v) = value {
        dto.rank_defense = v;
    }
}

fn validate_rank_craftspeed(_dto: &PalDto, value: &FieldValue) -> Result<(), HostError> {
    ranged_int("rank_craftspeed", value, 0, 255).map(|_| ())
}
fn apply_rank_craftspeed(dto: &mut PalDto, value: FieldValue) {
    if let FieldValue::Int(v) = value {
        dto.rank_craftspeed = v;
    }
}

fn validate_talent_hp(_dto: &PalDto, value: &FieldValue) -> Result<(), HostError> {
    ranged_int("talent_hp", value, 0, 100).map(|_| ())
}
fn apply_talent_hp(dto: &mut PalDto, value: FieldValue) {
    if let FieldValue::Int(v) = value {
        dto.talent_hp = v;
    }
}

fn validate_talent_shot(_dto: &PalDto, value: &FieldValue) -> Result<(), HostError> {
    ranged_int("talent_shot", value, 0, 100).map(|_| ())
}
fn apply_talent_shot(dto: &mut PalDto, value: FieldValue) {
    if let FieldValue::Int(v) = value {
        dto.talent_shot = v;
    }
}

fn validate_talent_defense(_dto: &PalDto, value: &FieldValue) -> Result<(), HostError> {
    ranged_int("talent_defense", value, 0, 100).map(|_| ())
}
fn apply_talent_defense(dto: &mut PalDto, value: FieldValue) {
    if let FieldValue::Int(v) = value {
        dto.talent_defense = v;
    }
}

/// The stored prefix is case-sensitive everywhere in `apply_pal_dto` (see
/// `boss_prefix_is_a_lucky_marker`'s doc), but the *gate* deciding whether a
/// demoting write even needs scrutiny must not be: `apply_pal_dto` derives
/// `should_be_boss` from a case-insensitive `to_uppercase().starts_with`, so
/// a `Boss_Foxparks`-cased id is already boss-eligible there even though it
/// fails an exact `starts_with("BOSS_")`. Missing that would let a
/// mixed-case prefix sail through this table's checks -- since
/// `boss_prefix_is_a_lucky_marker`'s exact-case `strip_prefix` would then
/// fail to strip it either -- leaving `is_lucky = false` with the prefix
/// untouched, which `apply_pal_dto`'s asymmetric case handling on flush
/// turns into a doubled `BOSS_Boss_Foxparks` id. Widening this gate alone is
/// enough: it makes `boss_prefix_is_a_lucky_marker`'s exact-case check fail
/// for a mixed-case prefix, which correctly refuses instead of attempting an
/// unreliable strip. `apply_pal_dto`'s own asymmetry is pre-existing and out
/// of scope; this only keeps this table from writing into it.
fn character_id_carries_boss_prefix(character_id: &str) -> bool {
    character_id.to_uppercase().starts_with("BOSS_")
}

/// True only when the `BOSS_` prefix on `character_id` really is the
/// lucky/boss marker rather than part of a human/NPC pal's own species name.
/// `format_character_key` (`psp-core/src/dto/pal.rs:64-70`) already draws
/// this distinction on read: it strips `boss_` only when the raw id is not
/// itself a known `pals.json` key. `pals.json` has 35 keys that literally
/// begin with `BOSS_` (`BOSS_Male_People`, `BOSS_DarkTrader`, `BOSS_Ninja`,
/// ...); for those, `character_key` keeps the prefix (`boss_male_people`),
/// so stripping it here would fabricate a species (`Male_People`) that may
/// not exist in `pals.json` at all. For an ordinary boosted pal
/// (`BOSS_Foxparks`) `character_key` is `foxparks`, which equals the
/// stripped, lowercased id -- that equality is the whole test. Deliberately
/// exact-case (`strip_prefix`, not the case-insensitive gate above): a
/// mixed-case prefix must fail this so the caller refuses rather than
/// strips, since `apply_pal_dto` cannot reliably remove anything but an
/// exact `BOSS_` prefix either.
fn boss_prefix_is_a_lucky_marker(dto: &PalDto) -> bool {
    dto.character_id.strip_prefix("BOSS_").is_some_and(|stripped| dto.character_key == stripped.to_lowercase())
}

/// `boss_prefix_is_a_lucky_marker` trusts `dto.character_key`, which is only
/// meaningful when the species catalog it was computed from actually loaded.
/// `GameData::load` tolerates a missing or malformed `pals.json` by leaving
/// the catalog empty rather than erroring, and `format_character_key` strips
/// `boss_` unconditionally when the catalog is empty (nothing is ever a
/// "known key" against an empty set) -- so every `BOSS_`-prefixed id,
/// including `BOSS_Male_People`, would silently look like a safe strip. An
/// unavailable catalog is not evidence that stripping is safe, so this is
/// checked separately, ahead of the ordinary species-name refusal, and
/// refuses outright rather than falling back to that check's (unreliable,
/// in this case) answer.
fn refuse_is_lucky_demote_without_a_catalog(
    game_data: &GameData,
    current: &PalDto,
    value: &FieldValue,
) -> Result<(), HostError> {
    let FieldValue::Bool(false) = value else { return Ok(()) };
    if current.is_lucky != Some(true) || !character_id_carries_boss_prefix(&current.character_id) {
        return Ok(());
    }
    if pal::known_pal_keys(game_data).is_empty() {
        return Err(HostError::new(format!(
            "cannot set is_lucky to false on {:?}: the pal species catalog is unavailable, so it \
             cannot be confirmed safe to strip the BOSS_ prefix",
            current.character_id
        )));
    }
    Ok(())
}

/// `character_id` is read-only everywhere else in this table, but the domain
/// already treats it and `is_lucky` as one coupled pair for an ordinary
/// pal: `apply_pal_dto` derives boss-ness from the `BOSS_` prefix once
/// `IsRarePal` is gone (`psp-core/src/domain/pal.rs:691-692`). Demoting a
/// lucky pal (`is_lucky` currently `true`, `character_id` still prefixed)
/// without also stripping that prefix would leave the pal boss-flagged
/// instead of plain -- the opposite of what setting `is_lucky = false`
/// means -- and HP would stay at the boosted value either way. But the
/// prefix is not always a lucky marker (see `boss_prefix_is_a_lucky_marker`),
/// so this is refused, naming the species, rather than risk fabricating one
/// that does not exist in `pals.json`. The catalog-availability refusal
/// (`refuse_is_lucky_demote_without_a_catalog`) runs separately, before this,
/// since it needs `GameData` and this does not.
fn validate_is_lucky(dto: &PalDto, value: &FieldValue) -> Result<(), HostError> {
    let new_value = expect_bool("is_lucky", value)?;
    if !new_value
        && dto.is_lucky == Some(true)
        && character_id_carries_boss_prefix(&dto.character_id)
        && !boss_prefix_is_a_lucky_marker(dto)
    {
        return Err(HostError::new(format!(
            "cannot set is_lucky to false on {:?}: its BOSS_ prefix is part of this pal's own \
             species name, not a lucky marker, and character_id is read-only",
            dto.character_id
        )));
    }
    Ok(())
}
/// Only ever strips exactly the prefix that `is_lucky` itself put there --
/// never any other part of `character_id` -- and only when
/// `boss_prefix_is_a_lucky_marker` confirms it is safe to (which
/// `validate_is_lucky` has already checked before this runs). Without this,
/// `is_lucky` would be advertised read-write but its `false` value would be
/// unreachable for every pal that is actually, safely, lucky.
fn apply_is_lucky(dto: &mut PalDto, value: FieldValue) {
    if let FieldValue::Bool(v) = value {
        if !v && dto.is_lucky == Some(true) && boss_prefix_is_a_lucky_marker(dto) {
            if let Some(stripped) = dto.character_id.strip_prefix("BOSS_") {
                dto.character_id = stripped.to_string();
            }
        }
        dto.is_lucky = Some(v);
    }
}

fn validate_is_awakened(_dto: &PalDto, value: &FieldValue) -> Result<(), HostError> {
    expect_bool("is_awakened", value).map(|_| ())
}
fn apply_is_awakened(dto: &mut PalDto, value: FieldValue) {
    if let FieldValue::Bool(v) = value {
        dto.is_awakened = Some(v);
    }
}

fn validate_is_imported(_dto: &PalDto, value: &FieldValue) -> Result<(), HostError> {
    expect_bool("is_imported", value).map(|_| ())
}
fn apply_is_imported(dto: &mut PalDto, value: FieldValue) {
    if let FieldValue::Bool(v) = value {
        dto.is_imported = Some(v);
    }
}

fn validate_friendship_point(_dto: &PalDto, value: &FieldValue) -> Result<(), HostError> {
    ranged_int("friendship_point", value, i32::MIN as i64, i32::MAX as i64).map(|_| ())
}
fn apply_friendship_point(dto: &mut PalDto, value: FieldValue) {
    if let FieldValue::Int(v) = value {
        dto.friendship_point = v;
    }
}

fn validate_learned_skills(_dto: &PalDto, value: &FieldValue) -> Result<(), HostError> {
    expect_list("learned_skills", value).map(|_| ())
}
fn apply_learned_skills(dto: &mut PalDto, value: FieldValue) {
    if let FieldValue::List(items) = value {
        dto.learned_skills = items;
    }
}

fn validate_active_skills(_dto: &PalDto, value: &FieldValue) -> Result<(), HostError> {
    expect_list("active_skills", value).map(|_| ())
}
fn apply_active_skills(dto: &mut PalDto, value: FieldValue) {
    if let FieldValue::List(items) = value {
        dto.active_skills = items;
    }
}

fn validate_passive_skills(_dto: &PalDto, value: &FieldValue) -> Result<(), HostError> {
    expect_list("passive_skills", value).map(|_| ())
}
fn apply_passive_skills(dto: &mut PalDto, value: FieldValue) {
    if let FieldValue::List(items) = value {
        dto.passive_skills = items;
    }
}

/// `apply_pal_dto` writes each rank through `as i32`, so a wider value would
/// silently wrap on the way into the save rather than being refused here.
fn validate_work_suitability(_dto: &PalDto, value: &FieldValue) -> Result<(), HostError> {
    let FieldValue::Map(entries) = value else {
        return Err(HostError::new(format!(
            "expected a table of work-suitability ranks for work_suitability, got {}",
            field_value_type_name(value)
        )));
    };
    for (key, rank) in entries.iter() {
        if !WORK_SUITABILITIES.contains(&key.as_str()) {
            return Err(HostError::new(format!(
                "work_suitability has no key {key:?}; the keys are {}",
                WORK_SUITABILITIES.join(", ")
            )));
        }
        if !(i64::from(i32::MIN)..=i64::from(i32::MAX)).contains(rank) {
            return Err(HostError::new(format!(
                "work_suitability rank for {key:?} must be between {} and {}, got {rank}",
                i32::MIN,
                i32::MAX
            )));
        }
    }
    Ok(())
}
/// Built by walking `WORK_SUITABILITIES` rather than the assigned map's own
/// entries: `OrderedMap`'s order is the JSON key order the frontend receives,
/// and the order Lua's `pairs` hands over is not an order at all.
///
/// A zero rank is dropped here for the same reason `apply_pal_dto` drops one on
/// the way into the save: it filters `rank != 0` before writing
/// `GotWorkSuitabilityAddRankList`, and removes the property outright when
/// nothing survives. Keeping the zero in the cached DTO would leave a value the
/// flush is going to discard, so the same read would answer `0` before a flush
/// and `nil` after one -- and a dry run, which never flushes, would preview a
/// map the real run does not produce. Dropping it here keeps one representation
/// instead of two.
fn apply_work_suitability(dto: &mut PalDto, value: FieldValue) {
    if let FieldValue::Map(entries) = value {
        let mut ordered = OrderedMap::new();
        for name in WORK_SUITABILITIES {
            match entries.get(name) {
                Some(rank) if *rank != 0 => ordered.insert(name.to_string(), *rank),
                _ => {}
            }
        }
        dto.work_suitability = ordered;
    }
}

/// Which `data/json` catalog a skill field's entries must come from. The app
/// picks `learned_skills` out of the active-skill catalog too -- its editor
/// enumerates the same table `active_skills` does -- so both answer
/// `active_skills`.
fn skill_catalog(field: &str) -> Option<&'static str> {
    match field {
        "learned_skills" | "active_skills" => Some("active_skills"),
        "passive_skills" => Some("passive_skills"),
        _ => None,
    }
}

/// Exact match only. A mis-cased id would be accepted and then stored verbatim
/// -- `apply_*_skills` never rewrites it to the catalog's spelling, and nothing
/// downstream does either, since `apply_pal_dto` writes `MasteredWaza`,
/// `EquipWaza` and `PassiveSkillList` through with no catalog check at all. So
/// a lenient match here would put a spelling the game does not know into the
/// save, which is worse than refusing the write. `work_suitability` keys are
/// exact-case too, but the stake there is lower: `apply_pal_dto` filters out a
/// name it does not recognize, so a mis-cased key would be dropped on the way to
/// the save rather than written to it.
fn catalog_holds(game_data: &GameData, catalog: &str, id: &str) -> bool {
    game_data
        .get(catalog)
        .and_then(serde_json::Value::as_object)
        .is_some_and(|entries| entries.contains_key(id))
}

/// Kept out of the row's own `validate`, which sees only the DTO: the catalogs
/// live on `GameData`, the same way `is_lucky`'s species check does. A non-list
/// is an error rather than a pass -- the row's `validate` rejects one first
/// today, but nothing here should depend on that still being true.
fn validate_skill_entries(
    game_data: &GameData,
    field: &str,
    catalog: &str,
    value: &FieldValue,
) -> Result<(), HostError> {
    let FieldValue::List(items) = value else {
        return Err(HostError::new(format!(
            "expected a list of strings for {field}, got {}",
            field_value_type_name(value)
        )));
    };
    for id in items {
        if !catalog_holds(game_data, catalog, id) {
            return Err(HostError::new(format!(
                "{field} entry {id:?} is not in the {catalog} catalog"
            )));
        }
    }
    Ok(())
}

const fn rw(
    name: &'static str,
    ty: ApiType,
    doc: &'static str,
    read: fn(&PalDto) -> FieldValue,
    validate: fn(&PalDto, &FieldValue) -> Result<(), HostError>,
    apply: fn(&mut PalDto, FieldValue),
) -> FieldSpec<PalDto, PalSummary> {
    FieldSpec {
        name,
        ty,
        access: Access::ReadWrite,
        doc,
        read: Reader::Dto(read),
        write: Some(FieldWrite { validate, apply }),
    }
}

const fn ro(
    name: &'static str,
    ty: ApiType,
    doc: &'static str,
    read: fn(&PalDto) -> FieldValue,
) -> FieldSpec<PalDto, PalSummary> {
    FieldSpec { name, ty, access: Access::ReadOnly, doc, read: Reader::Dto(read), write: None }
}

/// Like `ro`, but for a row with no corresponding `PalDto` field at all --
/// its value only ever existed on the cached pal summary.
const fn ro_summary(
    name: &'static str,
    ty: ApiType,
    doc: &'static str,
    read: fn(&PalSummary) -> FieldValue,
) -> FieldSpec<PalDto, PalSummary> {
    FieldSpec { name, ty, access: Access::ReadOnly, doc, read: Reader::Summary(read), write: None }
}

/// Every field this handle answers for: the fields the pal summary already
/// carries (most of them now also assignable), the fields that only live on
/// the full `PalDto`, or only on the summary, and the four collections.
/// `filtered_nickname` is the one `PalDto` field still missing, and stays
/// missing -- it never applies to a `Level.sav` pal.
pub const PAL_FIELDS: &[FieldSpec<PalDto, PalSummary>] = &[
    ro("instance_id", ApiType::String, "This pal's unique id, as a string. Read-only.", read_instance_id),
    ro(
        "character_id",
        ApiType::String,
        "The pal's species id, including a BOSS_ prefix when it has one. Read-only in itself, \
         but writing is_lucky changes it: setting is_lucky = false removes a BOSS_ prefix that \
         only marked the pal lucky, and setting it true puts one back when the change is saved. \
         A dry run saves nothing, so after is_lucky = true this still reads the unprefixed id.",
        read_character_id,
    ),
    ro(
        "character_key",
        ApiType::String,
        "The pal's species key, as used to look up game data. Read-only, and refreshed only \
         when the pal is next read from the save, so during a dry run it can still name the old \
         species after a write that changed character_id.",
        read_character_key,
    ),
    rw(
        "nickname",
        ApiType::Union(&[ApiType::String, ApiType::Nil]),
        "The pal's nickname, or nil if it has none.",
        read_nickname,
        validate_nickname,
        apply_nickname,
    ),
    ro(
        "owner_uid",
        ApiType::Union(&[ApiType::String, ApiType::Nil]),
        "The id of the player who owns this pal, or nil if a guild base owns it instead. \
         Read-only.",
        read_owner_uid,
    ),
    ro_summary(
        "guild_id",
        ApiType::Union(&[ApiType::String, ApiType::Nil]),
        "The id of the guild whose base this pal works at, or nil if it works at none. \
         Read-only.",
        read_guild_id_from_summary,
    ),
    ro_summary(
        "base_id",
        ApiType::Union(&[ApiType::String, ApiType::Nil]),
        "The id of the base this pal works at, or nil if it works at none. Read-only.",
        read_base_id_from_summary,
    ),
    rw(
        "gender",
        ApiType::String,
        "\"None\", \"Male\" or \"Female\".",
        read_gender,
        validate_gender,
        apply_gender,
    ),
    rw("level", ApiType::Integer, "The pal's level, 1-255.", read_level, validate_level, apply_level),
    ro(
        "hp",
        ApiType::Integer,
        "The pal's current HP. Read-only: assigning it raises. It is recalculated whenever the \
         pal is saved, so a value set here could not have been kept anyway; a dry run saves \
         nothing, so this keeps reading what it read before any of the run's writes.",
        read_hp,
    ),
    rw("rank", ApiType::Integer, "The pal's condensing rank, 0-255.", read_rank, validate_rank, apply_rank),
    rw("exp", ApiType::Integer, "The pal's experience points.", read_exp, validate_exp, apply_exp),
    rw(
        "talent_hp",
        ApiType::Integer,
        "HP talent value, 0-100.",
        read_talent_hp,
        validate_talent_hp,
        apply_talent_hp,
    ),
    rw(
        "talent_shot",
        ApiType::Integer,
        "Ranged-attack talent value, 0-100.",
        read_talent_shot,
        validate_talent_shot,
        apply_talent_shot,
    ),
    rw(
        "talent_defense",
        ApiType::Integer,
        "Defense talent value, 0-100.",
        read_talent_defense,
        validate_talent_defense,
        apply_talent_defense,
    ),
    rw(
        "rank_hp",
        ApiType::Integer,
        "HP soul rank, 0-255.",
        read_rank_hp,
        validate_rank_hp,
        apply_rank_hp,
    ),
    rw(
        "rank_attack",
        ApiType::Integer,
        "Attack soul rank, 0-255.",
        read_rank_attack,
        validate_rank_attack,
        apply_rank_attack,
    ),
    rw(
        "rank_defense",
        ApiType::Integer,
        "Defense soul rank, 0-255.",
        read_rank_defense,
        validate_rank_defense,
        apply_rank_defense,
    ),
    rw(
        "rank_craftspeed",
        ApiType::Integer,
        "Craft-speed soul rank, 0-255.",
        read_rank_craftspeed,
        validate_rank_craftspeed,
        apply_rank_craftspeed,
    ),
    ro(
        "is_boss",
        ApiType::Boolean,
        "True for a boss/alpha pal. Read-only, and never true at the same time as is_lucky: a \
         lucky pal carries the same BOSS_ prefix but is not a boss. That exclusion is applied \
         when the pal is saved, so during a dry run this can still read true right after \
         setting is_lucky = true on a boss pal.",
        read_is_boss,
    ),
    rw(
        "is_lucky",
        ApiType::Boolean,
        "True for a lucky pal. Setting it false also removes the BOSS_ prefix from \
         character_id, so the pal ends up plain rather than a boss; the write is refused, \
         naming the species, when that prefix is part of the species' own name.",
        read_is_lucky,
        validate_is_lucky,
        apply_is_lucky,
    ),
    rw(
        "is_awakened",
        ApiType::Boolean,
        "True if this pal has been awakened.",
        read_is_awakened,
        validate_is_awakened,
        apply_is_awakened,
    ),
    rw(
        "is_imported",
        ApiType::Boolean,
        "True if this pal was imported from another save.",
        read_is_imported,
        validate_is_imported,
        apply_is_imported,
    ),
    ro("is_predator", ApiType::Boolean, "True for a predator-species pal. Read-only.", read_is_predator),
    ro("is_tower", ApiType::Boolean, "True for a tower-boss pal. Read-only.", read_is_tower),
    ro(
        "group_id",
        ApiType::Union(&[ApiType::String, ApiType::Nil]),
        "The id of the group this pal belongs to, or nil if it belongs to none. Read-only.",
        read_group_id,
    ),
    ro(
        "stomach",
        ApiType::Number,
        "The pal's current fullness. Read-only: assigning it raises. The pal is fed back to \
         full whenever it is saved, so a value set here could not have been kept anyway; a dry \
         run saves nothing, so this keeps reading what it read before any of the run's writes.",
        read_stomach,
    ),
    ro(
        "sanity",
        ApiType::Number,
        "The pal's current sanity. Read-only: assigning it raises. It is restored to 100 \
         whenever the pal is saved, so a value set here could not have been kept anyway; a dry \
         run saves nothing, so this keeps reading what it read before any of the run's writes.",
        read_sanity,
    ),
    ro(
        "max_hp",
        ApiType::Integer,
        "The pal's maximum HP. Read-only: it is recalculated whenever the pal is saved, so \
         during a dry run it does not move when level, rank or a talent changes.",
        read_max_hp,
    ),
    ro(
        "storage_slot",
        ApiType::Integer,
        "This pal's slot number inside the container holding it. Read-only: assigning it \
         raises rather than moving the pal. Nothing would check whether the slot you named was \
         already taken, so allowing the write would risk putting two pals in the same place.",
        read_storage_slot,
    ),
    ro("storage_id", ApiType::String, "The id of the container holding this pal. Read-only.", read_storage_id),
    ro(
        "is_sick",
        ApiType::Boolean,
        "True if this pal is sick. Read-only: sickness is cleared whenever the pal is saved, so \
         during a dry run a sick pal keeps reading true.",
        read_is_sick,
    ),
    rw(
        "friendship_point",
        ApiType::Integer,
        "The pal's friendship points.",
        read_friendship_point,
        validate_friendship_point,
        apply_friendship_point,
    ),
    rw(
        "learned_skills",
        ApiType::List(&ApiType::String),
        "Every active skill this pal has learned, as catalog ids like \
         \"EPalWazaID::FireBall\", spelled exactly as the catalog spells them. Assigning \
         replaces the whole list, and every entry must be an active-skill id; any id in that \
         catalog is accepted, including a species-specific skill belonging to some other pal, \
         which the in-app skill picker would not offer you. The read returns a fresh table each \
         time, so changing that table changes nothing.",
        read_learned_skills,
        validate_learned_skills,
        apply_learned_skills,
    ),
    rw(
        "active_skills",
        ApiType::List(&ApiType::String),
        "The active skills this pal has equipped, as catalog ids like \
         \"EPalWazaID::FireBall\", spelled exactly as the catalog spells them. Assigning \
         replaces the whole list; any id in that catalog is accepted, including a \
         species-specific skill belonging to some other pal, which the in-app skill picker \
         would not offer you. The read returns a fresh table each time, so changing that table \
         changes nothing.",
        read_active_skills,
        validate_active_skills,
        apply_active_skills,
    ),
    rw(
        "passive_skills",
        ApiType::List(&ApiType::String),
        "The passive skills this pal carries, as catalog ids like \"Rare\", spelled exactly as \
         the catalog spells them. Assigning replaces the whole list; the read returns a fresh \
         table each time, so changing that table changes nothing.",
        read_passive_skills,
        validate_passive_skills,
        apply_passive_skills,
    ),
    rw(
        "work_suitability",
        ApiType::Map { key: &ApiType::String, value: &ApiType::Integer },
        "Work-suitability ranks added on top of the species' own, keyed by the wire names \
         EmitFlame, Watering, Seeding, GenerateElectricity, Handcraft, Collection, Deforest, \
         Mining, OilExtraction, ProductMedicine, Cool, Transport and MonsterFarm. Assigning \
         replaces the whole map, and a key it does not know is refused. Assigning a rank of \
         zero removes that key instead of storing it, so it reads back absent straight away, \
         and saving never stores a zero either. A save written by some other tool can still \
         hold one, and reading that pal gives you the zero it holds.",
        read_work_suitability,
        validate_work_suitability,
        apply_work_suitability,
    ),
];

static API_FIELDS: OnceLock<Vec<ApiField>> = OnceLock::new();

/// The published description of this handle's fields, projected from the same
/// rows that answer the reads and validate the writes. Not a `const`: mapping
/// `PAL_FIELDS` into the `&'static [ApiField]` an `ApiHandle` holds has no
/// const form.
pub(crate) fn api_fields() -> &'static [ApiField] {
    API_FIELDS
        .get_or_init(|| {
            PAL_FIELDS
                .iter()
                .map(|spec| ApiField {
                    name: spec.name,
                    ty: spec.ty.clone(),
                    access: spec.access,
                    doc: spec.doc,
                })
                .collect()
        })
        .as_slice()
}

fn find(field: &str) -> Option<&'static FieldSpec<PalDto, PalSummary>> {
    PAL_FIELDS.iter().find(|spec| spec.name == field)
}

/// Reads one field, from the cached `PalDto` or from the pal summary
/// depending on the row's `Reader`. An unrecognized field name returns `Nil`,
/// matching how every other handle's read side already treats a name it does
/// not carry.
pub(crate) fn pal_get(ctx: &mut RunContext<'_>, id: Uuid, field: &str) -> Result<FieldValue, HostError> {
    let Some(spec) = find(field) else {
        return Ok(FieldValue::Nil);
    };
    match spec.read {
        Reader::Dto(read) => {
            let dto = dto_cache::pal_read(ctx, id)?;
            Ok(read(dto))
        }
        Reader::Summary(read) => {
            ensure_pals_snapshot(ctx)?;
            let Some((snapshot, index)) = ctx.pals.as_ref() else {
                return Ok(FieldValue::Nil);
            };
            let Some(entry) = index.get(&id) else {
                return Ok(FieldValue::Nil);
            };
            let Some(summary) = snapshot.get(entry.position) else {
                return Ok(FieldValue::Nil);
            };
            Ok(read(summary))
        }
    }
}

/// `save.write` is checked before any field resolution -- before the name is
/// looked up, and so before `unknown pal field` or `is read-only` can be
/// reported -- so an ungranted write is not told which fields exist or which
/// of them it could have written.
///
/// It is not the very first thing that happens to the assignment, though:
/// `pal_newindex` has already read the value off the stack and checked its
/// shape by the time this runs, because a `FieldValue` is what this takes. So
/// an ungranted write of a malformed table reports the malformed table. That
/// costs nothing -- the shape check reads the stack and touches no save data,
/// and neither path reaches a write.
pub(crate) fn pal_set(ctx: &mut RunContext<'_>, id: Uuid, field: &str, value: FieldValue) -> Result<(), HostError> {
    if !ctx.grants(Capability::SaveWrite) {
        return Err(HostError::new("pal field assignment requires the save.write capability"));
    }
    let Some(spec) = find(field) else {
        return Err(HostError::new(format!("unknown pal field {field:?}")));
    };
    let Some(write) = spec.write.as_ref() else {
        return Err(HostError::new(format!("{field} is read-only")));
    };
    // An empty Lua table is an empty list and an empty map at once, and the
    // reader cannot tell them apart; the row's declared type can.
    let value = match (&spec.ty, value) {
        (ApiType::Map { .. }, FieldValue::List(items)) if items.is_empty() => {
            FieldValue::Map(OrderedMap::new())
        }
        (_, other) => other,
    };
    let game_data = ctx.game_data;
    let current = dto_cache::pal_read(ctx, id)?;
    if spec.name == "is_lucky" {
        refuse_is_lucky_demote_without_a_catalog(game_data, current, &value)?;
    }
    (write.validate)(current, &value)?;
    if let Some(catalog) = skill_catalog(spec.name) {
        validate_skill_entries(game_data, spec.name, catalog, &value)?;
    }
    // Counted once per accepted assignment, not once per claimed field: an
    // `is_lucky` demote claims `character_id` too, and a preview that said a
    // plugin would write two fields when the script assigned one would be a
    // lie about the script.
    if ctx.dry_run {
        ctx.bump(&format!("pal.{}", spec.name), 1);
    }
    let apply = write.apply;
    if spec.name != "is_lucky" {
        return dto_cache::pal_write(ctx, id, &[spec.name], move |dto| apply(dto, value));
    }
    // Only the demoting write rewrites `character_id` (see `apply_is_lucky`),
    // so only then does the cached id already equal what the flush will write.
    // The promoting write leaves the id alone and lets `apply_pal_dto` put the
    // prefix back, so any claim a demote earlier this run left behind has to be
    // released -- unclaimed, the read rebuilds the snapshot, which flushes
    // first and so reports the id that genuinely reaches the save.
    if matches!(value, FieldValue::Bool(false)) {
        return dto_cache::pal_write(ctx, id, &["is_lucky", "character_id"], move |dto| apply(dto, value));
    }
    dto_cache::pal_write(ctx, id, &["is_lucky"], move |dto| apply(dto, value))?;
    dto_cache::pal_release_field(ctx, id, "character_id");
    Ok(())
}

fn pal_newindex(state: *mut lua_State) -> Result<c_int, HostError> {
    unsafe {
        check_args(state, 3, "pal field assignment")?;
        let handle = read_handle(state, 1, HandleKind::Pal)?;
        let field = arg_string(state, 2, "field")?;
        let value = read_field_value(state, 3, &field)?;
        with_context(state, |ctx| pal_set(ctx, handle.id, &field, value))?;
        Ok(0)
    }
}

host_fn!(push_pal_newindex, pal_newindex);

#[cfg(test)]
mod tests {
    use super::*;

    fn catalog() -> GameData {
        GameData::from_entries([(
            "active_skills".to_string(),
            r#"{"EPalWazaID::FireBall": {}}"#.to_string(),
        )])
        .expect("the test catalog parses")
    }

    /// `pal_set` runs the row's own `validate` first, so a non-list never
    /// reaches this today. It still has to refuse one on its own rather than
    /// pass a value it never inspected, since nothing here should depend on
    /// two calls staying in that order.
    #[test]
    fn validate_skill_entries_refuses_a_value_that_is_not_a_list() {
        let error = validate_skill_entries(
            &catalog(),
            "active_skills",
            "active_skills",
            &FieldValue::Str("EPalWazaID::FireBall".to_string()),
        )
        .expect_err("a bare string is not a list of skills");
        let message = error.into_message();
        assert!(message.contains("active_skills"), "must name the field, got {message:?}");
        assert!(message.contains("string"), "must name the type it got, got {message:?}");
    }

    #[test]
    fn validate_skill_entries_matches_catalog_keys_exactly() {
        let data = catalog();
        assert!(validate_skill_entries(
            &data,
            "active_skills",
            "active_skills",
            &FieldValue::List(vec!["EPalWazaID::FireBall".to_string()]),
        )
        .is_ok());
        assert!(validate_skill_entries(
            &data,
            "active_skills",
            "active_skills",
            &FieldValue::List(vec!["epalwazaid::fireball".to_string()]),
        )
        .is_err());
    }
}
