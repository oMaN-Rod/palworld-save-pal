use std::ffi::c_int;
use std::sync::OnceLock;

use psp_core::domain::player::{EX_STATUS_NAME_MAP, STATUS_NAME_MAP};
use psp_core::dto::ordered_map::OrderedMap;
use psp_core::dto::player::PlayerDto;
use psp_core::dto::summary::PlayerSummary;
use psp_lua_sys::ffi::*;
use uuid::Uuid;

use super::{
    expect_finite_f32, expect_int, expect_list, expect_str, field_value_type_name, ranged_int,
    read_field_value, Access, FieldSpec, FieldValue, FieldWrite, Reader,
};
use crate::context::RunContext;
use crate::host::api_def::{ApiField, ApiType};
use crate::host::handle::{read_handle, HandleKind};
use crate::host::marshal::{arg_string, check_args};
use crate::host::save_read::iso_string;
use crate::host::{dto_cache, with_context, HostError};
use crate::host_fn;
use crate::manifest::Capability;

fn optional_uuid(id: Option<Uuid>) -> FieldValue {
    id.map(|id| FieldValue::Str(id.to_string())).unwrap_or(FieldValue::Nil)
}

fn string_list(items: &[String]) -> FieldValue {
    FieldValue::List(items.to_vec())
}

/// `apply_player_dto` writes every one of these through `as i32` after a clamp,
/// so a wider value would land silently truncated rather than being refused.
fn i32_ranged(name: &str, value: &FieldValue) -> Result<(), HostError> {
    ranged_int(name, value, i64::from(i32::MIN), i64::from(i32::MAX)).map(|_| ())
}

// --- readers -------------------------------------------------------------

fn read_uid(summary: &PlayerSummary) -> FieldValue {
    FieldValue::Str(summary.uid.to_string())
}
fn read_pal_count(summary: &PlayerSummary) -> FieldValue {
    FieldValue::Int(summary.pal_count)
}
fn read_guild_id(summary: &PlayerSummary) -> FieldValue {
    optional_uuid(summary.guild_id)
}
fn read_last_online(summary: &PlayerSummary) -> FieldValue {
    iso_string(summary.last_online_time).map(FieldValue::Str).unwrap_or(FieldValue::Nil)
}
fn read_last_online_ts(summary: &PlayerSummary) -> FieldValue {
    summary
        .last_online_time
        .map(|t| FieldValue::Int(t.0.and_utc().timestamp()))
        .unwrap_or(FieldValue::Nil)
}

fn read_name(dto: &PlayerDto) -> FieldValue {
    FieldValue::Str(dto.nickname.clone())
}
fn read_level(dto: &PlayerDto) -> FieldValue {
    FieldValue::Int(dto.level)
}
fn read_exp(dto: &PlayerDto) -> FieldValue {
    FieldValue::Int(dto.exp)
}
fn read_hp(dto: &PlayerDto) -> FieldValue {
    FieldValue::Int(dto.hp)
}
fn read_stomach(dto: &PlayerDto) -> FieldValue {
    FieldValue::Float(dto.stomach)
}
fn read_sanity(dto: &PlayerDto) -> FieldValue {
    FieldValue::Float(dto.sanity)
}
fn read_technology_points(dto: &PlayerDto) -> FieldValue {
    FieldValue::Int(dto.technology_points)
}
fn read_boss_technology_points(dto: &PlayerDto) -> FieldValue {
    FieldValue::Int(dto.boss_technology_points)
}
fn read_technologies(dto: &PlayerDto) -> FieldValue {
    string_list(&dto.technologies)
}
fn read_completed_missions(dto: &PlayerDto) -> FieldValue {
    string_list(&dto.completed_missions)
}
fn read_current_missions(dto: &PlayerDto) -> FieldValue {
    string_list(&dto.current_missions)
}
fn read_unlocked_fast_travel_points(dto: &PlayerDto) -> FieldValue {
    string_list(dto.unlocked_fast_travel_points.as_deref().unwrap_or(&[]))
}
fn read_collected_effigies(dto: &PlayerDto) -> FieldValue {
    string_list(dto.collected_effigies.as_deref().unwrap_or(&[]))
}
fn read_defeated_bosses(dto: &PlayerDto) -> FieldValue {
    string_list(dto.defeated_bosses.as_deref().unwrap_or(&[]))
}
fn read_status_points(dto: &PlayerDto) -> FieldValue {
    FieldValue::Map(dto.status_point_list.clone())
}
fn read_ext_status_points(dto: &PlayerDto) -> FieldValue {
    FieldValue::Map(dto.ext_status_point_list.clone())
}
fn read_instance_id(dto: &PlayerDto) -> FieldValue {
    optional_uuid(dto.instance_id)
}
fn read_pal_box_id(dto: &PlayerDto) -> FieldValue {
    optional_uuid(dto.pal_box_id)
}
fn read_otomo_container_id(dto: &PlayerDto) -> FieldValue {
    optional_uuid(dto.otomo_container_id)
}
fn read_effigy_possess_num(dto: &PlayerDto) -> FieldValue {
    FieldValue::Int(dto.effigy_possess_num)
}

