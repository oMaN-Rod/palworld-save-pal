//! Accessors into `uesave`'s dynamic `Property` tree, used where no typed
//! `uesave` struct exists for a node.
//!
//! Everything here returns `Option`: a missing or wrong-typed node in an
//! untrusted save file is a normal condition, not a bug. These accessors carry
//! no path in their failure, so callers should name the path in their own
//! `CoreError` when a lookup comes back `None`.

/// Looks up a property by name, descending through nested user structs when
/// `path` has more than one segment.
pub fn get<'a>(properties: &'a crate::ue::Properties, path: &[&str]) -> Option<&'a crate::ue::Property> {
    let (segment, rest) = path.split_first()?;
    let property = properties
        .into_iter()
        .find_map(|(key, value)| (key.1 == *segment).then_some(value))?;
    if rest.is_empty() {
        Some(property)
    } else {
        get_in(property, rest)
    }
}

/// Like `get`, but starting from a property rather than a `Properties` map.
pub fn get_in<'a>(property: &'a crate::ue::Property, path: &[&str]) -> Option<&'a crate::ue::Property> {
    let mut current = property;
    for segment in path {
        current = get(struct_properties(current)?, &[segment])?;
    }
    Some(current)
}

/// Mutable counterpart of `get`. Matches by NAME only, ignoring the `u32` half
/// of a `PropertyKey` that disambiguates same-named siblings, so it resolves
/// exactly the node `get` would.
pub fn get_mut<'a>(
    properties: &'a mut crate::ue::Properties,
    path: &[&str],
) -> Option<&'a mut crate::ue::Property> {
    let (segment, rest) = path.split_first()?;
    let property = properties
        .0
        .iter_mut()
        .find_map(|(key, value)| (key.1 == *segment).then_some(value))?;
    if rest.is_empty() {
        Some(property)
    } else {
        get_in_mut(property, rest)
    }
}

/// Mutable counterpart of `get_in`.
pub fn get_in_mut<'a>(
    property: &'a mut crate::ue::Property,
    path: &[&str],
) -> Option<&'a mut crate::ue::Property> {
    let mut current = property;
    for segment in path {
        current = get_mut(struct_props_mut(current)?, &[segment])?;
    }
    Some(current)
}

/// The nested `Properties` map of a user-struct property (e.g. `ContainerId`,
/// whose value is itself a bag of properties).
pub fn struct_properties(property: &crate::ue::Property) -> Option<&crate::ue::Properties> {
    match property {
        crate::ue::Property::Struct(crate::ue::StructValue::Struct(properties)) => Some(properties),
        _ => None,
    }
}

/// A `Str`, `Name`, or `Enum` property's text.
pub fn as_str(property: &crate::ue::Property) -> Option<&str> {
    match property {
        crate::ue::Property::Str(text)
        | crate::ue::Property::Name(text)
        | crate::ue::Property::Enum(text) => Some(text),
        _ => None,
    }
}

pub fn as_bool(property: &crate::ue::Property) -> Option<bool> {
    match property {
        crate::ue::Property::Bool(value) => Some(*value),
        _ => None,
    }
}

/// An `Enum` property's fully qualified variant name (e.g.
/// `"EPalGroupType::Guild"`).
pub fn as_enum(property: &crate::ue::Property) -> Option<&str> {
    match property {
        crate::ue::Property::Enum(value) => Some(value),
        _ => None,
    }
}

/// A raw `Byte` property's value. Returns `None` for an enum-backed byte
/// (`Byte::Label`) — use `as_enum` for those.
pub fn as_byte(property: &crate::ue::Property) -> Option<u8> {
    match property {
        crate::ue::Property::Byte(crate::ue::Byte::Byte(value)) => Some(*value),
        _ => None,
    }
}

/// .NET-style ticks from a `DateTime` struct property (the player .sav
/// "Timestamp").
pub fn as_datetime_ticks(property: &crate::ue::Property) -> Option<u64> {
    match property {
        crate::ue::Property::Struct(crate::ue::StructValue::DateTime(ticks)) => Some(*ticks),
        _ => None,
    }
}

pub fn map_entries(property: &crate::ue::Property) -> Option<&Vec<crate::ue::MapEntry>> {
    match property {
        crate::ue::Property::Map(entries) => Some(entries),
        _ => None,
    }
}

pub fn map_entries_mut(property: &mut crate::ue::Property) -> Option<&mut Vec<crate::ue::MapEntry>> {
    match property {
        crate::ue::Property::Map(entries) => Some(entries),
        _ => None,
    }
}

