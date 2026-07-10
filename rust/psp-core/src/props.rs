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

/// A `Str` or `Name` property's text.
pub fn as_str(property: &uesave::Property) -> Option<&str> {
    match property {
        uesave::Property::Str(text) | uesave::Property::Name(text) => Some(text),
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

/// A `Map` property's entries.
pub fn map_entries(property: &uesave::Property) -> Option<&[uesave::MapEntry]> {
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