// --- writers -------------------------------------------------------------

/// The one string `apply_player_dto` does not write literally: it compares the
/// incoming nickname against this exact pattern and, on a match, *removes* the
/// `NickName` property instead of storing it. `build_player_dto` synthesises
/// the same string when a player has no `NickName` at all, so it is the save's
/// own spelling of "nameless".
fn nameless_pattern(uid: Uuid) -> String {
    format!("\u{1f977} ({})", uid.to_string().split('-').next().unwrap_or(""))
}

/// Refused rather than applied, on both counts, because either would make the
/// assignment do something other than what it says. The nameless pattern
/// removes the property, so the name would read back as a generated
/// placeholder; an empty string is stored literally, but `build_player_summary`
/// treats an empty `NickName` as no name and substitutes its own, different
/// placeholder, so the same read would answer one thing this run and another
/// the next time the save is opened.
fn validate_name(dto: &PlayerDto, value: &FieldValue) -> Result<(), HostError> {
    let text = expect_str("name", value)?;
    if text.is_empty() {
        return Err(HostError::new(
            "name cannot be empty: the save records an empty name as no name at all, and it \
             would read back as a generated placeholder rather than as what was assigned",
        ));
    }
    if text == nameless_pattern(dto.uid) {
        return Err(HostError::new(format!(
            "name cannot be set to {text:?}: that is the save's own placeholder for a player \
             with no name, and assigning it removes the name rather than storing it"
        )));
    }
    Ok(())
}
fn apply_name(dto: &mut PlayerDto, value: FieldValue) {
    if let FieldValue::Str(text) = value {
        dto.nickname = text;
    }
}

fn validate_level(_dto: &PlayerDto, value: &FieldValue) -> Result<(), HostError> {
    ranged_int("level", value, 1, 255).map(|_| ())
}
fn apply_level(dto: &mut PlayerDto, value: FieldValue) {
    if let FieldValue::Int(v) = value {
        dto.level = v;
    }
}

fn validate_exp(_dto: &PlayerDto, value: &FieldValue) -> Result<(), HostError> {
    expect_int("exp", value).map(|_| ())
}
fn apply_exp(dto: &mut PlayerDto, value: FieldValue) {
    if let FieldValue::Int(v) = value {
        dto.exp = v;
    }
}

fn validate_hp(_dto: &PlayerDto, value: &FieldValue) -> Result<(), HostError> {
    expect_int("hp", value).map(|_| ())
}
fn apply_hp(dto: &mut PlayerDto, value: FieldValue) {
    if let FieldValue::Int(v) = value {
        dto.hp = v;
    }
}

fn validate_stomach(_dto: &PlayerDto, value: &FieldValue) -> Result<(), HostError> {
    expect_finite_f32("stomach", value).map(|_| ())
}
fn apply_stomach(dto: &mut PlayerDto, value: FieldValue) {
    match value {
        FieldValue::Float(v) => dto.stomach = v,
        FieldValue::Int(v) => dto.stomach = v as f64,
        _ => {}
    }
}

fn validate_sanity(_dto: &PlayerDto, value: &FieldValue) -> Result<(), HostError> {
    expect_finite_f32("sanity", value).map(|_| ())
}
fn apply_sanity(dto: &mut PlayerDto, value: FieldValue) {
    match value {
        FieldValue::Float(v) => dto.sanity = v,
        FieldValue::Int(v) => dto.sanity = v as f64,
        _ => {}
    }
}

fn validate_technology_points(_dto: &PlayerDto, value: &FieldValue) -> Result<(), HostError> {
    i32_ranged("technology_points", value)
}
fn apply_technology_points(dto: &mut PlayerDto, value: FieldValue) {
    if let FieldValue::Int(v) = value {
        dto.technology_points = v;
    }
}