/// A byte-array `Array` property's contents (e.g. `RawData`). Returns
/// `None` for an enum-labeled byte array.
pub fn as_byte_array(property: &crate::ue::Property) -> Option<&[u8]> {
    match property {
        crate::ue::Property::Array(crate::ue::ValueVec::Byte(crate::ue::ByteArray::Byte(bytes))) => {
            Some(bytes)
        }
        _ => None,
    }
}

/// `FGuid`'s `Display` already renders Palworld's guid byte order as a
/// canonical UUID string. Panics only on unreachable input; prefer
/// `guid_to_uuid` for untrusted save data.
pub fn fguid_to_uuid(guid: &crate::ue::FGuid) -> uuid::Uuid {
    guid.to_string()
        .parse()
        .expect("FGuid Display always yields a canonical uuid")
}

pub fn as_uuid(property: &crate::ue::Property) -> Option<uuid::Uuid> {
    match property {
        crate::ue::Property::Struct(crate::ue::StructValue::Guid(guid)) => Some(fguid_to_uuid(guid)),
        _ => None,
    }
}

/// The nil UUID — Palworld's sentinel for "no owner"/"no group" in several
/// property slots.
pub const EMPTY_UUID: uuid::Uuid = uuid::Uuid::nil();

/// `FGuid` -> `Uuid` without panicking, for untrusted save data. The nil
/// fallback is unreachable in practice (`FGuid`'s four `u32` fields always
/// format to 32 hex digits) but a defensive accessor must not panic.
pub fn guid_to_uuid(guid: &crate::ue::FGuid) -> uuid::Uuid {
    guid.to_string().parse().unwrap_or(uuid::Uuid::nil())
}

/// `Uuid` -> `FGuid` for writing back into a save. The nil fallback is
/// likewise unreachable: a `Uuid`'s canonical string is always 32 hex digits,
/// which `FGuid::parse_str` accepts.
pub fn uuid_to_guid(value: uuid::Uuid) -> crate::ue::FGuid {
    crate::ue::FGuid::parse_str(&value.to_string()).unwrap_or_else(|_| crate::ue::FGuid::nil())
}

/// Alias of `struct_properties`.
pub fn struct_props(property: &crate::ue::Property) -> Option<&crate::ue::Properties> {
    struct_properties(property)
}

pub fn struct_props_mut(property: &mut crate::ue::Property) -> Option<&mut crate::ue::Properties> {
    match property {
        crate::ue::Property::Struct(crate::ue::StructValue::Struct(properties)) => Some(properties),
        _ => None,
    }
}

pub fn as_i32(property: &crate::ue::Property) -> Option<i32> {
    match property {
        crate::ue::Property::Int(value) => Some(*value),
        _ => None,
    }
}

/// Also widens a plain `Int`: the game writes some numeric fields as `Int` or
/// `Int64` depending on the engine version that produced the save.
pub fn as_i64(property: &crate::ue::Property) -> Option<i64> {
    match property {
        crate::ue::Property::Int64(value) => Some(*value),
        crate::ue::Property::Int(value) => Some(*value as i64),
        _ => None,
    }
}

pub fn as_f32(property: &crate::ue::Property) -> Option<f32> {
    match property {
        crate::ue::Property::Float(crate::ue::Float(value)) => Some(*value),
        _ => None,
    }
}

/// Alias of `as_byte`.
pub fn as_byte_number(property: &crate::ue::Property) -> Option<u8> {
    as_byte(property)
}

pub fn name_values(property: &crate::ue::Property) -> Option<&Vec<String>> {
    match property {
        crate::ue::Property::Array(crate::ue::ValueVec::Name(values)) => Some(values),
        _ => None,
    }
}

pub fn enum_values(property: &crate::ue::Property) -> Option<&Vec<String>> {
    match property {
        crate::ue::Property::Array(crate::ue::ValueVec::Enum(values)) => Some(values),
        _ => None,
    }
}

pub fn struct_values(property: &crate::ue::Property) -> Option<&Vec<crate::ue::StructValue>> {
    match property {
        crate::ue::Property::Array(crate::ue::ValueVec::Struct(values)) => Some(values),
        _ => None,
    }
}

pub fn struct_values_mut(property: &mut crate::ue::Property) -> Option<&mut Vec<crate::ue::StructValue>> {
    match property {
        crate::ue::Property::Array(crate::ue::ValueVec::Struct(values)) => Some(values),
        _ => None,
    }
}

