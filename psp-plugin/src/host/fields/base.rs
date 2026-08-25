use std::ffi::c_int;
use std::sync::OnceLock;

use psp_core::domain::guild::{base_camp_location, base_guild_and_container};
use psp_core::dto::guild::BaseDto;
use psp_core::props;
use psp_core::ue::MapEntry;
use psp_lua_sys::ffi::*;
use uuid::Uuid;

use super::{
    expect_finite_f32, expect_str, read_field_value, Access, FieldSpec, FieldValue, FieldWrite, Reader,
};
use crate::context::RunContext;
use crate::host::api_def::{ApiField, ApiType};
use crate::host::handle::{read_handle, HandleKind};
use crate::host::marshal::{arg_string, check_args};
use crate::host::{dto_cache, with_context, HostError};
use crate::host_fn;
use crate::manifest::Capability;

// --- readers -------------------------------------------------------------

fn read_id(entry: &MapEntry) -> FieldValue {
    props::as_uuid(&entry.key).map(|id| FieldValue::Str(id.to_string())).unwrap_or(FieldValue::Nil)
}
fn read_guild_id(entry: &MapEntry) -> FieldValue {
    base_guild_and_container(entry)
        .map(|(guild_id, _)| FieldValue::Str(guild_id.to_string()))
        .unwrap_or(FieldValue::Nil)
}
fn read_x(entry: &MapEntry) -> FieldValue {
    base_camp_location(entry).map(|(x, _, _)| FieldValue::Float(x)).unwrap_or(FieldValue::Nil)
}
fn read_y(entry: &MapEntry) -> FieldValue {
    base_camp_location(entry).map(|(_, y, _)| FieldValue::Float(y)).unwrap_or(FieldValue::Nil)
}
fn read_z(entry: &MapEntry) -> FieldValue {
    base_camp_location(entry).map(|(_, _, z)| FieldValue::Float(z)).unwrap_or(FieldValue::Nil)
}

fn read_name(dto: &BaseDto) -> FieldValue {
    dto.name.clone().map(FieldValue::Str).unwrap_or(FieldValue::Nil)
}
fn read_area_range(dto: &BaseDto) -> FieldValue {
    dto.area_range.map(FieldValue::Float).unwrap_or(FieldValue::Nil)
}

// --- writers -------------------------------------------------------------

/// `apply_base_dto` reads an empty name as "leave it alone" and skips the
/// write, so assigning one would report success and change nothing.
fn validate_name(_dto: &BaseDto, value: &FieldValue) -> Result<(), HostError> {
    let text = expect_str("name", value)?;
    if text.is_empty() {
        return Err(HostError::new(
            "name cannot be empty: the save's base writer reads an empty name as \"leave the \
             name alone\", so the assignment would change nothing rather than clearing it",
        ));
    }
    Ok(())
}
fn apply_name(dto: &mut BaseDto, value: FieldValue) {
    if let FieldValue::Str(text) = value {
        dto.name = Some(text);
    }
}

fn validate_area_range(_dto: &BaseDto, value: &FieldValue) -> Result<(), HostError> {
    expect_finite_f32("area_range", value).map(|_| ())
}

/// Stored back through the same `as f32` narrowing `apply_base_dto` performs,
/// so the value this run reads out of the cache is the value the save ends up
/// holding rather than the wider one that was assigned. Without it a read
/// before the flush and a read after it would disagree for any number `f32`
/// cannot represent exactly.
fn apply_area_range(dto: &mut BaseDto, value: FieldValue) {
    let number = match value {
        FieldValue::Float(v) => v,
        FieldValue::Int(v) => v as f64,
        _ => return,
    };
    dto.area_range = Some(f64::from(number as f32));
}

