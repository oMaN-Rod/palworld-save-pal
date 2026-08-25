use std::ffi::c_int;
use std::sync::OnceLock;

use psp_core::domain::pal;
use psp_core::dto::pal::{PalDto, PalGender};
use psp_core::dto::summary::PalSummary;
use psp_core::gamedata::GameData;
use psp_lua_sys::ffi::*;
use uuid::Uuid;

use super::{field_value_type_name, read_field_value, Access, FieldSpec, FieldValue, FieldWrite, Reader};
use crate::context::RunContext;
use crate::host::api_def::{ApiField, ApiType};
use crate::host::handle::{read_handle, HandleKind};
use crate::host::marshal::{arg_string, check_args};
use crate::host::save_read::ensure_pals_snapshot;
use crate::host::{dto_cache, with_context, HostError};
use crate::host_fn;
use crate::manifest::Capability;

fn expect_int(name: &str, value: &FieldValue) -> Result<i64, HostError> {
    match value {
        FieldValue::Int(v) => Ok(*v),
        other => {
            Err(HostError::new(format!("expected an integer for {name}, got {}", field_value_type_name(other))))
        }
    }
}

fn expect_bool(name: &str, value: &FieldValue) -> Result<bool, HostError> {
    match value {
        FieldValue::Bool(v) => Ok(*v),
        other => {
            Err(HostError::new(format!("expected a boolean for {name}, got {}", field_value_type_name(other))))
        }
    }
}

fn expect_str<'v>(name: &str, value: &'v FieldValue) -> Result<&'v str, HostError> {
    match value {
        FieldValue::Str(v) => Ok(v.as_str()),
        other => {
            Err(HostError::new(format!("expected a string for {name}, got {}", field_value_type_name(other))))
        }
    }
}

fn ranged_int(name: &str, value: &FieldValue, lo: i64, hi: i64) -> Result<i64, HostError> {
    let v = expect_int(name, value)?;
    if !(lo..=hi).contains(&v) {
        return Err(HostError::new(format!("{name} must be between {lo} and {hi}, got {v}")));
    }
    Ok(v)
}

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

const fn rw(
    name: &'static str,
    ty: ApiType,
    doc: &'static str,
    read: fn(&PalDto) -> FieldValue,
    validate: fn(&PalDto, &FieldValue) -> Result<(), HostError>,
    apply: fn(&mut PalDto, FieldValue),
) -> FieldSpec<PalDto> {
    FieldSpec {
        name,
        ty,
        access: Access::ReadWrite,
        doc,
        read: Reader::Dto(read),
        write: Some(FieldWrite { validate, apply }),
    }
}

const fn ro(name: &'static str, ty: ApiType, doc: &'static str, read: fn(&PalDto) -> FieldValue) -> FieldSpec<PalDto> {
    FieldSpec { name, ty, access: Access::ReadOnly, doc, read: Reader::Dto(read), write: None }
}

/// Like `ro`, but for a row with no corresponding `PalDto` field at all --
/// its value only ever existed on the cached pal summary.
const fn ro_summary(
    name: &'static str,
    ty: ApiType,
    doc: &'static str,
    read: fn(&PalSummary) -> FieldValue,
) -> FieldSpec<PalDto> {
    FieldSpec { name, ty, access: Access::ReadOnly, doc, read: Reader::Summary(read), write: None }
}

/// Every scalar field this handle answers for: the fields the pal summary
/// already carries (most of them now also assignable) plus the fields that
/// only live on the full `PalDto`, or only on the summary, and were
/// previously unreadable through this handle at all. `learned_skills`,
/// `active_skills`, `passive_skills` and `work_suitability` are collections,
/// not scalars, and `filtered_nickname` never applies to a `Level.sav` pal --
/// none of the five belong here.
pub const PAL_FIELDS: &[FieldSpec<PalDto>] = &[
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

fn find(field: &str) -> Option<&'static FieldSpec<PalDto>> {
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

/// `save.write` is checked before anything else, including whether the field
/// name is even known, so an ungranted write always reports the missing
/// capability rather than a possibly-confusing message about the field.
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
    let game_data = ctx.game_data;
    let current = dto_cache::pal_read(ctx, id)?;
    if spec.name == "is_lucky" {
        refuse_is_lucky_demote_without_a_catalog(game_data, current, &value)?;
    }
    (write.validate)(current, &value)?;
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
        let value = read_field_value(state, 3)?;
        with_context(state, |ctx| pal_set(ctx, handle.id, &field, value))?;
        Ok(0)
    }
}

host_fn!(push_pal_newindex, pal_newindex);