/// A `FixedPoint64` stat field: always the bare user struct
/// `{"Value": Int64(n)}`. Uses `Properties::get`, never the panicking `Index`
/// impl, since a malformed save may be missing the field.
pub fn fixed_point64(property: &crate::ue::Property) -> Option<i64> {
    let inner = struct_props(property)?;
    as_i64(inner.0.get(&crate::ue::PropertyKey::from("Value"))?)
}

pub fn guid_property(value: uuid::Uuid) -> crate::ue::Property {
    crate::ue::Property::Struct(crate::ue::StructValue::Guid(uuid_to_guid(value)))
}

pub fn str_property(value: &str) -> crate::ue::Property {
    crate::ue::Property::Str(value.to_string())
}

pub fn name_property(value: &str) -> crate::ue::Property {
    crate::ue::Property::Name(value.to_string())
}

/// `value` must be the fully qualified variant name (e.g.
/// `"EPalGenderType::Female"`).
pub fn enum_property(value: &str) -> crate::ue::Property {
    crate::ue::Property::Enum(value.to_string())
}

pub fn bool_property(value: bool) -> crate::ue::Property {
    crate::ue::Property::Bool(value)
}

pub fn int_property(value: i32) -> crate::ue::Property {
    crate::ue::Property::Int(value)
}

pub fn int64_property(value: i64) -> crate::ue::Property {
    crate::ue::Property::Int64(value)
}

pub fn float_property(value: f32) -> crate::ue::Property {
    crate::ue::Property::Float(crate::ue::Float(value))
}

pub fn byte_property(value: u8) -> crate::ue::Property {
    crate::ue::Property::Byte(crate::ue::Byte::Byte(value))
}

pub fn name_array_property(values: Vec<String>) -> crate::ue::Property {
    crate::ue::Property::Array(crate::ue::ValueVec::Name(values))
}

pub fn enum_array_property(values: Vec<String>) -> crate::ue::Property {
    crate::ue::Property::Array(crate::ue::ValueVec::Enum(values))
}

/// Inverse of `fixed_point64`.
pub fn fixed_point64_property(value: i64) -> crate::ue::Property {
    let mut inner = crate::ue::Properties::default();
    inner.insert("Value", crate::ue::Property::Int64(value));
    crate::ue::Property::Struct(crate::ue::StructValue::Struct(inner))
}

// `uesave`'s writer looks up a property's schema by its exact dotted scope path
// and fails with `Error::MissingPropertySchema` when none is recorded. Any
// property NAME newly introduced into a save (as opposed to overwriting a
// present one, which already carries a schema from the read) must have a schema
// registered before write.

/// Finds a recorded schema path ending with `suffix` and returns everything
/// before it — the way to derive a schema for a brand-new property by copying
/// an existing sibling's shape, since no schema exists at the new path yet.
pub fn schema_prefix_ending_with(save: &crate::ue::Save, suffix: &str) -> Option<String> {
    save.schemas
        .schemas()
        .keys()
        .find(|key| key.ends_with(suffix))
        .map(|key| key[..key.len() - suffix.len()].to_string())
}

/// Records `tag` at `path` only when no schema exists there yet; never
/// overwrites one already read from the real save.
pub fn ensure_schema(save: &mut crate::ue::Save, path: String, tag: crate::ue::PropertyTagPartial) {
    if save.schemas.get(&path).is_none() {
        save.schemas.record(path, tag);
    }
}

/// Copies every schema `source` recorded and `target` lacks.
///
/// Schemas are per-`Save` but a schema path is just the chain of property names, so
/// a subtree grafted across saves keeps its path -- and carries properties the
/// target may never have had a tag for. `target`'s own tags always win: only they
/// describe how the target was actually read.
pub fn merge_schemas(target: &mut crate::ue::Save, source: &crate::ue::Save) {
    for (path, tag) in source.schemas.schemas().clone() {
        ensure_schema(target, path, tag);
    }
}

/// `old` -> `new`, `new` -> `old`, anything else `None`. Both sides are parsed
/// `Uuid`s, so the match ignores hyphenation and case in the source text.
fn swap_uuid_value(value: uuid::Uuid, old: uuid::Uuid, new: uuid::Uuid) -> Option<uuid::Uuid> {
    if value == old {
        Some(new)
    } else if value == new {
        Some(old)
    } else {
        None
    }
}

