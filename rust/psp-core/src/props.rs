//! Ergonomic accessors into `uesave`'s dynamic `Property` tree.
//!
//! The port types typed `uesave` structs first; these functions are the
//! fallback for reaching into a save's property tree when no typed struct
//! exists for a node yet (spec: "typed structs first, tree-access
//! fallback"). They return `Option` — a missing or wrong-typed node in an
//! untrusted save file is a normal condition, not a bug. These accessors
//! are too low-level to carry a path in their failure; callers that need
//! to surface a failure to the user should name the path in their own
//! error (e.g. `CoreError`) when a lookup comes back `None`.

/// Look up a top-level property by name inside a `Properties` map — the
/// entry point into the dynamic property tree. When `path` has more than
/// one segment, continues through nested user structs (see `get_in`).
pub fn get<'a>(properties: &'a uesave::Properties, path: &[&str]) -> Option<&'a uesave::Property> {
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

/// Walk a nested user-struct chain starting from a property (complement of
/// `get`, which starts from a `Properties` map).
pub fn get_in<'a>(property: &'a uesave::Property, path: &[&str]) -> Option<&'a uesave::Property> {
    let mut current = property;
    for segment in path {
        current = get(struct_properties(current)?, &[segment])?;
    }
    Some(current)
}

/// The nested `Properties` map of a user-struct property (e.g. a struct
/// field like `ContainerId` whose value is itself a bag of properties).
pub fn struct_properties(property: &uesave::Property) -> Option<&uesave::Properties> {
    match property {
        uesave::Property::Struct(uesave::StructValue::Struct(properties)) => Some(properties),
        _ => None,
    }
}

/// A `Str`, `Name`, or `Enum` property's text. Widened from Phase 1's
/// `Str | Name` to also cover `Enum` (Phase 2 constructors/DTOs read enum
/// variant names through the same accessor as free-text fields).
pub fn as_str(property: &uesave::Property) -> Option<&str> {
    match property {
        uesave::Property::Str(text)
        | uesave::Property::Name(text)
        | uesave::Property::Enum(text) => Some(text),
        _ => None,
    }
}

/// A `Bool` property's value.
pub fn as_bool(property: &uesave::Property) -> Option<bool> {
    match property {
        uesave::Property::Bool(value) => Some(*value),
        _ => None,
    }
}

/// An `Enum` property's fully qualified variant name (e.g.
/// `"EPalGroupType::Guild"`).
pub fn as_enum(property: &uesave::Property) -> Option<&str> {
    match property {
        uesave::Property::Enum(value) => Some(value),
        _ => None,
    }
}

/// A raw `Byte` property's value. Returns `None` for an enum-backed byte
/// (`Byte::Label`) — use `as_enum` for those.
pub fn as_byte(property: &uesave::Property) -> Option<u8> {
    match property {
        uesave::Property::Byte(uesave::Byte::Byte(value)) => Some(*value),
        _ => None,
    }
}

/// .NET-style ticks from a `DateTime` struct property (player .sav
/// "Timestamp").
pub fn as_datetime_ticks(property: &uesave::Property) -> Option<u64> {
    match property {
        uesave::Property::Struct(uesave::StructValue::DateTime(ticks)) => Some(*ticks),
        _ => None,
    }
}

/// A `Map` property's entries. Widened from Phase 1's `&[MapEntry]` to
/// `&Vec<MapEntry>` (matching `Property::Map`'s own field type exactly, no
/// slice narrowing) so the Phase-2 mutable counterpart (`map_entries_mut`)
/// can support `push`/`retain`/`remove` for pal/container CRUD. Existing
/// Phase-1 callers keep compiling unchanged: `&Vec<T>` coerces to `&[T]` at
/// every one of their call sites (direct function-argument passing).
pub fn map_entries(property: &uesave::Property) -> Option<&Vec<uesave::MapEntry>> {
    match property {
        uesave::Property::Map(entries) => Some(entries),
        _ => None,
    }
}