fn validate_boss_technology_points(_dto: &PlayerDto, value: &FieldValue) -> Result<(), HostError> {
    i32_ranged("boss_technology_points", value)
}
fn apply_boss_technology_points(dto: &mut PlayerDto, value: FieldValue) {
    if let FieldValue::Int(v) = value {
        dto.boss_technology_points = v;
    }
}

fn validate_technologies(_dto: &PlayerDto, value: &FieldValue) -> Result<(), HostError> {
    expect_list("technologies", value).map(|_| ())
}
fn apply_technologies(dto: &mut PlayerDto, value: FieldValue) {
    if let FieldValue::List(items) = value {
        dto.technologies = items;
    }
}

fn validate_completed_missions(_dto: &PlayerDto, value: &FieldValue) -> Result<(), HostError> {
    expect_list("completed_missions", value).map(|_| ())
}
fn apply_completed_missions(dto: &mut PlayerDto, value: FieldValue) {
    if let FieldValue::List(items) = value {
        dto.completed_missions = items;
    }
}

fn validate_current_missions(_dto: &PlayerDto, value: &FieldValue) -> Result<(), HostError> {
    expect_list("current_missions", value).map(|_| ())
}
fn apply_current_missions(dto: &mut PlayerDto, value: FieldValue) {
    if let FieldValue::List(items) = value {
        dto.current_missions = items;
    }
}

fn validate_unlocked_fast_travel_points(_dto: &PlayerDto, value: &FieldValue) -> Result<(), HostError> {
    expect_list("unlocked_fast_travel_points", value).map(|_| ())
}
fn apply_unlocked_fast_travel_points(dto: &mut PlayerDto, value: FieldValue) {
    if let FieldValue::List(items) = value {
        dto.unlocked_fast_travel_points = Some(items);
    }
}

fn validate_collected_effigies(_dto: &PlayerDto, value: &FieldValue) -> Result<(), HostError> {
    expect_list("collected_effigies", value).map(|_| ())
}
fn apply_collected_effigies(dto: &mut PlayerDto, value: FieldValue) {
    if let FieldValue::List(items) = value {
        dto.collected_effigies = Some(items);
    }
}

/// `apply_status_points` looks each key up in its name map and simply skips one
/// it does not recognise -- an unknown key is silently dropped, not reported.
/// So the key check has to happen here or not at all, exactly as it does for
/// `pal.work_suitability`.
fn validate_status_points(
    field: &str,
    name_map: &[(&str, &str)],
    value: &FieldValue,
) -> Result<(), HostError> {
    let FieldValue::Map(entries) = value else {
        return Err(HostError::new(format!(
            "expected a table of stat points for {field}, got {}",
            field_value_type_name(value)
        )));
    };
    for (key, point) in entries.iter() {
        if !name_map.iter().any(|(_, english)| *english == key.as_str()) {
            let known: Vec<&str> = name_map.iter().map(|(_, english)| *english).collect();
            return Err(HostError::new(format!(
                "{field} has no key {key:?}; the keys are {}",
                known.join(", ")
            )));
        }
        if !(i64::from(i32::MIN)..=i64::from(i32::MAX)).contains(point) {
            return Err(HostError::new(format!(
                "{field} value for {key:?} must be between {} and {}, got {point}",
                i32::MIN,
                i32::MAX
            )));
        }
    }
    Ok(())
}

