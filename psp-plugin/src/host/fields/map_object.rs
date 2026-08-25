use std::ffi::c_int;
use std::sync::OnceLock;

use psp_core::domain::map_object::{set_map_object_builder, set_map_object_hp, MapObjectView};
use psp_lua_sys::ffi::*;
use uuid::Uuid;

use super::{
    field_value_type_name, ranged_int, read_field_value, Access, FieldSpec, FieldValue, FieldWrite,
    Reader,
};
use crate::context::RunContext;
use crate::host::api_def::{ApiField, ApiType};
use crate::host::handle::{read_handle, HandleKind};
use crate::host::marshal::{arg_string, check_args};
use crate::host::{with_context, HostError};
use crate::host_fn;
use crate::manifest::Capability;

// --- readers -------------------------------------------------------------

fn optional_uuid(id: Option<Uuid>) -> FieldValue {
    id.map(|id| FieldValue::Str(id.to_string())).unwrap_or(FieldValue::Nil)
}

fn read_id(view: &MapObjectView) -> FieldValue {
    FieldValue::Str(view.map_object_id.clone())
}
fn read_instance_id(view: &MapObjectView) -> FieldValue {
    FieldValue::Str(view.instance_id.to_string())
}
fn read_base_id(view: &MapObjectView) -> FieldValue {
    optional_uuid(view.base_id)
}
fn read_guild_id(view: &MapObjectView) -> FieldValue {
    optional_uuid(view.guild_id)
}
fn read_build_player_uid(view: &MapObjectView) -> FieldValue {
    optional_uuid(view.build_player_uid)
}
fn read_kind(view: &MapObjectView) -> FieldValue {
    FieldValue::Str(view.kind.clone())
}
fn read_hp(view: &MapObjectView) -> FieldValue {
    FieldValue::Int(i64::from(view.hp))
}
fn read_max_hp(view: &MapObjectView) -> FieldValue {
    FieldValue::Int(i64::from(view.max_hp))
}

// --- writers -------------------------------------------------------------

/// No upper or lower bound beyond what an `i32` can hold: a plugin lowering a
/// structure's hp, including to zero or negative, is a legitimate write, and
/// clamping it to `max_hp` here would make that write look like it succeeded
/// while silently doing something else.
fn validate_hp(_current: &MapObjectView, value: &FieldValue) -> Result<(), HostError> {
    ranged_int("hp", value, i64::from(i32::MIN), i64::from(i32::MAX)).map(|_| ())
}
fn apply_hp(view: &mut MapObjectView, value: FieldValue) {
    if let FieldValue::Int(v) = value {
        if let Ok(v) = i32::try_from(v) {
            view.hp = v;
        }
    }
}

fn validate_build_player_uid(_current: &MapObjectView, value: &FieldValue) -> Result<(), HostError> {
    match value {
        FieldValue::Nil => Ok(()),
        FieldValue::Str(text) => Uuid::parse_str(text).map(|_| ()).map_err(|_| {
            HostError::new(format!("build_player_uid must be a uuid or nil, got {text:?}"))
        }),
        other => Err(HostError::new(format!(
            "build_player_uid must be a string uuid or nil, got {}",
            field_value_type_name(other)
        ))),
    }
}
fn apply_build_player_uid(view: &mut MapObjectView, value: FieldValue) {
    view.build_player_uid = match value {
        FieldValue::Nil => None,
        FieldValue::Str(text) => Uuid::parse_str(&text).ok(),
        _ => view.build_player_uid,
    };
}

const fn rw(
    name: &'static str,
    ty: ApiType,
    doc: &'static str,
    read: fn(&MapObjectView) -> FieldValue,
    validate: fn(&MapObjectView, &FieldValue) -> Result<(), HostError>,
    apply: fn(&mut MapObjectView, FieldValue),
) -> FieldSpec<MapObjectView, ()> {
    FieldSpec {
        name,
        ty,
        access: Access::ReadWrite,
        doc,
        read: Reader::Dto(read),
        instead_call: None,
        game_data_precheck: None,
        game_data_postcheck: None,
        write: Some(FieldWrite { validate, apply }),
    }
}

