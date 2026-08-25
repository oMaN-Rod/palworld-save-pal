use std::ffi::c_int;
use std::sync::OnceLock;

use psp_core::dto::guild::GuildDto;
use psp_core::dto::summary::GuildSummary;
use psp_lua_sys::ffi::*;
use uuid::Uuid;

use super::{expect_int, expect_str, read_field_value, Access, FieldSpec, FieldValue, FieldWrite, Reader};
use crate::context::RunContext;
use crate::host::api_def::{ApiField, ApiType};
use crate::host::handle::{read_handle, HandleKind};
use crate::host::marshal::{arg_string, check_args};
use crate::host::{dto_cache, with_context, HostError};
use crate::host_fn;
use crate::manifest::Capability;

fn optional_uuid(id: Option<Uuid>) -> FieldValue {
    id.map(|id| FieldValue::Str(id.to_string())).unwrap_or(FieldValue::Nil)
}

// --- readers -------------------------------------------------------------

fn read_id(summary: &GuildSummary) -> FieldValue {
    FieldValue::Str(summary.id.to_string())
}
fn read_admin_uid(summary: &GuildSummary) -> FieldValue {
    optional_uuid(summary.admin_player_uid)
}
fn read_player_count(summary: &GuildSummary) -> FieldValue {
    FieldValue::Int(summary.player_count)
}
fn read_base_count(summary: &GuildSummary) -> FieldValue {
    FieldValue::Int(summary.base_count)
}
fn read_pal_count(summary: &GuildSummary) -> FieldValue {
    FieldValue::Int(summary.pal_count)
}

fn read_name(dto: &GuildDto) -> FieldValue {
    dto.name.clone().map(FieldValue::Str).unwrap_or(FieldValue::Nil)
}
/// `base_camp_level` is `Option` only because `GuildDto` is also a request
/// shape, where `None` means "leave it alone". `load_guild_dto` always fills
/// it from the guild tail and `apply_level` always leaves it filled, so the
/// row is declared a plain integer and the `None` arm is unreachable. A `nil`
/// escaping here would be caught by
/// `every_guild_field_row_reads_back_at_its_declared_type`.
fn read_level(dto: &GuildDto) -> FieldValue {
    dto.base_camp_level.map(|level| FieldValue::Int(i64::from(level))).unwrap_or(FieldValue::Nil)
}
fn read_chest_container_id(dto: &GuildDto) -> FieldValue {
    optional_uuid(dto.container_id)
}

// --- writers -------------------------------------------------------------

/// `apply_guild_dto` reads an empty name as "leave it alone" and skips the
/// write, so assigning one would report success and change nothing.
fn validate_name(_dto: &GuildDto, value: &FieldValue) -> Result<(), HostError> {
    let text = expect_str("name", value)?;
    if text.is_empty() {
        return Err(HostError::new(
            "name cannot be empty: the save's guild writer reads an empty name as \"leave the \
             name alone\", so the assignment would change nothing rather than clearing it",
        ));
    }
    Ok(())
}
fn apply_name(dto: &mut GuildDto, value: FieldValue) {
    if let FieldValue::Str(text) = value {
        dto.name = Some(text);
    }
}

/// Zero gets the same treatment from `apply_guild_dto` that an empty name
/// does -- it means "leave it alone" -- so it is refused rather than silently
/// dropped. The upper bound is the save's own: the guild tail holds the level
/// as an `i32`.
fn validate_level(_dto: &GuildDto, value: &FieldValue) -> Result<(), HostError> {
    let level = expect_int("level", value)?;
    if level == 0 {
        return Err(HostError::new(
            "level cannot be 0: the save's guild writer reads a zero as \"leave the level \
             alone\", so the assignment would change nothing",
        ));
    }
    if !(1..=i64::from(i32::MAX)).contains(&level) {
        return Err(HostError::new(format!("level must be between 1 and {}, got {level}", i32::MAX)));
    }
    Ok(())
}
fn apply_level(dto: &mut GuildDto, value: FieldValue) {
    if let FieldValue::Int(level) = value {
        // Validated in range already; `None` would read as "leave it alone"
        // rather than as a wrong number, which is the safe way to be wrong.
        dto.base_camp_level = i32::try_from(level).ok();
    }
}

const fn rw(
    name: &'static str,
    ty: ApiType,
    doc: &'static str,
    read: fn(&GuildDto) -> FieldValue,
    validate: fn(&GuildDto, &FieldValue) -> Result<(), HostError>,
    apply: fn(&mut GuildDto, FieldValue),
) -> FieldSpec<GuildDto, GuildSummary> {
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
    read: fn(&GuildDto) -> FieldValue,
) -> FieldSpec<GuildDto, GuildSummary> {
    FieldSpec { name, ty, access: Access::ReadOnly, doc, read: Reader::Dto(read), write: None }
}

/// Like `ro`, but sourced from the guild summary the session already holds
/// rather than from the cached DTO.
const fn ro_summary(
    name: &'static str,
    ty: ApiType,
    doc: &'static str,
    read: fn(&GuildSummary) -> FieldValue,
) -> FieldSpec<GuildDto, GuildSummary> {
    FieldSpec { name, ty, access: Access::ReadOnly, doc, read: Reader::Summary(read), write: None }
}