/// Turns the assigned map into the map that, handed to `apply_status_points`,
/// actually replaces the list rather than merging into it.
///
/// The merge is the whole reason this exists. `apply_status_points` visits only
/// the keys the map it is given carries, so a key left out keeps whatever row
/// it already had -- assigning `{max_hp = 7}` would leave the other seventeen
/// stats untouched in the save while this handle reported a one-key map. There
/// is no removal to reach for either: the list has no "no row" write. What it
/// does have is a zero, which is what the save already means by an absent row
/// (the game creates a row lazily, only once a rank is bought, so an absent row
/// *is* rank zero). So every key of the name map is supplied, at zero wherever
/// the assignment left it out, and the replacement is one in the save and not
/// just in the docs.
///
/// A zero is then dropped from the result for a key the save has no row for,
/// because `apply_status_points` will not append one -- keeping it would make
/// the same read answer `0` before a flush and nothing after one, which is the
/// disagreement this function exists to remove. `saved_rows` is the key set as
/// the *save* has it, not as the cached DTO has it, for the reason
/// `CachedPlayer::saved_status_rows` gives.
///
/// Walking the name map rather than the assigned table's own entries also fixes
/// the order, for the same reason `pal.work_suitability` does: `OrderedMap`'s
/// order is the order the frontend receives, and the order Lua's `pairs` hands
/// over is not an order at all.
fn replacement_status_points(
    name_map: &[(&str, &str)],
    saved_rows: &[String],
    assigned: &OrderedMap<String, i64>,
) -> OrderedMap<String, i64> {
    let mut ordered = OrderedMap::new();
    for (_, english) in name_map {
        let points = assigned.get(*english).copied().unwrap_or(0);
        if points != 0 || saved_rows.iter().any(|saved| saved == *english) {
            ordered.insert((*english).to_string(), points);
        }
    }
    ordered
}

fn validate_status_point_list(_dto: &PlayerDto, value: &FieldValue) -> Result<(), HostError> {
    validate_status_points("status_point_list", &STATUS_NAME_MAP, value)
}
/// Takes the map already expanded to a genuine replacement by
/// `replacement_status_points`, which needs the save's own key set and so
/// cannot run from here.
fn apply_status_point_list(dto: &mut PlayerDto, value: FieldValue) {
    if let FieldValue::Map(entries) = value {
        dto.status_point_list = entries;
    }
}

fn validate_ext_status_point_list(_dto: &PlayerDto, value: &FieldValue) -> Result<(), HostError> {
    validate_status_points("ext_status_point_list", &EX_STATUS_NAME_MAP, value)
}
fn apply_ext_status_point_list(dto: &mut PlayerDto, value: FieldValue) {
    if let FieldValue::Map(entries) = value {
        dto.ext_status_point_list = entries;
    }
}

const fn rw(
    name: &'static str,
    ty: ApiType,
    doc: &'static str,
    read: fn(&PlayerDto) -> FieldValue,
    validate: fn(&PlayerDto, &FieldValue) -> Result<(), HostError>,
    apply: fn(&mut PlayerDto, FieldValue),
) -> FieldSpec<PlayerDto, PlayerSummary> {
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
    read: fn(&PlayerDto) -> FieldValue,
) -> FieldSpec<PlayerDto, PlayerSummary> {
    FieldSpec { name, ty, access: Access::ReadOnly, doc, read: Reader::Dto(read), write: None }
}

/// Like `ro`, but sourced from the player summary the session already holds.
/// The distinction is not cosmetic here: a `Dto` row costs a lazy load of that
/// player's own `.sav` from disk the first time it is read, and a summary row
/// costs nothing.
const fn ro_summary(
    name: &'static str,
    ty: ApiType,
    doc: &'static str,
    read: fn(&PlayerSummary) -> FieldValue,
) -> FieldSpec<PlayerDto, PlayerSummary> {
    FieldSpec { name, ty, access: Access::ReadOnly, doc, read: Reader::Summary(read), write: None }
}