const fn rw(
    name: &'static str,
    ty: ApiType,
    doc: &'static str,
    read: fn(&BaseDto) -> FieldValue,
    validate: fn(&BaseDto, &FieldValue) -> Result<(), HostError>,
    apply: fn(&mut BaseDto, FieldValue),
) -> FieldSpec<BaseDto, MapEntry> {
    FieldSpec {
        name,
        ty,
        access: Access::ReadWrite,
        doc,
        read: Reader::Dto(read),
        write: Some(FieldWrite { validate, apply }),
    }
}

/// Read-only and sourced from the base's own `BaseCampSaveData` entry, which
/// the session already holds -- the base handle has no summary of its own, so
/// the entry plays that part.
const fn ro_entry(
    name: &'static str,
    ty: ApiType,
    doc: &'static str,
    read: fn(&MapEntry) -> FieldValue,
) -> FieldSpec<BaseDto, MapEntry> {
    FieldSpec { name, ty, access: Access::ReadOnly, doc, read: Reader::Summary(read), write: None }
}

/// Every field this handle answers for. Every row but `id` admits nil, because
/// `bases_next` hands out a handle for any uuid-keyed `BaseCampSaveData` entry
/// without checking that its `RawData` is a base-camp record at all; on such an
/// entry the key is the only thing left to read. `id` and `guild_id` are identity, and
/// `x`/`y`/`z` are read-only for a blunter reason: nothing in this repository
/// writes a base's `transform.translation`, `BaseDto::location` is marked
/// output-only, and `apply_base_dto` never looks at it. There is no write path
/// to expose, so they are honest read-only rows rather than setters that
/// cannot be implemented.
///
/// Moving a base is also not merely a missing setter. A base's placed
/// structures and its working pals carry their own world coordinates, and
/// nothing in the save's own writers relates those to the base's; a base moved
/// without them would leave both behind. Which of the two a "move" should mean
/// is unsettled here and not settled by this table.
pub const BASE_FIELDS: &[FieldSpec<BaseDto, MapEntry>] = &[
    ro_entry("id", ApiType::String, "The base's UUID, as a string. Read-only.", read_id),
    ro_entry(
        "guild_id",
        ApiType::Union(&[ApiType::String, ApiType::Nil]),
        "The UUID, as a string, of the guild this base belongs to, or nil if it could not be \
         resolved. Read-only.",
        read_guild_id,
    ),
    rw(
        "name",
        ApiType::Union(&[ApiType::String, ApiType::Nil]),
        "The base's name, or nil if the save holds no base camp record for this base -- the \
         same case x, y and z read nil for. Newly built bases carry a generated template name \
         rather than one the player chose. An empty string cannot be assigned: the save reads \
         one as \"leave the name alone\", so the assignment would change nothing rather than \
         clearing it. Assigning nil raises: the nil is an answer about the save's record, not \
         a value that can be written.",
        read_name,
        validate_name,
        apply_name,
    ),
    rw(
        "area_range",
        ApiType::Union(&[ApiType::Number, ApiType::Nil]),
        "The radius, in world units, of the base's working area, or nil in the same case name \
         reads nil. Stored as a 32-bit float, so a value outside that range is refused rather \
         than written as an infinity, and one that range cannot hold exactly reads back \
         rounded to what the save will actually hold. No other bound is enforced: zero and \
         negative radii are accepted and written as given, because nothing in the game's data \
         or in this app establishes what a legal radius is, and refusing them here would be \
         inventing a rule rather than reporting one.",
        read_area_range,
        validate_area_range,
        apply_area_range,
    ),
    ro_entry(
        "x",
        ApiType::Union(&[ApiType::Number, ApiType::Nil]),
        "The base's world X coordinate, or nil if its location could not be resolved. \
         Read-only: nothing in this app writes a base's position, so there is no write path to \
         offer.",
        read_x,
    ),
    ro_entry(
        "y",
        ApiType::Union(&[ApiType::Number, ApiType::Nil]),
        "The base's world Y coordinate, or nil if its location could not be resolved. \
         Read-only, for the same reason as x.",
        read_y,
    ),
    ro_entry(
        "z",
        ApiType::Union(&[ApiType::Number, ApiType::Nil]),
        "The base's world Z coordinate, or nil if its location could not be resolved. \
         Read-only, for the same reason as x.",
        read_z,
    ),
];

