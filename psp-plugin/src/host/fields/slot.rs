use std::ffi::c_int;
use std::sync::OnceLock;

use psp_core::domain::containers;
use psp_core::dto::container::{ItemContainerDto, ItemContainerSlotDto};
use psp_core::gamedata::GameData;
use psp_lua_sys::ffi::*;
use uuid::Uuid;

use super::{
    expect_int, expect_str, read_field_value, Access, FieldSpec, FieldValue, FieldWrite,
    GameDataCheck, Reader,
};
use crate::context::RunContext;
use crate::host::api_def::{ApiField, ApiType};
use crate::host::handle::{read_handle, HandleKind};
use crate::host::marshal::{arg_string, check_args};
use crate::host::{dto_cache, save_read, with_context, HostError};
use crate::host_fn;
use crate::manifest::Capability;

// --- readers -------------------------------------------------------------

fn read_index(slot: &ItemContainerSlotDto) -> FieldValue {
    FieldValue::Int(i64::from(slot.slot_index))
}

/// The three spellings of an empty slot the save uses, all reported as nil --
/// the same mapping the hand-written read did before this table existed.
fn read_item_id(slot: &ItemContainerSlotDto) -> FieldValue {
    match slot.static_id.as_deref() {
        Some("") | Some("None") | None => FieldValue::Nil,
        Some(id) => FieldValue::Str(id.to_string()),
    }
}

fn read_count(slot: &ItemContainerSlotDto) -> FieldValue {
    FieldValue::Int(i64::from(slot.count))
}

// --- writers -------------------------------------------------------------

/// The refusal for `"None"`: the one value `apply_item_container_dto` routes to
/// `remove_raw_slot`, which deletes the slot entry and shifts every later one --
/// the same structural write `slot.clear()` performs, and one an assignment must
/// not be able to spell.
fn removes_the_slot() -> HostError {
    HostError::new(
        "item_id cannot be assigned \"None\": that is the one value the save reads as \"delete \
         this slot\", and removing an entry is a structural write that invalidates every live \
         handle and iterator. Use slot.clear() instead",
    )
}

/// The refusal for the two values that only *look* like `"None"`. Neither
/// reaches `remove_raw_slot`: an empty string would be upserted verbatim,
/// leaving a slot that reads back as nil but still holds an entry, and nil never
/// reaches `psp-core` at all. Both are refused because emptying a slot is what
/// they mean and `slot.clear()` is what does it -- not because either would
/// remove anything by itself.
fn does_not_empty_the_slot(spelled: &str) -> HostError {
    HostError::new(format!(
        "item_id cannot be assigned {spelled}: it reads back as an empty slot without emptying \
         one -- the entry would still be there, holding an item with no id. Use slot.clear() to \
         empty the slot instead"
    ))
}

/// Kept out of the row's own `validate`, which sees only the slot: the catalog
/// lives on `GameData`, the same way the pal handle's skill-entry check does.
/// Matched case-insensitively, because save ids and `items.json` do not agree on
/// casing; the id is stored exactly as written, since `upsert_raw_slot` never
/// rewrites it to the catalog's spelling.
///
/// An absent or empty catalog turns the check off rather than refusing every
/// write. `is_known_item_key` answers false for every id against an empty set,
/// so a missing `items.json` would otherwise become a wall across the whole row,
/// and an unavailable catalog is not evidence that an id is wrong.
fn validate_item_is_known(
    game_data: &GameData,
    field: &'static str,
    _current: &ItemContainerSlotDto,
    value: &FieldValue,
) -> Result<(), HostError> {
    let FieldValue::Str(id) = value else {
        return Ok(());
    };
    let populated = game_data
        .get("items")
        .and_then(serde_json::Value::as_object)
        .is_some_and(|items| !items.is_empty());
    if !populated {
        return Ok(());
    }
    if !game_data.is_known_item_key(id) {
        return Err(HostError::new(format!("{field} {id:?} is not in the items catalog")));
    }
    Ok(())
}

/// Three refusals, for three different reasons.
///
/// `"None"` is structural: it removes the slot entry rather than writing a value
/// into it. Nil and the empty string are not -- they are refused because they
/// mean "empty this slot" and would not do it; see `does_not_empty_the_slot`.
///
/// The last is the per-item record. A slot carrying one -- a weapon's
/// durability and remaining rounds, an egg's pal, armour's condition -- is a
/// slot whose record names its own item, and `upsert_dynamic_item` never
/// rewrites an existing record's `static_id`, having no authoritative value for
/// it. Re-pointing only the slot would leave the two naming different items.
fn validate_item_id(current: &ItemContainerSlotDto, value: &FieldValue) -> Result<(), HostError> {
    if matches!(value, FieldValue::Nil) {
        return Err(does_not_empty_the_slot("nil"));
    }
    let text = expect_str("item_id", value)?;
    if text == "None" {
        return Err(removes_the_slot());
    }
    if text.is_empty() {
        return Err(does_not_empty_the_slot("an empty string"));
    }
    if current.dynamic_item.is_some() {
        return Err(HostError::new(
            "item_id cannot be assigned on a slot that carries a per-item record -- durability, \
             remaining rounds, an egg's pal, or a weapon's passives. That record names its own \
             item and nothing here can rewrite it, so the slot and the record would end up \
             naming different items. Use slot.clear() to empty the slot instead",
        ));
    }
    Ok(())
}