/// Mutable counterpart of `map_entries` (Phase 2: pal/container CRUD needs
/// to push/remove map entries in place).
pub fn map_entries_mut(property: &mut uesave::Property) -> Option<&mut Vec<uesave::MapEntry>> {
    match property {
        uesave::Property::Map(entries) => Some(entries),
        _ => None,
    }
}

/// A byte-array `Array` property's contents (e.g. `RawData`). Returns
/// `None` for an enum-labeled byte array.
pub fn as_byte_array(property: &uesave::Property) -> Option<&[u8]> {
    match property {
        uesave::Property::Array(uesave::ValueVec::Byte(uesave::ByteArray::Byte(bytes))) => {
            Some(bytes)
        }
        _ => None,
    }
}

/// `FGuid`'s `Display` already renders the Palworld byte order as a
/// canonical UUID string (verified equal to Python
/// `palworld_save_tools`'s `uuid_reader`).
pub fn fguid_to_uuid(guid: &uesave::FGuid) -> uuid::Uuid {
    guid.to_string()
        .parse()
        .expect("FGuid Display always yields a canonical uuid")
}

/// A property's UUID value, if it is a `Guid` struct.
pub fn as_uuid(property: &uesave::Property) -> Option<uuid::Uuid> {
    match property {
        uesave::Property::Struct(uesave::StructValue::Guid(guid)) => Some(fguid_to_uuid(guid)),
        _ => None,
    }
}

// ============================================================================
// Phase 2 — accessors, constructors, and schema management (edit core).
//
// A constructor built here produces a `uesave::Property` that later gets
// re-serialized into a save on disk. Every constructor is paired with the
// accessor that parses its shape back out, and the round-trip is asserted
// in `phase2_tests` below (construct -> parse -> equals input) — a
// constructor that only "compiles" is not considered done for this port.
// ============================================================================

/// The canonical nil UUID (`00000000-0000-0000-0000-000000000000`) —
/// Palworld's sentinel for "no owner"/"no group" in several property slots.
pub const EMPTY_UUID: uuid::Uuid = uuid::Uuid::nil();

/// Converts an `FGuid` to a `Uuid` without panicking. Save data reaching
/// this accessor is untrusted, so unlike Phase 1's `fguid_to_uuid` (which
/// `.expect()`s), this falls back to the nil UUID if `FGuid`'s Display
/// output ever failed to parse — a case that should be unreachable in
/// practice (`FGuid`'s four `u32` fields always format to exactly 32 hex
/// digits, per `fguid_to_uuid`'s own established parity with Python's
/// `uuid_reader`), but a defensive accessor must not panic on the
/// unreachable case either. Prefer this over `fguid_to_uuid` in new Phase-2
/// code.
pub fn guid_to_uuid(guid: &uesave::FGuid) -> uuid::Uuid {
    guid.to_string().parse().unwrap_or(uuid::Uuid::nil())
}

/// Converts a `Uuid` to an `FGuid` for writing back into a save. Falls back
/// to the nil `FGuid` on the same unreachable parse failure as
/// `guid_to_uuid` (a `Uuid`'s canonical string form is always 32 valid hex
/// digits, which `FGuid::parse_str` accepts).
pub fn uuid_to_guid(value: uuid::Uuid) -> uesave::FGuid {
    uesave::FGuid::parse_str(&value.to_string()).unwrap_or_else(|_| uesave::FGuid::nil())
}

/// The nested `Properties` map of a user-struct property. Phase-2 name for
/// `struct_properties` — later Phase-2/3/4 tasks are written against this
/// name; kept as a thin alias rather than renaming `struct_properties`
/// (Phase-1 callers already depend on that name).
pub fn struct_props(property: &uesave::Property) -> Option<&uesave::Properties> {
    struct_properties(property)
}

/// Mutable counterpart of `struct_props`/`struct_properties` (Phase 1 only
/// ever needed read access).
pub fn struct_props_mut(property: &mut uesave::Property) -> Option<&mut uesave::Properties> {
    match property {
        uesave::Property::Struct(uesave::StructValue::Struct(properties)) => Some(properties),
        _ => None,
    }
}

/// An `Int` property's value.
pub fn as_i32(property: &uesave::Property) -> Option<i32> {
    match property {
        uesave::Property::Int(value) => Some(*value),
        _ => None,
    }
}