const fn ro(
    name: &'static str,
    ty: ApiType,
    doc: &'static str,
    read: fn(&MapObjectView) -> FieldValue,
) -> FieldSpec<MapObjectView, ()> {
    FieldSpec {
        name,
        ty,
        access: Access::ReadOnly,
        doc,
        read: Reader::Dto(read),
        instead_call: None,
        game_data_precheck: None,
        game_data_postcheck: None,
        write: None,
    }
}

/// Every field this handle answers for. `id` is the `MapObjectId` asset name
/// two objects of the same kind share; `instance_id` is what tells them apart,
/// and the only thing a `save.map_objects():delete_where(...)` predicate is
/// handed. `hp` and `build_player_uid` are the writable rows -- the save
/// writes both in place, so they cost no structural change and no epoch bump.
pub const MAP_OBJECT_FIELDS: &[FieldSpec<MapObjectView, ()>] = &[
    ro(
        "id",
        ApiType::String,
        "The MapObjectId asset name, shared by every instance of this kind. Read-only.",
        read_id,
    ),
    ro(
        "instance_id",
        ApiType::String,
        "This instance's UUID, as a string -- unique even among map objects that share an id. \
         Read-only.",
        read_instance_id,
    ),
    ro(
        "base_id",
        ApiType::Union(&[ApiType::String, ApiType::Nil]),
        "The UUID, as a string, of the base this object belongs to, or nil if it is unattached. \
         Read-only.",
        read_base_id,
    ),
    ro(
        "guild_id",
        ApiType::Union(&[ApiType::String, ApiType::Nil]),
        "The UUID, as a string, of the guild this object belongs to, or nil if it has none. \
         Read-only.",
        read_guild_id,
    ),
    rw(
        "build_player_uid",
        ApiType::Union(&[ApiType::String, ApiType::Nil]),
        "The UUID, as a string, of the player who built this object, or nil if it has none. \
         Assigning nil clears it; assigning a uuid string sets it, without checking that the \
         uuid names a player who exists.",
        read_build_player_uid,
        validate_build_player_uid,
        apply_build_player_uid,
    ),
    rw(
        "hp",
        ApiType::Integer,
        "This object's current hit points. Any 32-bit integer is accepted, including zero or a \
         negative value -- lowering a structure's hp is a legitimate write, and it is not \
         clamped to max_hp.",
        read_hp,
        validate_hp,
        apply_hp,
    ),
    ro(
        "max_hp",
        ApiType::Integer,
        "This object's maximum hit points. Read-only.",
        read_max_hp,
    ),
    ro(
        "kind",
        ApiType::String,
        "The concrete model type name this object was built from. Read-only.",
        read_kind,
    ),
];

static API_FIELDS: OnceLock<Vec<ApiField>> = OnceLock::new();

/// The published description of this handle's fields, projected from the same
/// rows that answer the reads and validate the writes.
pub(crate) fn api_fields() -> &'static [ApiField] {
    API_FIELDS
        .get_or_init(|| {
            MAP_OBJECT_FIELDS
                .iter()
                .map(|spec| ApiField {
                    name: spec.name,
                    ty: spec.ty,
                    access: spec.access,
                    doc: spec.doc,
                })
                .collect()
        })
        .as_slice()
}

fn find(field: &str) -> Option<&'static FieldSpec<MapObjectView, ()>> {
    MAP_OBJECT_FIELDS.iter().find(|spec| spec.name == field)
}