fn apply_item_id(slot: &mut ItemContainerSlotDto, value: FieldValue) {
    if let FieldValue::Str(text) = value {
        slot.static_id = Some(text);
    }
}

/// Zero and below are refused rather than written: a slot holding none of its
/// item is an empty slot, and the save empties one by removing its entry, which
/// is structural. The upper bound is the save's own -- the raw slot holds the
/// count as an `i32`.
fn validate_count(_current: &ItemContainerSlotDto, value: &FieldValue) -> Result<(), HostError> {
    let count = expect_int("count", value)?;
    if count < 1 {
        return Err(HostError::new(format!(
            "count must be at least 1, got {count}: a slot holding none of its item is an empty \
             slot, and emptying one removes its entry -- a structural write. Use slot.clear() \
             instead"
        )));
    }
    if count > i64::from(i32::MAX) {
        return Err(HostError::new(format!("count must be between 1 and {}, got {count}", i32::MAX)));
    }
    Ok(())
}

fn apply_count(slot: &mut ItemContainerSlotDto, value: FieldValue) {
    if let FieldValue::Int(count) = value {
        if let Ok(count) = i32::try_from(count) {
            slot.count = count;
        }
    }
}

const fn rw(
    name: &'static str,
    ty: ApiType,
    doc: &'static str,
    read: fn(&ItemContainerSlotDto) -> FieldValue,
    validate: fn(&ItemContainerSlotDto, &FieldValue) -> Result<(), HostError>,
    apply: fn(&mut ItemContainerSlotDto, FieldValue),
) -> FieldSpec<ItemContainerSlotDto, ()> {
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

/// Like `rw`, plus a game-data check the row carries itself, run after
/// `validate`. The position is what `item_id` needs: its own refusals name the
/// operation to use instead, and a catalog check running first would answer
/// `"None"` and the empty string with a bare "not in the catalog".
const fn rw_postchecked(
    name: &'static str,
    ty: ApiType,
    doc: &'static str,
    read: fn(&ItemContainerSlotDto) -> FieldValue,
    validate: fn(&ItemContainerSlotDto, &FieldValue) -> Result<(), HostError>,
    apply: fn(&mut ItemContainerSlotDto, FieldValue),
    postcheck: GameDataCheck<ItemContainerSlotDto>,
) -> FieldSpec<ItemContainerSlotDto, ()> {
    FieldSpec {
        name,
        ty,
        access: Access::ReadWrite,
        doc,
        read: Reader::Dto(read),
        instead_call: None,
        game_data_precheck: None,
        game_data_postcheck: Some(postcheck),
        write: Some(FieldWrite { validate, apply }),
    }
}

const fn ro(
    name: &'static str,
    ty: ApiType,
    doc: &'static str,
    read: fn(&ItemContainerSlotDto) -> FieldValue,
) -> FieldSpec<ItemContainerSlotDto, ()> {
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

/// Every field this handle answers for. `index` is where the slot sits, which
/// only a structural write moves; the other two are ordinary values the save
/// holds in the raw slot and `upsert_raw_slot` overwrites in place.
///
/// The per-item record a slot may carry -- `dynamic_item`, with its skills,
/// talents, durability and gender -- has no row here. Writes preserve it (see
/// `slot_write_payload`); exposing it is a separate question with its own
/// nested shape, and this table does not answer it.
pub const SLOT_FIELDS: &[FieldSpec<ItemContainerSlotDto, ()>] = &[
    ro(
        "index",
        ApiType::Integer,
        "This slot's position within its container. Read-only: moving a slot means removing and \
         re-adding its entry, which is structural.",
        read_index,
    ),
    rw_postchecked(
        "item_id",
        ApiType::Union(&[ApiType::String, ApiType::Nil]),
        "The static item id occupying this slot, or nil if the slot is empty. Must name an item \
         the loaded game data knows, matched case-insensitively and stored exactly as written; \
         an id the catalog does not hold raises, and the check is skipped entirely when no \
         catalog is loaded. Assigning \"None\" raises: it is the one value the save reads as \
         \"delete this slot\", which is structural -- use slot.clear(). Assigning nil or an \
         empty string raises too: both read back as an empty slot without emptying one, leaving \
         an entry holding an item with no id. Assigning on a slot that carries a per-item record \
         (durability, an egg's pal, a weapon's passives) also raises: the record names its own \
         item and cannot be re-pointed here.",
        read_item_id,
        validate_item_id,
        apply_item_id,
        validate_item_is_known,
    ),
    rw(
        "count",
        ApiType::Integer,
        "How many of the item occupy this slot. Must be at least 1: a slot holding none of its \
         item is an empty slot, and emptying one is structural -- use slot.clear(). No upper \
         bound beyond what the save can hold, because nothing in the game's data or in this app \
         establishes a stack limit, and refusing one here would be inventing a rule rather than \
         reporting one.",
        read_count,
        validate_count,
        apply_count,
    ),
];

static API_FIELDS: OnceLock<Vec<ApiField>> = OnceLock::new();

/// The published description of this handle's fields, projected from the same
/// rows that answer the reads and validate the writes.
pub(crate) fn api_fields() -> &'static [ApiField] {
    API_FIELDS
        .get_or_init(|| {
            SLOT_FIELDS
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

fn find(field: &str) -> Option<&'static FieldSpec<ItemContainerSlotDto, ()>> {
    SLOT_FIELDS.iter().find(|spec| spec.name == field)
}

fn current_slot(
    ctx: &mut RunContext<'_>,
    container_id: Uuid,
    slot_index: i32,
) -> Option<ItemContainerSlotDto> {
    save_read::read_container(ctx, container_id)
        .and_then(|dto| dto.slots.iter().find(|slot| slot.slot_index == slot_index).cloned())
}

/// The only `ItemContainerDto` a slot assignment ever hands to
/// `apply_item_container_dto`: one slot, rebuilt field by field from the one
/// the save currently holds.
///
/// `dynamic_item` is why this exists rather than being an extra copy. An
/// incoming slot whose `dynamic_item` is `None` makes
/// `apply_resolved_container_dto` delete the record the slot already had --
/// correct for a clear, silent data loss for an assignment. Carrying the record
/// across unchanged is what keeps a `count` write a `count` write. The record
/// round-trips faithfully because it was read with `modified` false, which is
/// what makes `build_dynamic_item_type` keep the existing payload rather than
/// rebuild it from the DTO's flattened view.
///
/// `r#type` is left empty so the essential-container branch, which resizes a
/// paired common container from scratch, cannot be reached; `slot_num` and
/// `local_id` are output-only and never read on the way in.
///
/// Destructured exhaustively, with no `..`, so a field added to
/// `ItemContainerSlotDto` later stops compiling here rather than silently
/// arriving in -- or silently vanishing from -- the write.
fn slot_write_payload(container_id: Uuid, slot: &ItemContainerSlotDto) -> ItemContainerDto {
    let ItemContainerSlotDto { dynamic_item, slot_index, count, static_id, local_id: _ } = slot;
    ItemContainerDto {
        id: container_id,
        r#type: String::new(),
        slots: vec![ItemContainerSlotDto {
            dynamic_item: dynamic_item.clone(),
            slot_index: *slot_index,
            count: *count,
            static_id: static_id.clone(),
            local_id: None,
        }],
        key: None,
        slot_num: 0,
    }
}

/// Reads one field off the slot the save currently holds. A slot the container
/// no longer carries -- or a container that cannot be read at all -- answers
/// nil for every row, `index` included, which is what the hand-written read did
/// before this table existed. An unrecognized field name also returns `Nil`,
/// matching every other handle's read side.
pub(crate) fn slot_get(
    ctx: &mut RunContext<'_>,
    container_id: Uuid,
    slot_index: i32,
    field: &str,
) -> Result<FieldValue, HostError> {
    let Some(spec) = find(field) else {
        return Ok(FieldValue::Nil);
    };
    let read = match spec.read {
        Reader::Dto(read) => read,
        // No row sources from a summary: a slot has none, and `()` is what
        // stands in its place.
        Reader::Summary(read) => return Ok(read(&())),
    };
    Ok(current_slot(ctx, container_id, slot_index).as_ref().map(read).unwrap_or(FieldValue::Nil))
}

/// A slot write lands on the save at the point of assignment rather than being
/// held in a cache and flushed later, so a read afterwards re-reads the save and
/// the two cannot disagree -- not at any point in the run, and not across a
/// flush. `note_mutation` is deliberately not called: `upsert_raw_slot`
/// overwrites the existing entry in place, adding and removing nothing, so no
/// handle or iterator is looking at anything that moved. `note_write` drops the
/// run's cached copy of the container so the next read comes off the save.
///
/// A dry run writes nothing and records the accepted value instead, which is
/// what its own later reads are answered from.
///
/// `save.write` is checked before any field resolution -- before the name is
/// looked up, and so before `unknown slot field` or `is read-only` can be
/// reported -- so an ungranted write is not told which fields exist or which of
/// them it could have written.
pub(crate) fn slot_set(
    ctx: &mut RunContext<'_>,
    container_id: Uuid,
    slot_index: i32,
    field: &str,
    value: FieldValue,
) -> Result<(), HostError> {
    if !ctx.grants(Capability::SaveWrite) {
        return Err(HostError::new("slot field assignment requires the save.write capability"));
    }
    let Some(spec) = find(field) else {
        return Err(HostError::new(format!("unknown slot field {field:?}")));
    };
    let Some(write) = spec.write.as_ref() else {
        return Err(spec.not_assignable());
    };
    let value = spec.coerce_empty_table(value);
    let game_data = ctx.game_data;
    let Some(current) = current_slot(ctx, container_id, slot_index) else {
        return Err(HostError::new(format!(
            "{field} cannot be written: container {container_id} holds no slot {slot_index}"
        )));
    };
    spec.validate_write(game_data, &current, &value)?;
    let mut next = current;
    (write.apply)(&mut next, value);
    if ctx.dry_run {
        ctx.bump(&format!("slot.{}", spec.name), 1);
        dto_cache::note_pending_slot(ctx, container_id, &next);
        return Ok(());
    }
    let payload = slot_write_payload(container_id, &next);
    containers::apply_item_container_dto(ctx.session, container_id, &payload, None)
        .map_err(|error| HostError::new(error.to_string()))?;
    ctx.note_write();
    Ok(())
}

fn slot_newindex(state: *mut lua_State) -> Result<c_int, HostError> {
    unsafe {
        check_args(state, 3, "slot field assignment")?;
        let handle = read_handle(state, 1, HandleKind::Slot)?;
        let field = arg_string(state, 2, "field")?;
        let value = read_field_value(state, 3, &field)?;
        with_context(state, |ctx| slot_set(ctx, handle.id, handle.slot, &field, value))?;
        Ok(0)
    }
}

host_fn!(push_slot_newindex, slot_newindex);

#[cfg(test)]
mod tests {
    use super::*;

    fn a_slot_carrying_a_record() -> ItemContainerSlotDto {
        let record = serde_json::json!({
            "local_id": "11111111-2222-3333-4444-555555555555",
            "type": "weapon",
            "durability": 71.0,
            "remaining_bullets": 1,
            "static_id": "WeakerBow",
        });
        ItemContainerSlotDto {
            dynamic_item: Some(serde_json::from_value(record).expect("the fixture record parses")),
            slot_index: 4,
            count: 5,
            static_id: Some("WeakerBow".to_string()),
            local_id: Some("11111111-2222-3333-4444-555555555555".parse().expect("a uuid")),
        }
    }

    /// The property the exhaustive destructure cannot state: a slot write sends
    /// exactly the slot it was given and no other. Every extra slot in the
    /// payload is one `apply_resolved_container_dto` would act on -- removing
    /// the entry of any it finds spelled `"None"`, and deleting the record of
    /// any that arrives without one. This kills the "the helper started
    /// carrying the container" form; a caller that bypassed the helper
    /// altogether would need a synthetic raw slot to catch and is not covered
    /// here.
    #[test]
    fn the_write_payload_carries_exactly_the_one_slot_it_was_given() {
        let slot = a_slot_carrying_a_record();
        let container_id = "22222222-2222-3333-4444-555555555555".parse().expect("a uuid");
        let payload = slot_write_payload(container_id, &slot);

        assert_eq!(payload.slots.len(), 1, "a slot write must send one slot, not a container");
        assert_eq!(payload.id, container_id);
        assert!(
            payload.r#type.is_empty(),
            "an empty type is what keeps the essential-container resize branch unreachable"
        );

        let sent = payload.slots.first().expect("checked above");
        assert_eq!(sent.slot_index, slot.slot_index);
        assert_eq!(sent.count, slot.count);
        assert_eq!(sent.static_id, slot.static_id);

        let record = sent.dynamic_item.as_ref().expect("the record must be carried across");
        let original = slot.dynamic_item.as_ref().expect("the fixture carries one");
        assert_eq!(record.local_id, original.local_id);
        assert_eq!(record.durability, original.durability);
        assert_eq!(record.remaining_bullets, original.remaining_bullets);
        assert!(!record.modified, "a modified record would be rebuilt rather than carried over");
    }
}