/// An `Int64` property's value. Also widens a plain `Int`, since Palworld
/// writes some numeric fields (engine-version-dependent) as one or the
/// other.
pub fn as_i64(property: &uesave::Property) -> Option<i64> {
    match property {
        uesave::Property::Int64(value) => Some(*value),
        uesave::Property::Int(value) => Some(*value as i64),
        _ => None,
    }
}

/// A `Float` property's value.
pub fn as_f32(property: &uesave::Property) -> Option<f32> {
    match property {
        uesave::Property::Float(uesave::Float(value)) => Some(*value),
        _ => None,
    }
}

/// A raw `Byte` property's value. Phase-2 name for `as_byte` — later tasks
/// are written against this name.
pub fn as_byte_number(property: &uesave::Property) -> Option<u8> {
    as_byte(property)
}

/// An `Array(Name)` property's contents.
pub fn name_values(property: &uesave::Property) -> Option<&Vec<String>> {
    match property {
        uesave::Property::Array(uesave::ValueVec::Name(values)) => Some(values),
        _ => None,
    }
}

/// An `Array(Enum)` property's contents.
pub fn enum_values(property: &uesave::Property) -> Option<&Vec<String>> {
    match property {
        uesave::Property::Array(uesave::ValueVec::Enum(values)) => Some(values),
        _ => None,
    }
}

/// An `Array(Struct)` property's contents.
pub fn struct_values(property: &uesave::Property) -> Option<&Vec<uesave::StructValue>> {
    match property {
        uesave::Property::Array(uesave::ValueVec::Struct(values)) => Some(values),
        _ => None,
    }
}

/// Mutable counterpart of `struct_values`.
pub fn struct_values_mut(property: &mut uesave::Property) -> Option<&mut Vec<uesave::StructValue>> {
    match property {
        uesave::Property::Array(uesave::ValueVec::Struct(values)) => Some(values),
        _ => None,
    }
}

/// A `FixedPoint64`-shaped user struct's decoded value — Palworld's
/// fixed-point encoding for several stat fields, always a bare
/// `{"Value": Int64(n)}`. Uses `Properties::get` (never the panicking
/// `Index` impl) since a malformed save may be missing the field.
pub fn fixed_point64(property: &uesave::Property) -> Option<i64> {
    let inner = struct_props(property)?;
    as_i64(inner.0.get(&uesave::PropertyKey::from("Value"))?)
}

// ---- constructors (mirror PalObjects.*Property builders) ----

/// `Struct(Guid)` — the shape `uesave` both reads and writes for a `Guid`
/// struct property (see `StructValue::Guid` in `uesave/src/lib.rs`).
pub fn guid_property(value: uuid::Uuid) -> uesave::Property {
    uesave::Property::Struct(uesave::StructValue::Guid(uuid_to_guid(value)))
}

pub fn str_property(value: &str) -> uesave::Property {
    uesave::Property::Str(value.to_string())
}

pub fn name_property(value: &str) -> uesave::Property {
    uesave::Property::Name(value.to_string())
}

/// `value` must be the fully qualified enum variant name (e.g.
/// `"EPalGenderType::Female"`), matching what `as_str`/`as_enum` read back.
pub fn enum_property(value: &str) -> uesave::Property {
    uesave::Property::Enum(value.to_string())
}

pub fn bool_property(value: bool) -> uesave::Property {
    uesave::Property::Bool(value)
}

pub fn int_property(value: i32) -> uesave::Property {
    uesave::Property::Int(value)
}

pub fn int64_property(value: i64) -> uesave::Property {
    uesave::Property::Int64(value)
}

pub fn float_property(value: f32) -> uesave::Property {
    uesave::Property::Float(uesave::Float(value))
}

pub fn byte_property(value: u8) -> uesave::Property {
    uesave::Property::Byte(uesave::Byte::Byte(value))
}

pub fn name_array_property(values: Vec<String>) -> uesave::Property {
    uesave::Property::Array(uesave::ValueVec::Name(values))
}

pub fn enum_array_property(values: Vec<String>) -> uesave::Property {
    uesave::Property::Array(uesave::ValueVec::Enum(values))
}