/// Every field this handle answers for. The five container DTOs
/// (`common_container` and friends), `pals`, `pal_box`, `party`,
/// `collected_relics`, `location` and `dps` are deliberately absent: each is a
/// nested structure rather than a scalar or a flat collection, and the five
/// item containers in particular are nulled before any write reaches the save
/// (see `dto_cache::player_read`).
pub const PLAYER_FIELDS: &[FieldSpec<PlayerDto, PlayerSummary>] = &[
    ro_summary("uid", ApiType::String, "The player's UUID, as a string. Read-only.", read_uid),
    rw(
        "name",
        ApiType::String,
        "The player's nickname. Neither an empty string nor the save's own placeholder for a \
         nameless player can be assigned: both would read back as something other than what was \
         written.",
        read_name,
        validate_name,
        apply_name,
    ),
    rw(
        "level",
        ApiType::Union(&[ApiType::Integer, ApiType::Nil]),
        "The player's level, 1-255, or nil for a player the save records no level for at all. \
         Assigning nil raises rather than clearing it: the save has no way to record a player \
         with no level, so only an integer can be written.",
        read_level,
        validate_level,
        apply_level,
    ),
    ro_summary(
        "guild_id",
        ApiType::Union(&[ApiType::String, ApiType::Nil]),
        "The UUID, as a string, of the guild this player belongs to, or nil if the player is in \
         no guild. Read-only.",
        read_guild_id,
    ),
    ro_summary(
        "pal_count",
        ApiType::Integer,
        "How many pals this player owns. Read-only: it is derived by counting, not stored.",
        read_pal_count,
    ),
    ro_summary(
        "last_online",
        ApiType::Union(&[ApiType::String, ApiType::Nil]),
        "An ISO-8601 timestamp of when the player was last online, or nil if the save records \
         none. Read-only.",
        read_last_online,
    ),
    ro_summary(
        "last_online_ts",
        ApiType::Union(&[ApiType::Integer, ApiType::Nil]),
        "The Unix timestamp, in seconds, of when the player was last online, or nil if the save \
         records none. Read-only.",
        read_last_online_ts,
    ),
    ro(
        "instance_id",
        ApiType::Union(&[ApiType::String, ApiType::Nil]),
        "The id of this player's own character entry, or nil if the save records none. \
         Read-only.",
        read_instance_id,
    ),
    rw("exp", ApiType::Integer, "The player's experience points.", read_exp, validate_exp, apply_exp),
    rw(
        "hp",
        ApiType::Integer,
        "The player's current HP. Unlike a pal's, this is written through as given rather than \
         recalculated when the save is written.",
        read_hp,
        validate_hp,
        apply_hp,
    ),
    rw(
        "stomach",
        ApiType::Number,
        "The player's current fullness. Stored as a 32-bit float, so a value outside that range \
         is refused rather than written as an infinity.",
        read_stomach,
        validate_stomach,
        apply_stomach,
    ),
    rw(
        "sanity",
        ApiType::Number,
        "The player's current sanity. Stored as a 32-bit float, so a value outside that range \
         is refused rather than written as an infinity.",
        read_sanity,
        validate_sanity,
        apply_sanity,
    ),
    rw(
        "technology_points",
        ApiType::Integer,
        "Unspent technology points.",
        read_technology_points,
        validate_technology_points,
        apply_technology_points,
    ),
    rw(
        "boss_technology_points",
        ApiType::Integer,
        "Unspent ancient technology points.",
        read_boss_technology_points,
        validate_boss_technology_points,
        apply_boss_technology_points,
    ),
    rw(
        "technologies",
        ApiType::List(&ApiType::String),
        "The technologies this player has unlocked, as recipe names. Assigning replaces the \
         whole list, and any string is accepted: nothing checks a name against the game's own \
         technology list, here or on the way into the save. The read returns a fresh table each \
         time, so changing that table changes nothing.",
        read_technologies,
        validate_technologies,
        apply_technologies,
    ),
    rw(
        "completed_missions",
        ApiType::List(&ApiType::String),
        "The quests this player has completed, as quest names. Assigning replaces the whole \
         list, and any string is accepted: nothing checks a name against the game's own quest \
         list, here or on the way into the save.",
        read_completed_missions,
        validate_completed_missions,
        apply_completed_missions,
    ),
    rw(
        "current_missions",
        ApiType::List(&ApiType::String),
        "The quests this player has in progress, as quest names. Assigning replaces the whole \
         list; each name becomes a fresh quest entry with no progress recorded against it.",
        read_current_missions,
        validate_current_missions,
        apply_current_missions,
    ),
    rw(
        "unlocked_fast_travel_points",
        ApiType::List(&ApiType::String),
        "The fast-travel points this player has unlocked, as flag keys. Assigning replaces the \
         whole set.",
        read_unlocked_fast_travel_points,
        validate_unlocked_fast_travel_points,
        apply_unlocked_fast_travel_points,
    ),
    rw(
        "collected_effigies",
        ApiType::List(&ApiType::String),
        "The Lifmunk effigies this player has collected, as flag keys. Assigning replaces the \
         whole set, and moves effigy_possess_num by the number of keys newly collected minus \
         the number un-collected, never below zero -- so it counts unspent effigies, not \
         collected ones, and un-collecting more than are unspent leaves it at zero rather \
         than going negative.",
        read_collected_effigies,
        validate_collected_effigies,
        apply_collected_effigies,
    ),
    ro(
        "effigy_possess_num",
        ApiType::Integer,
        "How many unspent Lifmunk effigies this player holds -- not how many they have \
         collected, since spending one does not un-collect it. Read-only in itself: it moves \
         when collected_effigies is assigned, and only then.",
        read_effigy_possess_num,
    ),
    ro(
        "defeated_bosses",
        ApiType::List(&ApiType::String),
        "The bosses this player has defeated, as flag keys, with the tower bosses merged in. \
         Read-only.",
        read_defeated_bosses,
    ),
    rw(
        "status_point_list",
        ApiType::Map { key: &ApiType::String, value: &ApiType::Integer },
        "Points spent on each base stat, keyed by max_hp, max_sp, attack, weight, capture_rate, \
         work_speed, hunger_reduction, swim_speed, food_decay_reduction, jump_power, \
         glider_speed, climb_speed, status_ailment_resist, exp_bonus, rainbow_passive_rate, \
         move_speed, sphere_homing and stamina_reduction. Assigning replaces the whole map: a \
         key you leave out is set to zero, which is the only way the save can express \"no \
         points spent\" -- there is no way to remove a stat once the save carries one. A key \
         the save has never carried and that you leave out (or assign zero) stays absent and \
         reads back nil. A key this map does not know is refused rather than silently dropped.",
        read_status_points,
        validate_status_point_list,
        apply_status_point_list,
    ),
    rw(
        "ext_status_point_list",
        ApiType::Map { key: &ApiType::String, value: &ApiType::Integer },
        "Points spent on each extended stat, keyed by max_hp, max_sp, attack, weight and \
         work_speed -- the base-stat keys minus capture_rate, which the extended list has no \
         entry for. Assigning replaces the whole map on the same terms as status_point_list, \
         and a key it does not know is refused.",
        read_ext_status_points,
        validate_ext_status_point_list,
        apply_ext_status_point_list,
    ),
    ro(
        "pal_box_id",
        ApiType::Union(&[ApiType::String, ApiType::Nil]),
        "The id of this player's pal box container, or nil if the save records none. Read-only.",
        read_pal_box_id,
    ),
    ro(
        "otomo_container_id",
        ApiType::Union(&[ApiType::String, ApiType::Nil]),
        "The id of this player's party container, or nil if the save records none. Read-only.",
        read_otomo_container_id,
    ),
];