/// Reads one field off the run's map-object snapshot -- built once and
/// indexed by id, since the save has no cheaper way to resolve one instance
/// among 5000-odd positional entries than walking all of them. An unrecognized
/// field name, or an instance the snapshot does not carry, returns `Nil`,
/// matching every other handle's read side.
pub(crate) fn map_object_get(
    ctx: &mut RunContext<'_>,
    id: Uuid,
    field: &str,
) -> Result<FieldValue, HostError> {
    let Some(spec) = find(field) else {
        return Ok(FieldValue::Nil);
    };
    let read = match spec.read {
        Reader::Dto(read) => read,
        Reader::Summary(read) => return Ok(read(&())),
    };
    crate::host::save_read::ensure_map_objects_snapshot(ctx)?;
    let value = ctx
        .map_objects
        .as_ref()
        .and_then(|(views, index)| index.get(&id).and_then(|&position| views.get(position)))
        .map(read)
        .unwrap_or(FieldValue::Nil);
    Ok(value)
}

/// A write lands on the save at the point of assignment: `set_map_object_hp`
/// mutates hp in place, so nothing here can drift from what it did. The
/// snapshot's own copy of the row is updated in the same step rather than
/// dropped, so a read immediately afterwards -- in the same run -- sees the
/// write without paying to rebuild the whole snapshot. A dry run bumps the
/// count and stops short of both, which is what keeps it byte-identical to
/// the save it started from.
///
/// `save.write` is checked before any field resolution -- before the name is
/// looked up, and so before `unknown map_object field` or `is read-only` can
/// be reported -- so an ungranted write is not told which fields exist or
/// which of them it could have written.
pub(crate) fn map_object_set(
    ctx: &mut RunContext<'_>,
    id: Uuid,
    field: &str,
    value: FieldValue,
) -> Result<(), HostError> {
    if !ctx.grants(Capability::SaveWrite) {
        return Err(HostError::new("map_object field assignment requires the save.write capability"));
    }
    let Some(spec) = find(field) else {
        return Err(HostError::new(format!("unknown map_object field {field:?}")));
    };
    let Some(write) = spec.write.as_ref() else {
        return Err(spec.not_assignable());
    };
    let value = spec.coerce_empty_table(value);
    let game_data = ctx.game_data;
    crate::host::save_read::ensure_map_objects_snapshot(ctx)?;
    let Some((position, current)) = ctx.map_objects.as_ref().and_then(|(views, index)| {
        let position = *index.get(&id)?;
        Some((position, views.get(position)?.clone()))
    }) else {
        return Err(HostError::new(format!(
            "{field} cannot be written: map object {id} could not be resolved"
        )));
    };
    spec.validate_write(game_data, &current, &value)?;
    let mut next = current;
    (write.apply)(&mut next, value);
    if ctx.dry_run {
        ctx.bump(&format!("map_object.{}", spec.name), 1);
        return Ok(());
    }
    match spec.name {
        "hp" => {
            set_map_object_hp(ctx.session, id, next.hp)
                .map_err(|error| HostError::new(error.to_string()))?;
            if let Some((views, _)) = ctx.map_objects.as_mut() {
                if let Some(view) = views.get_mut(position) {
                    view.hp = next.hp;
                }
            }
        }
        "build_player_uid" => {
            set_map_object_builder(ctx.session, id, next.build_player_uid)
                .map_err(|error| HostError::new(error.to_string()))?;
            if let Some((views, _)) = ctx.map_objects.as_mut() {
                if let Some(view) = views.get_mut(position) {
                    view.build_player_uid = next.build_player_uid;
                }
            }
        }
        other => return Err(HostError::new(format!("{other} has no write path"))),
    }
    ctx.note_write();
    Ok(())
}

fn map_object_newindex(state: *mut lua_State) -> Result<c_int, HostError> {
    unsafe {
        check_args(state, 3, "map_object field assignment")?;
        let handle = read_handle(state, 1, HandleKind::MapObject)?;
        let field = arg_string(state, 2, "field")?;
        let value = read_field_value(state, 3, &field)?;
        with_context(state, |ctx| map_object_set(ctx, handle.id, &field, value))?;
        Ok(0)
    }
}

host_fn!(push_map_object_newindex, map_object_newindex);