/// Swaps a single ownership-key leaf in place: a `Str` holding uuid text, or a
/// `Guid` struct. Any other shape (including a `Str` that isn't valid uuid
/// text) is left untouched.
fn swap_leaf_uuid_property(property: &mut crate::ue::Property, old: uuid::Uuid, new: uuid::Uuid) {
    match property {
        crate::ue::Property::Str(text) => {
            if let Ok(parsed) = text.parse::<uuid::Uuid>() {
                if let Some(swapped) = swap_uuid_value(parsed, old, new) {
                    *text = swapped.to_string();
                }
            }
        }
        crate::ue::Property::Struct(crate::ue::StructValue::Guid(guid)) => {
            if let Some(swapped) = swap_uuid_value(guid_to_uuid(guid), old, new) {
                *guid = uuid_to_guid(swapped);
            }
        }
        _ => {}
    }
}

/// Descends only through the shapes that can carry a named field bag: a user
/// struct, an array of user structs, and a map's key/value pairs. The
/// game-specific typed structs (`PalGroupData`, `PalCharacterData`, the map
/// object models) have no generic `Properties` bag, so recursion stops at them
/// — see `swap_uuid_values_deep` for why that is correct and must not be
/// "fixed" by hand-adding typed-struct arms.
fn swap_uuid_values_deep_in_property(
    property: &mut crate::ue::Property,
    keys: &[&str],
    old: uuid::Uuid,
    new: uuid::Uuid,
) {
    match property {
        crate::ue::Property::Struct(crate::ue::StructValue::Struct(nested)) => {
            swap_uuid_values_deep(nested, keys, old, new);
        }
        crate::ue::Property::Array(crate::ue::ValueVec::Struct(values)) => {
            for value in values.iter_mut() {
                if let crate::ue::StructValue::Struct(nested) = value {
                    swap_uuid_values_deep(nested, keys, old, new);
                }
            }
        }
        crate::ue::Property::Map(entries) => {
            for entry in entries.iter_mut() {
                swap_uuid_values_deep_in_property(&mut entry.key, keys, old, new);
                swap_uuid_values_deep_in_property(&mut entry.value, keys, old, new);
            }
        }
        _ => {}
    }
}

/// Walks every `Property` reachable from `properties` and, for each field whose
/// NAME is in `keys` and whose leaf is a uuid string or `Guid` struct equal to
/// `old` or `new`, swaps it to the other — bidirectionally, in one pass.
///
/// On a real save this walk finds nothing, by design: `OwnerPlayerUId` lives
/// inside the typed `PalCharacterData` RawData, and the lowercase ownership
/// keys are typed `FGuid` fields on the map-object models — all
/// `StructValue` variants with no generic `Properties` bag, which the walk
/// stops at. The real uid rewriting is done by `swap_player_uids`'s other
/// steps (`swap_player_gvas_uids`, the character-map key rewrite,
/// `swap_guild_member_uids`, `swap_player_file_refs`). Adding typed-struct
/// arms here would double-swap those fields.
pub fn swap_uuid_values_deep(
    properties: &mut crate::ue::Properties,
    keys: &[&str],
    old: uuid::Uuid,
    new: uuid::Uuid,
) {
    for (property_key, property_value) in properties.0.iter_mut() {
        if keys.contains(&property_key.1.as_str()) {
            swap_leaf_uuid_property(property_value, old, new);
        }
        swap_uuid_values_deep_in_property(property_value, keys, old, new);
    }
}

#[cfg(test)]
mod deep_swap_tests {
    use super::*;
    use crate::ue::{MapEntry, Properties, Property, StructValue, ValueVec};

    const OWNERSHIP_KEYS: [&str; 4] = [
        "OwnerPlayerUId",
        "owner_player_uid",
        "build_player_uid",
        "private_lock_player_uid",
    ];

    fn uid(text: &str) -> uuid::Uuid {
        text.parse().unwrap()
    }

    const OLD: &str = "11111111-1111-1111-1111-111111111111";
    const NEW: &str = "22222222-2222-2222-2222-222222222222";

    #[test]
    fn swaps_guid_struct_leaf_old_to_new() {
        let mut properties = Properties::default();
        properties.insert("OwnerPlayerUId", guid_property(uid(OLD)));

        swap_uuid_values_deep(&mut properties, &OWNERSHIP_KEYS, uid(OLD), uid(NEW));

        assert_eq!(as_uuid(&properties["OwnerPlayerUId"]), Some(uid(NEW)));
    }

    #[test]
    fn swaps_guid_struct_leaf_new_to_old_bidirectionally() {
        let mut properties = Properties::default();
        properties.insert("OwnerPlayerUId", guid_property(uid(NEW)));

        swap_uuid_values_deep(&mut properties, &OWNERSHIP_KEYS, uid(OLD), uid(NEW));

        assert_eq!(as_uuid(&properties["OwnerPlayerUId"]), Some(uid(OLD)));
    }