/// Every field this handle answers for. The three counts are derived by
/// counting rather than stored, so there is nothing on the guild for a write
/// to land on; `id` and `admin_uid` are identity. That leaves `name` and
/// `level` -- exactly the two `apply_guild_dto` writes into the guild tail.
///
/// `lab_research` has no row: it is a list of structures rather than a scalar
/// or a flat collection, and the save's own writer for it (`update_lab_research`)
/// is a different call than the one behind this handle.
pub const GUILD_FIELDS: &[FieldSpec<GuildDto, GuildSummary>] = &[
    ro_summary("id", ApiType::String, "The guild's UUID, as a string. Read-only.", read_id),
    rw(
        "name",
        ApiType::String,
        "The guild's name. An empty string cannot be assigned: the save reads one as \"leave \
         the name alone\", so the assignment would change nothing rather than clearing it.",
        read_name,
        validate_name,
        apply_name,
    ),
    ro_summary(
        "admin_uid",
        ApiType::Union(&[ApiType::String, ApiType::Nil]),
        "The UUID, as a string, of the guild's admin player, or nil if the guild has none. \
         Read-only.",
        read_admin_uid,
    ),
    ro_summary(
        "player_count",
        ApiType::Integer,
        "How many players belong to this guild. Read-only: it is derived by counting, not \
         stored.",
        read_player_count,
    ),
    ro_summary(
        "base_count",
        ApiType::Integer,
        "How many bases this guild has. Read-only: it is derived by counting, not stored.",
        read_base_count,
    ),
    rw(
        "level",
        ApiType::Integer,
        "The guild's base-camp level. Never nil: the guild tail stores it as a plain integer \
         with no way to record its absence, so a guild that has a handle at all has a level. \
         Zero cannot be assigned: the save reads one as \"leave the level alone\", so the \
         assignment would change nothing. Assigning nil raises for the same reason.",
        read_level,
        validate_level,
        apply_level,
    ),
    ro_summary(
        "pal_count",
        ApiType::Integer,
        "How many pals belong to this guild's bases. Read-only: it is derived by counting, not \
         stored.",
        read_pal_count,
    ),
    ro(
        "chest_container_id",
        ApiType::Union(&[ApiType::String, ApiType::Nil]),
        "The UUID, as a string, of this guild's shared chest container, or nil if the guild has \
         no chest. Read-only: it is resolved from the save itself, so a plugin cannot redirect \
         a chest edit by assigning a different id.",
        read_chest_container_id,
    ),
];

static API_FIELDS: OnceLock<Vec<ApiField>> = OnceLock::new();

/// The published description of this handle's fields, projected from the same
/// rows that answer the reads and validate the writes.
pub(crate) fn api_fields() -> &'static [ApiField] {
    API_FIELDS
        .get_or_init(|| {
            GUILD_FIELDS
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

fn find(field: &str) -> Option<&'static FieldSpec<GuildDto, GuildSummary>> {
    GUILD_FIELDS.iter().find(|spec| spec.name == field)
}

/// Reads one field, from the cached `GuildDto` or from the guild summary
/// depending on the row's `Reader`. An unrecognized field name returns `Nil`,
/// matching how every other handle's read side already treats a name it does
/// not carry.
///
/// The cached DTO is the only source for the two writable rows, in both
/// directions: it is loaded from the guild tail in the save, so an unwritten
/// read answers what the save holds and a read after a flush answers what the
/// flush actually put there -- not what this run believes it wrote.
pub(crate) fn guild_get(ctx: &mut RunContext<'_>, id: Uuid, field: &str) -> Result<FieldValue, HostError> {
    let Some(spec) = find(field) else {
        return Ok(FieldValue::Nil);
    };
    match spec.read {
        Reader::Dto(read) => {
            let dto = dto_cache::guild_read(ctx, id)?;
            Ok(read(dto))
        }
        Reader::Summary(read) => {
            Ok(ctx.session.guild_summaries.get(&id).map(read).unwrap_or(FieldValue::Nil))
        }
    }
}

/// `save.write` is checked before any field resolution -- before the name is
/// looked up, and so before `unknown guild field` or `is read-only` can be
/// reported -- so an ungranted write is not told which fields exist or which of
/// them it could have written.
pub(crate) fn guild_set(
    ctx: &mut RunContext<'_>,
    id: Uuid,
    field: &str,
    value: FieldValue,
) -> Result<(), HostError> {
    if !ctx.grants(Capability::SaveWrite) {
        return Err(HostError::new("guild field assignment requires the save.write capability"));
    }
    let Some(spec) = find(field) else {
        return Err(HostError::new(format!("unknown guild field {field:?}")));
    };
    let Some(write) = spec.write.as_ref() else {
        return Err(HostError::new(format!("{field} is read-only")));
    };
    let current = dto_cache::guild_read(ctx, id)?;
    (write.validate)(current, &value)?;
    if ctx.dry_run {
        ctx.bump(&format!("guild.{}", spec.name), 1);
    }
    let apply = write.apply;
    dto_cache::guild_write(ctx, id, &[spec.name], move |dto| apply(dto, value))
}

fn guild_newindex(state: *mut lua_State) -> Result<c_int, HostError> {
    unsafe {
        check_args(state, 3, "guild field assignment")?;
        let handle = read_handle(state, 1, HandleKind::Guild)?;
        let field = arg_string(state, 2, "field")?;
        let value = read_field_value(state, 3, &field)?;
        with_context(state, |ctx| guild_set(ctx, handle.id, &field, value))?;
        Ok(0)
    }
}

host_fn!(push_guild_newindex, guild_newindex);
