//! Guild-lookup helpers shared by pal-summary extraction (`domain::pal`,
//! Task 5) and guild detail loading (Task 8).

use crate::props;

/// From a `BaseCampSaveData` entry: `(group_id_belong_to, WorkerDirector
/// container_id)`. Python paths: `value.RawData.value.group_id_belong_to`
/// and `value.WorkerDirector.value.RawData.value.container_id`
/// (`game/mixins/loading.py`'s `_load_base_camps`,
/// `game/mixins/summaries.py`'s `_build_base_container_map`).
///
/// Deviation from the brief: the brief's version of this function matched on
/// `Property::Struct(StructValue::PalWorkerDirector(director))` and read
/// `director.container_id`. Neither that variant nor that struct exists in
/// `uesave-rs`. The API-shape checkpoint the brief called out was
/// necessary but insufficient -- the real gap is one level up: `../uesave-
/// rs/uesave/src/games/palworld/mod.rs` registers
/// `worldSaveData.BaseCampSaveData.WorkerDirector.RawData` in its
/// `struct_hints` list as a generic `StructType::Struct(None)`, and
/// `is_pal_struct_type` (same file) does not recognize `Struct(None)` as
/// Palworld-embedded data -- so `process_property_for_read` never attempts
/// to decode that byte array at all. The property survives parsing as a
/// plain, undecoded `Property::Array(ValueVec::Byte(ByteArray::Byte(bytes)))`,
/// not any `StructValue` variant, typed or otherwise. Phase 1 already solved
/// exactly this for `domain::summaries::guild_worker_container_ids`:
/// `palbin::worker_director_container_id` is a bounds-checked, already-
/// tested parser for this exact fixed 118-byte layout
/// (`palworld_save_tools/rawdata/worker_director.py`'s `decode_bytes`) --
/// this function reuses it rather than reinventing a byte parser or
/// depending on a struct that doesn't exist.
pub fn base_guild_and_container(entry: &uesave::MapEntry) -> Option<(uuid::Uuid, uuid::Uuid)> {
    let value_properties = props::struct_props(&entry.value)?;
    let raw_data = props::get(value_properties, &["RawData"])?;
    let uesave::Property::Struct(uesave::StructValue::PalBaseCamp(base_camp)) = raw_data else {
        return None;
    };
    let guild_id = props::guid_to_uuid(&base_camp.group_id_belong_to);

    let worker_director_blob = props::get(value_properties, &["WorkerDirector", "RawData"])
        .and_then(props::as_byte_array)?;
    let container_id = crate::palbin::worker_director_container_id(worker_director_blob).ok()?;

    Some((guild_id, container_id))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::palbin::test_bytes::shuffle_guid_bytes;
    use uesave::games::palworld::{PalBaseCamp, PalTransform};
    use uesave::{
        ByteArray, Double, MapEntry, Properties, Property, Quat, StructValue, ValueVec, Vector,
    };

    const GUILD_ID: &str = "33333333-3333-3333-3333-333333333333";
    const BASE_ID: &str = "44444444-4444-4444-4444-444444444444";
    const CONTAINER_ID: &str = "55555555-5555-5555-5555-555555555555";

    fn fguid(text: &str) -> uesave::FGuid {
        serde_json::from_value(serde_json::Value::String(text.to_string())).unwrap()
    }

    fn guid_property(text: &str) -> Property {
        Property::Struct(StructValue::Guid(fguid(text)))
    }

    fn zero_transform() -> PalTransform {
        PalTransform {
            rotation: Quat {
                x: Double(0.0),
                y: Double(0.0),
                z: Double(0.0),
                w: Double(1.0),
            },
            translation: Vector {
                x: Double(0.0),
                y: Double(0.0),
                z: Double(0.0),
            },
            scale: Vector {
                x: Double(1.0),
                y: Double(1.0),
                z: Double(1.0),
            },
        }
    }

    fn worker_director_blob(container_id: &str) -> Vec<u8> {
        let mut blob = vec![0u8; 118];
        let display_bytes = *container_id.parse::<uuid::Uuid>().unwrap().as_bytes();
        blob[98..114].copy_from_slice(&shuffle_guid_bytes(display_bytes));
        blob
    }

    fn base_camp_entry(base_id: &str, guild_id: &str, worker_container_id: &str) -> MapEntry {
        let camp = PalBaseCamp {
            id: fguid(base_id),
            name: String::new(),
            state: 0,
            transform: zero_transform(),
            area_range: 0.0,
            group_id_belong_to: fguid(guild_id),
            fast_travel_local_transform: zero_transform(),
            owner_map_object_instance_id: uesave::FGuid::nil(),
            trailing_bytes: [0; 4],
        };
        let mut worker_properties = Properties::default();
        worker_properties.insert(
            "RawData",
            Property::Array(ValueVec::Byte(ByteArray::Byte(worker_director_blob(
                worker_container_id,
            )))),
        );
        let mut value_properties = Properties::default();
        value_properties.insert(
            "RawData",
            Property::Struct(StructValue::PalBaseCamp(Box::new(camp))),
        );
        value_properties.insert(
            "WorkerDirector",
            Property::Struct(StructValue::Struct(worker_properties)),
        );
        MapEntry {
            key: guid_property(base_id),
            value: Property::Struct(StructValue::Struct(value_properties)),
        }
    }

    #[test]
    fn base_guild_and_container_resolves_both_ids() {
        let entry = base_camp_entry(BASE_ID, GUILD_ID, CONTAINER_ID);

        let (guild_id, container_id) = base_guild_and_container(&entry).unwrap();

        assert_eq!(GUILD_ID, guild_id.to_string());
        assert_eq!(CONTAINER_ID, container_id.to_string());
    }

    #[test]
    fn base_guild_and_container_returns_none_when_raw_data_is_the_wrong_variant() {
        let mut value_properties = Properties::default();
        value_properties.insert("RawData", Property::Bool(true));
        let entry = MapEntry {
            key: guid_property(BASE_ID),
            value: Property::Struct(StructValue::Struct(value_properties)),
        };

        assert!(base_guild_and_container(&entry).is_none());
    }

    #[test]
    fn base_guild_and_container_returns_none_when_worker_director_blob_is_wrong_length() {
        let camp = PalBaseCamp {
            id: fguid(BASE_ID),
            name: String::new(),
            state: 0,
            transform: zero_transform(),
            area_range: 0.0,
            group_id_belong_to: fguid(GUILD_ID),
            fast_travel_local_transform: zero_transform(),
            owner_map_object_instance_id: uesave::FGuid::nil(),
            trailing_bytes: [0; 4],
        };
        let mut worker_properties = Properties::default();
        worker_properties.insert(
            "RawData",
            Property::Array(ValueVec::Byte(ByteArray::Byte(vec![0u8; 10]))),
        );
        let mut value_properties = Properties::default();
        value_properties.insert(
            "RawData",
            Property::Struct(StructValue::PalBaseCamp(Box::new(camp))),
        );
        value_properties.insert(
            "WorkerDirector",
            Property::Struct(StructValue::Struct(worker_properties)),
        );
        let entry = MapEntry {
            key: guid_property(BASE_ID),
            value: Property::Struct(StructValue::Struct(value_properties)),
        };

        assert!(base_guild_and_container(&entry).is_none());
    }
}