    #[test]
    fn swaps_str_leaf_case_insensitively() {
        let mut properties = Properties::default();
        properties.insert("owner_player_uid", str_property(&OLD.to_uppercase()));

        swap_uuid_values_deep(&mut properties, &OWNERSHIP_KEYS, uid(OLD), uid(NEW));

        assert_eq!(as_str(&properties["owner_player_uid"]), Some(NEW));
    }

    #[test]
    fn no_match_leaves_value_unchanged() {
        let unrelated = uid("33333333-3333-3333-3333-333333333333");
        let mut properties = Properties::default();
        properties.insert("owner_player_uid", guid_property(unrelated));

        swap_uuid_values_deep(&mut properties, &OWNERSHIP_KEYS, uid(OLD), uid(NEW));

        assert_eq!(as_uuid(&properties["owner_player_uid"]), Some(unrelated));
    }

    #[test]
    fn ignores_non_ownership_keys() {
        let mut properties = Properties::default();
        properties.insert("SomeOtherKey", guid_property(uid(OLD)));

        swap_uuid_values_deep(&mut properties, &OWNERSHIP_KEYS, uid(OLD), uid(NEW));

        assert_eq!(as_uuid(&properties["SomeOtherKey"]), Some(uid(OLD)));
    }

    #[test]
    fn recurses_into_nested_struct() {
        let mut inner = Properties::default();
        inner.insert("owner_player_uid", guid_property(uid(OLD)));
        let mut outer = Properties::default();
        outer.insert("Outer", Property::Struct(StructValue::Struct(inner)));

        swap_uuid_values_deep(&mut outer, &OWNERSHIP_KEYS, uid(OLD), uid(NEW));

        let nested = struct_props(&outer["Outer"]).unwrap();
        assert_eq!(as_uuid(&nested["owner_player_uid"]), Some(uid(NEW)));
    }

    #[test]
    fn recurses_into_array_of_structs_independently_per_element() {
        let mut first = Properties::default();
        first.insert("owner_player_uid", guid_property(uid(OLD)));
        let mut second = Properties::default();
        second.insert("owner_player_uid", guid_property(uid(NEW)));

        let mut properties = Properties::default();
        properties.insert(
            "Items",
            Property::Array(ValueVec::Struct(vec![
                StructValue::Struct(first),
                StructValue::Struct(second),
            ])),
        );

        swap_uuid_values_deep(&mut properties, &OWNERSHIP_KEYS, uid(OLD), uid(NEW));

        let Property::Array(ValueVec::Struct(values)) = &properties["Items"] else {
            panic!("expected Array(Struct)");
        };
        let StructValue::Struct(first_props) = &values[0] else {
            panic!("expected Struct");
        };
        let StructValue::Struct(second_props) = &values[1] else {
            panic!("expected Struct");
        };
        assert_eq!(as_uuid(&first_props["owner_player_uid"]), Some(uid(NEW)));
        assert_eq!(as_uuid(&second_props["owner_player_uid"]), Some(uid(OLD)));
    }

    #[test]
    fn recurses_into_map_key_and_value_structs() {
        let mut key_props = Properties::default();
        key_props.insert("owner_player_uid", guid_property(uid(OLD)));
        let mut value_props = Properties::default();
        value_props.insert("build_player_uid", guid_property(uid(OLD)));

        let mut properties = Properties::default();
        properties.insert(
            "SomeMap",
            Property::Map(vec![MapEntry {
                key: Property::Struct(StructValue::Struct(key_props)),
                value: Property::Struct(StructValue::Struct(value_props)),
            }]),
        );

        swap_uuid_values_deep(&mut properties, &OWNERSHIP_KEYS, uid(OLD), uid(NEW));

        let Property::Map(entries) = &properties["SomeMap"] else {
            panic!("expected Map");
        };
        let key_props = struct_props(&entries[0].key).unwrap();
        let value_props = struct_props(&entries[0].value).unwrap();
        assert_eq!(as_uuid(&key_props["owner_player_uid"]), Some(uid(NEW)));
        assert_eq!(as_uuid(&value_props["build_player_uid"]), Some(uid(NEW)));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ue::{Properties, Property, StructValue};

    fn fguid(text: &str) -> crate::ue::FGuid {
        serde_json::from_value(serde_json::Value::String(text.to_string())).unwrap()
    }

    #[test]
    fn test_get_finds_top_level_property() {
        let mut properties = Properties::default();
        properties.insert("WorldName", Property::Str("MyWorld".to_string()));

        let found = get(&properties, &["WorldName"]).unwrap();
        assert_eq!(Some("MyWorld"), as_str(found));
    }

    #[test]
    fn test_get_returns_none_for_missing_top_level_property() {
        let mut properties = Properties::default();
        properties.insert("WorldName", Property::Str("MyWorld".to_string()));

        assert!(get(&properties, &["DoesNotExist"]).is_none());
    }

    #[test]
    fn test_get_traverses_nested_path() {
        let mut inner = Properties::default();
        inner.insert("Value", Property::Str("nested".to_string()));
        let mut outer = Properties::default();
        outer.insert("Inner", Property::Struct(StructValue::Struct(inner)));

        let found = get(&outer, &["Inner", "Value"]).unwrap();
        assert_eq!(Some("nested"), as_str(found));
    }

    #[test]
    fn test_as_uuid_extracts_guid() {
        let uuid_text = "0b1c2d3e-1111-2222-3333-444455556666";
        let property = Property::Struct(StructValue::Guid(fguid(uuid_text)));

        assert_eq!(uuid_text, as_uuid(&property).unwrap().to_string());
    }

    #[test]
    fn test_as_uuid_returns_none_for_non_guid_property() {
        assert!(as_uuid(&Property::Bool(true)).is_none());
    }
}

#[cfg(test)]
mod extension_tests {
    use super::*;
    use crate::ue::{ByteArray, Properties, Property, StructValue, ValueVec};