/// The two rows the player summary answers on this handle's behalf, kept here
/// rather than in `save_read.rs` so the shortcut and the capability rule below
/// cannot disagree about which rows they are. Both are assignable, so their
/// rows read the cached `PlayerDto` for a value written this run -- the cheap,
/// no-load path for an unwritten one is `player_field`'s, and this names it.
pub(crate) const SUMMARY_SHORTCUT_FIELDS: &[&str] = &["name", "level"];

/// Whether reading `field` needs the `players` capability on top of
/// `save.read`.
///
/// A `save.read` grant reached seven player rows before this handle gained
/// writable fields, all of them answered from the `PlayerSummary` the session
/// already holds. Every other row is served by reading that player's own
/// `PlayerDto`, which before this task was reachable only through `raw`'s
/// `player:<uid>` target -- gated on `players` on top of `save.raw` -- or from
/// the character-map entry, which needed `save.raw`. Serving them to a plugin
/// holding `save.read` alone would widen what that grant means to the person
/// giving it, which is a consent boundary rather than a performance one.
///
/// Derived from the table rather than listed, so a row added later is gated by
/// default: opting one out means giving it a `Reader::Summary`, which is also
/// the thing that makes it free to read.
pub(crate) fn read_requires_players(field: &str) -> bool {
    if SUMMARY_SHORTCUT_FIELDS.contains(&field) {
        return false;
    }
    PLAYER_FIELDS.iter().any(|spec| spec.name == field && matches!(spec.read, Reader::Dto(_)))
}

static API_FIELDS: OnceLock<Vec<ApiField>> = OnceLock::new();

/// The capability note is appended from `read_requires_players` rather than
/// written into each row's own doc, so the published description and the gate
/// cannot drift: a row that starts or stops needing `players` says so in
/// `psp.lua` the moment it does. The leak is bounded and one-shot -- one string
/// per gated row, built once inside this `OnceLock` -- and buys an `ApiField`
/// that still borrows for `'static` without widening the published model.
fn published_doc(spec: &'static FieldSpec<PlayerDto, PlayerSummary>) -> &'static str {
    if !read_requires_players(spec.name) {
        return spec.doc;
    }
    Box::leak(format!("{} Reading it requires the players capability.", spec.doc).into_boxed_str())
}

/// The published description of this handle's fields, projected from the same
/// rows that answer the reads and validate the writes.
pub(crate) fn api_fields() -> &'static [ApiField] {
    API_FIELDS
        .get_or_init(|| {
            PLAYER_FIELDS
                .iter()
                .map(|spec| ApiField {
                    name: spec.name,
                    ty: spec.ty.clone(),
                    access: spec.access,
                    doc: published_doc(spec),
                })
                .collect()
        })
        .as_slice()
}