/// Inverse of `fixed_point64`: wraps `value` in the `{"Value": Int64(n)}`
/// user struct Palworld's fixed-point fields expect.
pub fn fixed_point64_property(value: i64) -> uesave::Property {
    let mut inner = uesave::Properties::default();
    inner.insert("Value", uesave::Property::Int64(value));
    uesave::Property::Struct(uesave::StructValue::Struct(inner))
}

// ---- schema management ----
//
// uesave's writer looks up a property's schema by its exact dotted scope
// path and refuses to write (`Error::MissingPropertySchema`) when none is
// recorded (see `write_property` in uesave/src/lib.rs). Every property NAME
// this port newly introduces into a save — as opposed to only ever
// overwriting an already-present one, which already carries a schema
// recorded during the original read — must have a schema registered before
// write.

/// Finds a recorded schema path ending with `suffix` and returns everything
/// before the suffix. Used to derive a schema for a brand-new sibling
/// property (e.g. a field this port adds to every pal entry) by copying the
/// shape already recorded for an existing sibling at the same tree
/// position, since there is by definition no schema yet at the exact new
/// path.
pub fn schema_prefix_ending_with(save: &uesave::Save, suffix: &str) -> Option<String> {
    save.schemas
        .schemas()
        .keys()
        .find(|key| key.ends_with(suffix))
        .map(|key| key[..key.len() - suffix.len()].to_string())
}

/// Records `tag` at `path` if no schema exists there yet; a no-op when one
/// is already recorded (never overwrites — the existing schema was either
/// recorded from the real save during read, or already `ensure_schema`d by
/// an earlier call for this same path).
pub fn ensure_schema(save: &mut uesave::Save, path: String, tag: uesave::PropertyTagPartial) {
    if save.schemas.get(&path).is_none() {
        save.schemas.record(path, tag);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use uesave::{Properties, Property, StructValue};

    fn fguid(text: &str) -> uesave::FGuid {
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
    use uesave::{ByteArray, Properties, Property, StructValue, ValueVec};

    fn fguid(text: &str) -> uesave::FGuid {
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
        assert_eq!(Some(42), as_byte(&Property::Byte(uesave::Byte::Byte(42))));
        assert_eq!(
            None,
            as_byte(&Property::Byte(uesave::Byte::Label("None".to_string())))
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
    use uesave::{Byte, MapEntry, Properties, Property, StructValue, ValueVec};

    fn fguid(text: &str) -> uesave::FGuid {
        serde_json::from_value(serde_json::Value::String(text.to_string())).unwrap()
    }

    fn sample_tag() -> uesave::PropertyTagPartial {
        uesave::PropertyTagPartial {
            id: None,
            data: uesave::PropertyTagDataPartial::Other(uesave::PropertyType::BoolProperty),
        }
    }

    fn empty_save() -> uesave::Save {
        uesave::Save {
            header: uesave::Header {
                magic: 0,
                save_game_version: 0,
                package_version: uesave::PackageVersion { ue4: 0, ue5: None },
                engine_version_major: 0,
                engine_version_minor: 0,
                engine_version_patch: 0,
                engine_version_build: 0,
                engine_version: String::new(),
                custom_version: None,
            },
            schemas: uesave::PropertySchemas::default(),
            root: uesave::Root {
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
        // guid_to_uuid must not silently diverge from the already
        // parity-verified Phase-1 conversion (fguid_to_uuid), just drop its
        // panic-on-malformed-input behavior.
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
        // as_i64 also widens a plain Int, since some fields are written as
        // either depending on engine version.
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
            as_bool(inner.0.get(&uesave::PropertyKey::from("Added")).unwrap()),
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

        let second_tag = uesave::PropertyTagPartial {
            id: None,
            data: uesave::PropertyTagDataPartial::Other(uesave::PropertyType::IntProperty),
        };
        ensure_schema(&mut save, "Foo.Bar".to_string(), second_tag);
        assert_eq!(
            save.schemas.get("Foo.Bar"),
            Some(&first_tag),
            "ensure_schema must not overwrite an existing schema"
        );
    }
}