    fn fguid(text: &str) -> crate::ue::FGuid {
        serde_json::from_value(serde_json::Value::String(text.to_string())).unwrap()
    }

    #[test]
    fn test_fguid_to_uuid_roundtrips_display() {
        let uuid_text = "0b1c2d3e-1111-2222-3333-444455556666";
        let converted = fguid_to_uuid(&fguid(uuid_text));
        assert_eq!(uuid_text, converted.to_string());
    }

    #[test]
    fn test_get_in_walks_nested_structs() {
        let mut inner = Properties::default();
        inner.insert(
            "ID",
            Property::Struct(StructValue::Guid(fguid(
                "0b1c2d3e-1111-2222-3333-444455556666",
            ))),
        );
        let mut outer = Properties::default();
        outer.insert("ContainerId", Property::Struct(StructValue::Struct(inner)));
        let root = Property::Struct(StructValue::Struct(outer));

        let found = get_in(&root, &["ContainerId", "ID"]).unwrap();
        assert_eq!(
            "0b1c2d3e-1111-2222-3333-444455556666",
            as_uuid(found).unwrap().to_string()
        );
        assert!(get_in(&root, &["ContainerId", "Missing"]).is_none());
    }

    #[test]
    fn test_struct_properties_returns_none_for_non_struct() {
        assert!(struct_properties(&Property::Bool(true)).is_none());
    }

    #[test]
    fn test_scalar_accessors() {
        assert_eq!(Some("hello"), as_str(&Property::Str("hello".to_string())));
        assert_eq!(Some("hello"), as_str(&Property::Name("hello".to_string())));
        assert_eq!(None, as_str(&Property::Bool(true)));
        assert_eq!(Some(true), as_bool(&Property::Bool(true)));
        assert_eq!(None, as_bool(&Property::Str("hello".to_string())));
        assert_eq!(
            Some("EPalGroupType::Guild"),
            as_enum(&Property::Enum("EPalGroupType::Guild".to_string()))
        );
        assert_eq!(None, as_enum(&Property::Bool(true)));
        assert_eq!(Some(42), as_byte(&Property::Byte(crate::ue::Byte::Byte(42))));
        assert_eq!(
            None,
            as_byte(&Property::Byte(crate::ue::Byte::Label("None".to_string())))
        );
        assert_eq!(
            Some(638400000000000000),
            as_datetime_ticks(&Property::Struct(StructValue::DateTime(638400000000000000)))
        );
        assert_eq!(
            None,
            as_datetime_ticks(&Property::Struct(StructValue::Guid(fguid(
                "0b1c2d3e-1111-2222-3333-444455556666"
            ))))
        );
        assert_eq!(
            Some(&[1u8, 2, 3][..]),
            as_byte_array(&Property::Array(ValueVec::Byte(ByteArray::Byte(vec![
                1, 2, 3
            ]))))
        );
        assert_eq!(
            None,
            as_byte_array(&Property::Array(ValueVec::Byte(ByteArray::Label(vec![
                "Label".to_string()
            ]))))
        );
        assert!(map_entries(&Property::Map(vec![])).is_some());
        assert!(map_entries(&Property::Bool(false)).is_none());
    }
}

#[cfg(test)]
mod phase2_tests {
    use super::*;
    use crate::ue::{Byte, MapEntry, Properties, Property, StructValue, ValueVec};