fn find(field: &str) -> Option<&'static FieldSpec<PlayerDto, PlayerSummary>> {
    PLAYER_FIELDS.iter().find(|spec| spec.name == field)
}

/// Reads one field, from the cached `PlayerDto` or from the player summary
/// depending on the row's `Reader`. An unrecognized field name returns `Nil`,
/// matching how every other handle's read side already treats a name it does
/// not carry -- and, importantly, without paying for a load to find that out.
pub(crate) fn player_get(ctx: &mut RunContext<'_>, uid: Uuid, field: &str) -> Result<FieldValue, HostError> {
    // Ahead of `player_read`, which is the part that matters and the part a
    // test can see: a refused read must not have pulled the player's own save
    // off disk on its way to refusing.
    //
    // Placed ahead of `find` as well, to match how `player_set` checks
    // `save.write` -- but unlike the write path, that ordering is not
    // observable here and no test claims it is. An unknown name is never gated
    // (`read_requires_players` answers `false` for it) and reads as nil either
    // way, so nothing about the table leaks whichever side of `find` this sits.
    if read_requires_players(field) && !ctx.grants(Capability::Players) {
        return Err(HostError::new(format!(
            "reading player.{field} requires the players capability"
        )));
    }
    let Some(spec) = find(field) else {
        return Ok(FieldValue::Nil);
    };
    match spec.read {
        Reader::Dto(read) => {
            let dto = dto_cache::player_read(ctx, uid)?;
            Ok(read(dto))
        }
        Reader::Summary(read) => {
            Ok(ctx.session.player_summaries.get(&uid).map(read).unwrap_or(FieldValue::Nil))
        }
    }
}

/// `save.write` is checked before any field resolution -- before the name is
/// looked up, and so before `unknown player field` or `is read-only` can be
/// reported -- so an ungranted write is not told which fields exist or which of
/// them it could have written. It is also checked before `player_read`, which
/// would otherwise pull the player's `.sav` off disk on behalf of a plugin that
/// was never allowed to write it.
pub(crate) fn player_set(
    ctx: &mut RunContext<'_>,
    uid: Uuid,
    field: &str,
    value: FieldValue,
) -> Result<(), HostError> {
    if !ctx.grants(Capability::SaveWrite) {
        return Err(HostError::new("player field assignment requires the save.write capability"));
    }
    let Some(spec) = find(field) else {
        return Err(HostError::new(format!("unknown player field {field:?}")));
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
    let current = dto_cache::player_read(ctx, uid)?;
    (write.validate)(current, &value)?;
    // Validated against what the script actually assigned, so a refusal names
    // the script's own keys; expanded to a genuine replacement only afterwards.
    let value = match (spec.name, value) {
        ("status_point_list", FieldValue::Map(assigned)) => FieldValue::Map(replacement_status_points(
            &STATUS_NAME_MAP,
            dto_cache::player_saved_status_rows(ctx, uid, spec.name),
            &assigned,
        )),
        ("ext_status_point_list", FieldValue::Map(assigned)) => FieldValue::Map(replacement_status_points(
            &EX_STATUS_NAME_MAP,
            dto_cache::player_saved_status_rows(ctx, uid, spec.name),
            &assigned,
        )),
        (_, other) => other,
    };
    if ctx.dry_run {
        ctx.bump(&format!("player.{}", spec.name), 1);
    }
    let apply = write.apply;
    dto_cache::player_write(ctx, uid, &[spec.name], move |dto| apply(dto, value))
}

fn player_newindex(state: *mut lua_State) -> Result<c_int, HostError> {
    unsafe {
        check_args(state, 3, "player field assignment")?;
        let handle = read_handle(state, 1, HandleKind::Player)?;
        let field = arg_string(state, 2, "field")?;
        let value = read_field_value(state, 3, &field)?;
        with_context(state, |ctx| player_set(ctx, handle.id, &field, value))?;
        Ok(0)
    }
}

host_fn!(push_player_newindex, player_newindex);