static API_FIELDS: OnceLock<Vec<ApiField>> = OnceLock::new();

/// The published description of this handle's fields, projected from the same
/// rows that answer the reads and validate the writes.
pub(crate) fn api_fields() -> &'static [ApiField] {
    API_FIELDS
        .get_or_init(|| {
            BASE_FIELDS
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

fn find(field: &str) -> Option<&'static FieldSpec<BaseDto, MapEntry>> {
    BASE_FIELDS.iter().find(|spec| spec.name == field)
}

/// Reads one field, from the cached `BaseDto` or straight off the base's
/// `BaseCampSaveData` entry depending on the row's `Reader`. An unrecognized
/// field name returns `Nil`, matching how every other handle's read side
/// already treats a name it does not carry.
///
/// The cached DTO is loaded from that same entry, so an unwritten read of a
/// writable row answers what the save holds and a read after a flush answers
/// what the flush actually put there.
pub(crate) fn base_get(ctx: &mut RunContext<'_>, id: Uuid, field: &str) -> Result<FieldValue, HostError> {
    let Some(spec) = find(field) else {
        return Ok(FieldValue::Nil);
    };
    match spec.read {
        Reader::Dto(read) => {
            let dto = dto_cache::base_read(ctx, id)?;
            Ok(read(dto))
        }
        Reader::Summary(read) => {
            let entries = ctx.session.base_camp_map().unwrap_or(&[]);
            Ok(entries
                .iter()
                .find(|entry| props::as_uuid(&entry.key) == Some(id))
                .map(read)
                .unwrap_or(FieldValue::Nil))
        }
    }
}

/// `save.write` is checked before any field resolution -- before the name is
/// looked up, and so before `unknown base field` or `is read-only` can be
/// reported -- so an ungranted write is not told which fields exist or which of
/// them it could have written.
pub(crate) fn base_set(
    ctx: &mut RunContext<'_>,
    id: Uuid,
    field: &str,
    value: FieldValue,
) -> Result<(), HostError> {
    if !ctx.grants(Capability::SaveWrite) {
        return Err(HostError::new("base field assignment requires the save.write capability"));
    }
    let Some(spec) = find(field) else {
        return Err(HostError::new(format!("unknown base field {field:?}")));
    };
    let Some(write) = spec.write.as_ref() else {
        return Err(HostError::new(format!("{field} is read-only")));
    };
    // Refused rather than applied. `apply_base_dto` writes both writable rows
    // into the entry's base-camp record and simply does nothing when there is
    // none, so the assignment would report success, read back inside the run
    // out of the cache, and then read nil again once the flush had "written"
    // it. This is the write-side half of the read degrading to nil.
    if !dto_cache::base_camp_record_exists(ctx, id) {
        return Err(HostError::new(format!(
            "{field} cannot be written: the save holds no base camp record for this base, so \
             the assignment would change nothing"
        )));
    }
    let current = dto_cache::base_read(ctx, id)?;
    (write.validate)(current, &value)?;
    if ctx.dry_run {
        ctx.bump(&format!("base.{}", spec.name), 1);
    }
    let apply = write.apply;
    dto_cache::base_write(ctx, id, move |dto| apply(dto, value))
}

fn base_newindex(state: *mut lua_State) -> Result<c_int, HostError> {
    unsafe {
        check_args(state, 3, "base field assignment")?;
        let handle = read_handle(state, 1, HandleKind::Base)?;
        let field = arg_string(state, 2, "field")?;
        let value = read_field_value(state, 3, &field)?;
        with_context(state, |ctx| base_set(ctx, handle.id, &field, value))?;
        Ok(0)
    }
}

host_fn!(push_base_newindex, base_newindex);