    fn fguid(text: &str) -> crate::ue::FGuid {
        serde_json::from_value(serde_json::Value::String(text.to_string())).unwrap()
    }

    fn sample_tag() -> crate::ue::PropertyTagPartial {
        crate::ue::PropertyTagPartial {
            id: None,
            data: crate::ue::PropertyTagDataPartial::Other(crate::ue::PropertyType::BoolProperty),
        }
    }

    fn empty_save() -> crate::ue::Save {
        crate::ue::Save {
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
        }
    }

    #[test]
    fn empty_uuid_is_nil() {
        assert_eq!(EMPTY_UUID, uuid::Uuid::nil());
    }

    #[test]
    fn uuid_guid_round_trip() {
        let original = uuid::Uuid::parse_str("11111111-2222-3333-4444-555555555555").unwrap();
        assert_eq!(guid_to_uuid(&uuid_to_guid(original)), original);
    }

    #[test]
    fn guid_to_uuid_matches_phase1_fguid_to_uuid() {
        // guid_to_uuid differs from fguid_to_uuid only in not panicking on
        // malformed input; the conversion itself must be identical.
        let guid = fguid("0b1c2d3e-1111-2222-3333-444455556666");
        assert_eq!(guid_to_uuid(&guid), fguid_to_uuid(&guid));
    }

    #[test]
    fn scalar_accessors_read_expected_variants() {
        assert_eq!(as_str(&name_property("Sheepball")), Some("Sheepball"));
        assert_eq!(as_str(&str_property("nick")), Some("nick"));
        assert_eq!(
            as_str(&enum_property("EPalGenderType::Female")),
            Some("EPalGenderType::Female")
        );
        assert_eq!(
            as_enum(&enum_property("EPalGenderType::Female")),
            Some("EPalGenderType::Female")
        );
        assert_eq!(as_str(&Property::Bool(true)), None);
        assert_eq!(as_i32(&int_property(7)), Some(7));
        assert_eq!(as_i32(&Property::Bool(true)), None);
        assert_eq!(as_i64(&int64_property(1_234_567)), Some(1_234_567));
        assert_eq!(as_i64(&int_property(7)), Some(7));
        assert_eq!(as_i64(&Property::Bool(true)), None);
        assert_eq!(as_f32(&float_property(1.5)), Some(1.5));
        assert_eq!(as_f32(&Property::Bool(true)), None);
        assert_eq!(as_bool(&bool_property(true)), Some(true));
        assert_eq!(as_byte_number(&byte_property(42)), Some(42));
        assert_eq!(
            as_byte_number(&Property::Byte(Byte::Label("None".to_string()))),
            None
        );
    }

    #[test]
    fn array_accessors_read_full_contents() {
        let names = name_array_property(vec!["a".into(), "b".into()]);
        assert_eq!(
            name_values(&names).unwrap(),
            &vec!["a".to_string(), "b".to_string()]
        );
        assert!(name_values(&Property::Bool(true)).is_none());

        let enums = enum_array_property(vec!["EPalWazaID::Unique_SheepBall_Roll".into()]);
        assert_eq!(
            enum_values(&enums).unwrap(),
            &vec!["EPalWazaID::Unique_SheepBall_Roll".to_string()]
        );
        assert!(enum_values(&Property::Bool(true)).is_none());
    }

    #[test]
    fn struct_values_mut_supports_pushing_and_reading_back() {
        let mut property = Property::Array(ValueVec::Struct(Vec::new()));
        struct_values_mut(&mut property)
            .expect("Array(Struct) property yields Some")
            .push(StructValue::Guid(fguid(
                "11111111-2222-3333-4444-555555555555",
            )));

        let values = struct_values(&property).expect("Array(Struct) property yields Some");
        assert_eq!(values.len(), 1);
        assert!(matches!(values[0], StructValue::Guid(_)));
        assert!(struct_values(&Property::Bool(true)).is_none());
        assert!(struct_values_mut(&mut Property::Bool(true)).is_none());
    }

    #[test]
    fn struct_props_mut_allows_inserting_a_field_visible_to_struct_props() {
        let mut property = Property::Struct(StructValue::Struct(Properties::default()));
        struct_props_mut(&mut property)
            .expect("Struct property yields Some")
            .insert("Added", Property::Bool(true));

        let inner = struct_props(&property).expect("Struct property yields Some");
        assert_eq!(
            as_bool(inner.0.get(&crate::ue::PropertyKey::from("Added")).unwrap()),
            Some(true)
        );
        assert!(struct_props(&Property::Bool(true)).is_none());
        assert!(struct_props_mut(&mut Property::Bool(true)).is_none());
    }

