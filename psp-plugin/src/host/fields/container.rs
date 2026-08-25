use std::ffi::c_int;
use std::sync::OnceLock;

use psp_core::dto::container::ItemContainerDto;
use psp_lua_sys::ffi::*;
use uuid::Uuid;

use super::{read_field_value, Access, FieldSpec, FieldValue, Reader};
use crate::context::RunContext;
use crate::host::api_def::{ApiField, ApiType};
use crate::host::handle::{read_handle, HandleKind};
use crate::host::marshal::{arg_string, check_args};
use crate::host::{with_context, HostError};
use crate::host_fn;
use crate::manifest::Capability;

// --- readers -------------------------------------------------------------

/// From the handle, not from the container: `id` is what the handle was minted
/// with, so it answers even for a container the save can no longer read -- the
/// case `slot_count` degrades to nil for.
fn read_id(id: &Uuid) -> FieldValue {
    FieldValue::Str(id.to_string())
}

fn read_slot_count(dto: &ItemContainerDto) -> FieldValue {
    FieldValue::Int(i64::from(dto.slot_num))
}

const fn ro(
    name: &'static str,
    ty: ApiType,
    doc: &'static str,
    read: fn(&ItemContainerDto) -> FieldValue,
    instead_call: Option<&'static str>,
) -> FieldSpec<ItemContainerDto, Uuid> {
    FieldSpec {
        name,
        ty,
        access: Access::ReadOnly,
        doc,
        read: Reader::Dto(read),
        instead_call,
        game_data_precheck: None,
        game_data_postcheck: None,
        write: None,
    }
}

/// Read-only and sourced from the handle's own id rather than from the
/// container -- the container handle has no summary, so the id plays that part.
const fn ro_handle(
    name: &'static str,
    ty: ApiType,
    doc: &'static str,
    read: fn(&Uuid) -> FieldValue,
) -> FieldSpec<ItemContainerDto, Uuid> {
    FieldSpec {
        name,
        ty,
        access: Access::ReadOnly,
        doc,
        read: Reader::Summary(read),
        instead_call: None,
        game_data_precheck: None,
        game_data_postcheck: None,
        write: None,
    }
}

/// Every field this handle answers for, and none of them is assignable. That
/// is the whole of this handle's write surface, not a stage it is passing
/// through: `id` is identity, `slots` is an iterator rather than a field, and
/// `slot_count` is structural -- resizing a container removes or admits raw
/// slot entries, which invalidates every live handle and iterator in the run.
/// An assignment would make that look like a cheap local write and would take
/// the collect-then-write shape out of the call site, so it stays
/// `set_slot_count(n)` and the row names it in its refusal.
pub const CONTAINER_FIELDS: &[FieldSpec<ItemContainerDto, Uuid>] = &[
    ro_handle("id", ApiType::String, "The container's UUID, as a string. Read-only.", read_id),
    ro(
        "slot_count",
        ApiType::Union(&[ApiType::Integer, ApiType::Nil]),
        "How many slots this container has, or nil if the container could not be read. Cannot \
         be assigned: resizing a container is a structural write that invalidates every live \
         handle and iterator, so it stays container.set_slot_count(n), which reports whether it \
         resized and refuses rather than destroying an occupied slot.",
        read_slot_count,
        Some("container.set_slot_count(n)"),
    ),
];

/// The zero-assignable-rows property, enforced rather than described. A row
/// given a write later stops this file compiling, which is the point at which
/// somebody has to decide whether that write is structural.
const _: () = {
    let mut index = 0;
    while index < CONTAINER_FIELDS.len() {
        assert!(
            CONTAINER_FIELDS[index].write.is_none(),
            "a writable container row needs a write path this handle does not have"
        );
        index += 1;
    }
};

static API_FIELDS: OnceLock<Vec<ApiField>> = OnceLock::new();

/// The published description of this handle's fields, projected from the same
/// rows that answer the reads and refuse the writes.
pub(crate) fn api_fields() -> &'static [ApiField] {
    API_FIELDS
        .get_or_init(|| {
            CONTAINER_FIELDS
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

fn find(field: &str) -> Option<&'static FieldSpec<ItemContainerDto, Uuid>> {
    CONTAINER_FIELDS.iter().find(|spec| spec.name == field)
}

/// Reads one field, from the handle's id or from the container the save holds.
/// An unrecognized field name returns `Nil`, matching every other handle's read
/// side.
///
/// There is no cache between this and the save: `read_container` re-reads the
/// container whenever the run has moved on from the one it last held, so a read
/// after any write answers what the save actually holds rather than what the
/// run believes it wrote.
pub(crate) fn container_get(
    ctx: &mut RunContext<'_>,
    id: Uuid,
    field: &str,
) -> Result<FieldValue, HostError> {
    let Some(spec) = find(field) else {
        return Ok(FieldValue::Nil);
    };
    match spec.read {
        Reader::Dto(read) => {
            Ok(crate::host::save_read::read_container(ctx, id).map(read).unwrap_or(FieldValue::Nil))
        }
        Reader::Summary(read) => Ok(read(&id)),
    }
}

/// Refuses every assignment, because every row is read-only -- but refuses each
/// in the terms of its own row, so `slot_count` names `set_slot_count`.
///
/// `save.write` is checked before any field resolution -- before the name is
/// looked up, and so before `unknown container field` or either refusal can be
/// reported -- so an ungranted write is not told which fields exist.
pub(crate) fn container_set(
    ctx: &mut RunContext<'_>,
    field: &str,
    _value: FieldValue,
) -> Result<(), HostError> {
    if !ctx.grants(Capability::SaveWrite) {
        return Err(HostError::new("container field assignment requires the save.write capability"));
    }
    let Some(spec) = find(field) else {
        return Err(HostError::new(format!("unknown container field {field:?}")));
    };
    Err(spec.not_assignable())
}

fn container_newindex(state: *mut lua_State) -> Result<c_int, HostError> {
    unsafe {
        check_args(state, 3, "container field assignment")?;
        read_handle(state, 1, HandleKind::Container)?;
        let field = arg_string(state, 2, "field")?;
        let value = read_field_value(state, 3, &field)?;
        with_context(state, |ctx| container_set(ctx, &field, value))?;
        Ok(0)
    }
}

host_fn!(push_container_newindex, container_newindex);