    #[test]
    fn map_entries_mut_supports_pushing_and_reading_back() {
        let mut property = Property::Map(Vec::new());
        map_entries_mut(&mut property)
            .expect("Map property yields Some")
            .push(MapEntry {
                key: bool_property(true),
                value: int_property(9),
            });

        let entries = map_entries(&property).expect("Map property yields Some");
        assert_eq!(entries.len(), 1);
        assert_eq!(as_bool(&entries[0].key), Some(true));
        assert_eq!(as_i32(&entries[0].value), Some(9));
        assert!(map_entries(&Property::Bool(true)).is_none());
        assert!(map_entries_mut(&mut Property::Bool(true)).is_none());
    }

    #[test]
    fn guid_property_round_trips_through_as_uuid() {
        let original = uuid::Uuid::parse_str("aaaaaaaa-bbbb-cccc-dddd-eeeeeeeeeeee").unwrap();
        let property = guid_property(original);
        assert_eq!(as_uuid(&property), Some(original));
    }

    #[test]
    fn fixed_point64_round_trip() {
        let property = fixed_point64_property(545_000);
        assert_eq!(fixed_point64(&property), Some(545_000));
    }

    #[test]
    fn fixed_point64_returns_none_for_missing_or_wrong_typed_value_field() {
        let no_value_field = Property::Struct(StructValue::Struct(Properties::default()));
        assert_eq!(fixed_point64(&no_value_field), None);

        let mut wrong_type = Properties::default();
        wrong_type.insert("Value", Property::Bool(true));
        let wrong_type_property = Property::Struct(StructValue::Struct(wrong_type));
        assert_eq!(fixed_point64(&wrong_type_property), None);

        assert_eq!(fixed_point64(&Property::Bool(true)), None);
    }

    #[test]
    fn get_mut_navigates_mutates_and_is_visible_through_get() {
        let mut inner = Properties::default();
        inner.insert("Value", Property::Str("before".to_string()));
        let mut outer = Properties::default();
        outer.insert("Inner", Property::Struct(StructValue::Struct(inner)));

        let found = get_mut(&mut outer, &["Inner", "Value"]).expect("get_mut finds nested node");
        *found = Property::Str("after".to_string());

        let read_back = get(&outer, &["Inner", "Value"]).expect("get finds the mutated node");
        assert_eq!(Some("after"), as_str(read_back));
    }

    #[test]
    fn get_mut_returns_none_for_missing_or_wrong_typed_path() {
        let mut properties = Properties::default();
        properties.insert("Present", Property::Bool(true));

        assert!(get_mut(&mut properties, &["Missing"]).is_none());
        // "Present" exists but isn't a struct, so descending into it fails.
        assert!(get_mut(&mut properties, &["Present", "Nested"]).is_none());
    }

    #[test]
    fn struct_props_reads_nested_struct() {
        let property = fixed_point64_property(9);
        let inner = struct_props(&property).expect("nested Properties");
        assert!(matches!(inner["Value"], Property::Int64(9)));
    }

    #[test]
    fn schema_prefix_ending_with_finds_and_strips_matching_suffix() {
        let mut save = empty_save();
        save.schemas.record(
            "worldSaveData.CharacterSaveParameterMap.0.Value.RawData".to_string(),
            sample_tag(),
        );

        assert_eq!(
            schema_prefix_ending_with(&save, ".Value.RawData"),
            Some("worldSaveData.CharacterSaveParameterMap.0".to_string())
        );
        assert_eq!(schema_prefix_ending_with(&save, ".NoSuchSuffix"), None);
    }

    #[test]
    fn ensure_schema_records_once_and_never_overwrites() {
        let mut save = empty_save();
        let first_tag = sample_tag();
        ensure_schema(&mut save, "Foo.Bar".to_string(), first_tag.clone());
        assert_eq!(save.schemas.get("Foo.Bar"), Some(&first_tag));

        let second_tag = crate::ue::PropertyTagPartial {
            id: None,
            data: crate::ue::PropertyTagDataPartial::Other(crate::ue::PropertyType::IntProperty),
        };
        ensure_schema(&mut save, "Foo.Bar".to_string(), second_tag);
        assert_eq!(
            save.schemas.get("Foo.Bar"),
            Some(&first_tag),
            "ensure_schema must not overwrite an existing schema"
        );
    }
}
