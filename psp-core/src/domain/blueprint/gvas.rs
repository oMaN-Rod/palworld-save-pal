//! The blueprint's wire form: a synthetic `Save<Palworld>`. `.psp` is that
//! `Save`'s binary encoding (the same GVAS container real saves use); JSON is
//! `serde_json` over the identical object. One object, two encodings -- see
//! the amendment on this task for why that is load-bearing, not incidental.

use super::{BaseBlueprint, BlueprintHeader, BlueprintStructure, SCHEMA_VERSION};
use crate::error::CoreError;
use crate::props;
use crate::savio;
use crate::ue::games::palworld::{PalDynamicItemType, PalMapConcreteModelVariant, PalTransform};
use crate::ue::{
    Byte, FGuid, MapEntry, PalStruct, PropertyKey, PropertyTagDataPartial, PropertyTagPartial,
    PropertyType, Save, StructType, StructValue, ValueVec,
};
use crate::ue::{Properties, Property};

pub const PSP_MAGIC: &[u8; 8] = b"PSPBP1\0\0";

/// The `Root::save_game_type` every blueprint carries. Checked on decode so a
/// foreign `Save` cannot be read as a blueprint through the JSON door.
pub const SAVE_GAME_TYPE: &str = "PspBaseBlueprint";

/// The blueprint's payload lives under the SAME property names the game's own
/// `Level.sav` uses, nested under the same `worldSaveData` root struct.
///
/// This is not cosmetic. `savio::read_sav_bytes` installs
/// `palworld_types()`, whose every hint is keyed `worldSaveData.*`, and
/// Palworld's context-dependent parsers (map objects, works) are gated on the
/// literal paths `worldSaveData.MapObjectSaveData` and
/// `worldSaveData.WorkSaveData`. A blueprint written at any other path reads
/// back byte-identical but *typeless*: every `Model`/`ConcreteModel`/`Work`/
/// character `RawData` stays an opaque byte array, and every consumer that
/// pattern-matches `StructValue::Game(PalStruct::..)` silently sees nothing.
/// Borrowing the game's own paths makes every hint and both context parsers
/// apply for free.
const WORLD_SAVE_DATA: &str = "worldSaveData";
const MAP_OBJECT_SAVE_DATA: &str = "MapObjectSaveData";
const ITEM_CONTAINER_SAVE_DATA: &str = "ItemContainerSaveData";
const CHARACTER_CONTAINER_SAVE_DATA: &str = "CharacterContainerSaveData";
const CHARACTER_SAVE_PARAMETER_MAP: &str = "CharacterSaveParameterMap";
const WORK_SAVE_DATA: &str = "WorkSaveData";
const DYNAMIC_ITEM_SAVE_DATA: &str = "DynamicItemSaveData";
const BASE_CAMP_SAVE_DATA: &str = "BaseCampSaveData";
const BLUEPRINT_HEADER: &str = "BlueprintHeader";
const RELATIVE_TRANSFORM: &str = "RelativeTransform";

pub fn to_psp_bytes(blueprint: &BaseBlueprint) -> Result<Vec<u8>, CoreError> {
    let save = to_save(blueprint)?;
    let body = savio::write_sav_bytes(&save)?;

    let mut out = Vec::with_capacity(PSP_MAGIC.len() + 4 + body.len());
    out.extend_from_slice(PSP_MAGIC);
    out.extend_from_slice(&SCHEMA_VERSION.to_le_bytes());
    out.extend_from_slice(&body);
    Ok(out)
}

pub fn from_psp_bytes(bytes: &[u8]) -> Result<BaseBlueprint, CoreError> {
    let prefix = PSP_MAGIC.len() + 4;
    if bytes.len() < prefix || &bytes[..PSP_MAGIC.len()] != PSP_MAGIC {
        return Err(CoreError::Parse("not a psp blueprint file".to_string()));
    }
    let mut version_bytes = [0u8; 4];
    version_bytes.copy_from_slice(&bytes[PSP_MAGIC.len()..prefix]);
    let version = u32::from_le_bytes(version_bytes);
    if version > SCHEMA_VERSION {
        return Err(CoreError::Parse(format!(
            "psp blueprint schema version {version} is newer than supported version {SCHEMA_VERSION}"
        )));
    }

    let save = savio::read_sav_bytes(&bytes[prefix..])?;
    from_save(&save)
}

pub fn to_json(blueprint: &BaseBlueprint) -> Result<String, CoreError> {
    let save = to_save(blueprint)?;
    serde_json::to_string_pretty(&save)
        .map_err(|e| CoreError::Parse(format!("blueprint json encode failed: {e}")))
}

pub fn from_json(text: &str) -> Result<BaseBlueprint, CoreError> {
    let save: Save = serde_json::from_str(text)
        .map_err(|e| CoreError::Parse(format!("blueprint json decode failed: {e}")))?;
    from_save(&save)
}

// ---------------------------------------------------------------------------
// Save construction
// ---------------------------------------------------------------------------

pub fn to_save(blueprint: &BaseBlueprint) -> Result<Save, CoreError> {
    let mut save = Save {
        header: blueprint.source_header.clone(),
        schemas: crate::ue::PropertySchemas::default(),
        root: crate::ue::Root {
            save_game_type: SAVE_GAME_TYPE.to_string(),
            properties: Properties::default(),
        },
        extra: Vec::new(),
    };
    let mut weak = std::collections::HashMap::new();

    let header_json = serde_json::to_string(&blueprint.header)
        .map_err(|e| CoreError::Parse(format!("blueprint header json encode failed: {e}")))?;
    set_property(&mut save, &mut weak, BLUEPRINT_HEADER, Property::Str(header_json))?;

    let mut structures = Vec::with_capacity(blueprint.structures.len());
    for structure in &blueprint.structures {
        let mut element = structure.properties.clone();
        element.insert("MapObjectId", props::name_property(&structure.map_object_id));
        element.insert(RELATIVE_TRANSFORM, transform_property(&structure.relative_transform));
        structures.push(StructValue::Struct(element));
    }

    let mut world = Properties::default();
    world.insert(MAP_OBJECT_SAVE_DATA, Property::Array(ValueVec::Struct(structures)));
    world.insert(ITEM_CONTAINER_SAVE_DATA, Property::Map(blueprint.item_containers.clone()));
    world.insert(
        CHARACTER_CONTAINER_SAVE_DATA,
        Property::Map(blueprint.character_containers.clone()),
    );
    world.insert(CHARACTER_SAVE_PARAMETER_MAP, Property::Map(blueprint.characters.clone()));
    world.insert(WORK_SAVE_DATA, Property::Array(ValueVec::Struct(blueprint.works.clone())));
    world.insert(
        DYNAMIC_ITEM_SAVE_DATA,
        Property::Array(ValueVec::Struct(blueprint.dynamic_items.clone())),
    );
    if let Some(base_camp) = &blueprint.base_camp {
        world.insert(BASE_CAMP_SAVE_DATA, base_camp_property(base_camp));
    }

    set_property(
        &mut save,
        &mut weak,
        WORLD_SAVE_DATA,
        Property::Struct(StructValue::Struct(world)),
    )?;

    Ok(save)
}

pub fn from_save(save: &Save) -> Result<BaseBlueprint, CoreError> {
    if save.root.save_game_type != SAVE_GAME_TYPE {
        return Err(CoreError::Parse(format!(
            "not a psp blueprint: save game type is {:?}, expected {SAVE_GAME_TYPE:?}",
            save.root.save_game_type
        )));
    }

    let header_prop = save
        .root
        .properties
        .0
        .get(&PropertyKey::from(BLUEPRINT_HEADER))
        .ok_or_else(|| CoreError::Parse("blueprint missing BlueprintHeader".to_string()))?;
    let header_json = props::as_str(header_prop)
        .ok_or_else(|| CoreError::Parse("BlueprintHeader is not a string property".to_string()))?;
    let header: BlueprintHeader = serde_json::from_str(header_json)
        .map_err(|e| CoreError::Parse(format!("blueprint header json decode failed: {e}")))?;

    let world = world_save_data(&save.root.properties)?;

    let mut structures = Vec::new();
    for item in get_struct_array(world, MAP_OBJECT_SAVE_DATA)? {
        let StructValue::Struct(props) = item else {
            return Err(CoreError::Parse(format!(
                "{MAP_OBJECT_SAVE_DATA} element is not a struct"
            )));
        };
        let mut element = props.clone();
        let map_object_id = match element.0.get(&PropertyKey::from("MapObjectId")) {
            Some(Property::Name(name)) => name.clone(),
            _ => return Err(CoreError::Parse("structure missing MapObjectId".to_string())),
        };
        let relative_transform = element
            .0
            .shift_remove(&PropertyKey::from(RELATIVE_TRANSFORM))
            .ok_or_else(|| CoreError::Parse("structure missing RelativeTransform".to_string()))?;
        let relative_transform = transform_from_property(&relative_transform)?;
        structures.push(BlueprintStructure { map_object_id, relative_transform, properties: element });
    }

    let item_containers = get_map_entries(world, ITEM_CONTAINER_SAVE_DATA)?;
    let character_containers = get_map_entries(world, CHARACTER_CONTAINER_SAVE_DATA)?;
    let characters = get_map_entries(world, CHARACTER_SAVE_PARAMETER_MAP)?;
    let works = get_struct_array(world, WORK_SAVE_DATA)?.to_vec();
    let dynamic_items = get_struct_array(world, DYNAMIC_ITEM_SAVE_DATA)?.to_vec();
    let base_camp = base_camp_from_world(world)?;

    let blueprint = BaseBlueprint {
        header,
        source_header: save.header.clone(),
        base_camp,
        structures,
        item_containers,
        character_containers,
        characters,
        works,
        dynamic_items,
    };
    blueprint.check_schema_version()?;
    blueprint.check_manifest_consistency()?;
    Ok(blueprint)
}

/// The game keys `BaseCampSaveData` by base id; a blueprint carries exactly
/// one base and no id of its own, so it writes a single nil-keyed entry. The
/// shape is what matters: only at `worldSaveData.BaseCampSaveData` do the
/// `RawData` -> `PalBaseCamp`, `WorkerDirector.RawData` and `ModuleMap.Value`
/// hints apply.
fn base_camp_property(base_camp: &Properties) -> Property {
    Property::Map(vec![MapEntry {
        key: Property::Struct(StructValue::Guid(FGuid::nil())),
        value: Property::Struct(StructValue::Struct(base_camp.clone())),
    }])
}

fn base_camp_from_world(world: &Properties) -> Result<Option<Properties>, CoreError> {
    let Some(prop) = world.0.get(&PropertyKey::from(BASE_CAMP_SAVE_DATA)) else {
        return Ok(None);
    };
    let Property::Map(entries) = prop else {
        return Err(CoreError::Parse(format!("{BASE_CAMP_SAVE_DATA} is not a map property")));
    };
    let Some(entry) = entries.first() else {
        return Ok(None);
    };
    match &entry.value {
        Property::Struct(StructValue::Struct(props)) => Ok(Some(props.clone())),
        _ => Err(CoreError::Parse(format!(
            "{BASE_CAMP_SAVE_DATA} entry value is not a struct"
        ))),
    }
}

fn world_save_data(properties: &Properties) -> Result<&Properties, CoreError> {
    match properties.0.get(&PropertyKey::from(WORLD_SAVE_DATA)) {
        Some(Property::Struct(StructValue::Struct(props))) => Ok(props),
        Some(_) => Err(CoreError::Parse(format!("{WORLD_SAVE_DATA} is not a struct property"))),
        None => Err(CoreError::Parse(format!("blueprint missing {WORLD_SAVE_DATA}"))),
    }
}

/// A genuinely absent key is an empty collection; a key that is PRESENT but
/// mis-shaped is a decode error. Returning empty for the latter is how a
/// blueprint silently loses its contents.
fn get_map_entries(properties: &Properties, key: &str) -> Result<Vec<MapEntry>, CoreError> {
    match properties.0.get(&PropertyKey::from(key)) {
        None => Ok(Vec::new()),
        Some(Property::Map(entries)) => Ok(entries.clone()),
        Some(_) => Err(CoreError::Parse(format!("{key} is not a map property"))),
    }
}

fn get_struct_array<'a>(
    properties: &'a Properties,
    key: &str,
) -> Result<&'a [StructValue], CoreError> {
    match properties.0.get(&PropertyKey::from(key)) {
        None => Ok(&[]),
        Some(Property::Array(ValueVec::Struct(items))) => Ok(items),
        Some(_) => Err(CoreError::Parse(format!("{key} is not a struct array property"))),
    }
}

fn transform_property(transform: &PalTransform) -> Property {
    let mut fields = Properties::default();
    fields.insert("Rotation", Property::Struct(StructValue::Quat(transform.rotation.clone())));
    fields.insert("Translation", Property::Struct(StructValue::Vector(transform.translation.clone())));
    fields.insert("Scale3D", Property::Struct(StructValue::Vector(transform.scale.clone())));
    Property::Struct(StructValue::Struct(fields))
}

fn transform_from_property(prop: &Property) -> Result<PalTransform, CoreError> {
    let Property::Struct(StructValue::Struct(fields)) = prop else {
        return Err(CoreError::Parse("RelativeTransform is not a struct".to_string()));
    };
    let rotation = match fields.0.get(&PropertyKey::from("Rotation")) {
        Some(Property::Struct(StructValue::Quat(value))) => value.clone(),
        _ => return Err(CoreError::Parse("RelativeTransform missing Rotation".to_string())),
    };
    let translation = match fields.0.get(&PropertyKey::from("Translation")) {
        Some(Property::Struct(StructValue::Vector(value))) => value.clone(),
        _ => return Err(CoreError::Parse("RelativeTransform missing Translation".to_string())),
    };
    let scale = match fields.0.get(&PropertyKey::from("Scale3D")) {
        Some(Property::Struct(StructValue::Vector(value))) => value.clone(),
        _ => return Err(CoreError::Parse("RelativeTransform missing Scale3D".to_string())),
    };
    Ok(PalTransform { rotation, translation, scale })
}

// ---------------------------------------------------------------------------
// Schema priming.
//
// uesave's JSON deserializer picks each `Property`/`StructValue` variant by
// consulting `save.schemas` at the property's exact dotted path -- the same
// mechanism the binary reader uses (see the amendment at the top of this
// task). A property inserted into `root.properties` without a matching schema
// entry either fails to write (`Error::MissingPropertySchema`) or, worse,
// deserializes from JSON as the wrong variant. `set_property` is the only
// place a property is inserted, and `prime` is the only place a schema is
// recorded, so a property cannot be added here without a tag: every path
// through this module goes through both.
// ---------------------------------------------------------------------------

/// Per-path record of whether the tag recorded there was a guess. Threaded
/// through the whole of `to_save`, so two subtrees that meet at one path still
/// see each other's observations.
type Observations = std::collections::HashMap<String, bool>;

/// Inserts `prop` at the root under `key` and records its schema (recursing
/// into whatever nested properties it carries).
fn set_property(
    save: &mut Save,
    observed: &mut Observations,
    key: &str,
    prop: Property,
) -> Result<(), CoreError> {
    prime(save, observed, key, key, &prop)?;
    save.root.properties.insert(key, prop);
    Ok(())
}

/// Records `prop`'s schema at `path` and recurses into any nested `Properties`
/// it carries, at the paths uesave's own pipeline would use to look them up.
///
/// A path can legitimately see more than one call (every `MapObjectSaveData`
/// element shares "worldSaveData.MapObjectSaveData.<field>", every map entry
/// shares its map's own path, ...), and siblings are not always shaped alike:
/// a `RawData` blob that failed to parse (or was empty) on capture stays a raw
/// byte array instead of the typed Palworld struct another sibling has, and an
/// empty map/array yields no real key/value shape to observe. See
/// `merge_observation` for how two observations of one path are reconciled.
fn prime(
    save: &mut Save,
    observed: &mut Observations,
    path: &str,
    key: &str,
    prop: &Property,
) -> Result<(), CoreError> {
    let incoming = tag_data_for(prop, path, key)?;
    let merged = match (observed.get(path).copied(), save.schemas.get(path).cloned()) {
        (Some(was_weak), Some(existing)) => {
            merge_observation(path, (existing.data, was_weak), incoming)?
        }
        _ => incoming,
    };
    save.schemas.record(path.to_string(), PropertyTagPartial { id: None, data: merged.0 });
    observed.insert(path.to_string(), merged.1);

    match prop {
        Property::Struct(value) => prime_struct_value(save, observed, path, value)?,
        Property::Array(ValueVec::Struct(items)) | Property::Set(ValueVec::Struct(items)) => {
            for item in items {
                prime_struct_value(save, observed, path, item)?;
            }
        }
        Property::Map(entries) => {
            for entry in entries {
                prime_map_side(save, observed, path, &entry.key)?;
                prime_map_side(save, observed, path, &entry.value)?;
            }
        }
        _ => {}
    }
    Ok(())
}

/// Reconciles two observations of the same path.
///
/// A guess never overwrites a real observation, and a real observation always
/// overwrites a guess -- that half is load-bearing (a `ConcreteModel.RawData`
/// left as raw bytes must not erase the typed `PalMapConcreteModel` a sibling
/// proved is there).
///
/// Two DIFFERING real observations are not mergeable at all. One schema tag
/// decides how EVERY value at that path decodes, so keeping either silently
/// rewrites the other's variant: a `Property::Name` comes back a
/// `Property::Str`, an `Int64` is truncated to an `Int`. That is the exact
/// silent variant corruption this format exists to prevent, so encoding
/// refuses rather than picking a winner.
fn merge_observation(
    path: &str,
    existing: TaggedData,
    incoming: TaggedData,
) -> Result<TaggedData, CoreError> {
    match (existing.1, incoming.1) {
        (false, true) => Ok(existing),
        (false, false) if existing.0 != incoming.0 => Err(CoreError::Parse(format!(
            "blueprint property {path} was observed with two irreconcilable types, {:?} and \
             {:?}; a single schema tag decides how every value at that path decodes, so \
             encoding either one would silently rewrite the other",
            existing.0, incoming.0
        ))),
        _ => Ok(incoming),
    }
}

/// Every field of `properties`, each recorded at `path` + "." + its own name --
/// the convention uesave's `PropertiesSeed` uses whether `properties` is a
/// struct's fields, an array element's fields, or a map entry's fields (all
/// three share the same path prefix; only the property NAME distinguishes them).
fn prime_properties(
    save: &mut Save,
    observed: &mut Observations,
    path: &str,
    properties: &Properties,
) -> Result<(), CoreError> {
    for (property_key, prop) in properties.0.iter() {
        let name = &property_key.1;
        let prop_path = if path.is_empty() { name.clone() } else { format!("{path}.{name}") };
        prime(save, observed, &prop_path, name, prop)?;
    }
    Ok(())
}

fn prime_struct_value(
    save: &mut Save,
    observed: &mut Observations,
    path: &str,
    value: &StructValue,
) -> Result<(), CoreError> {
    match value {
        StructValue::Struct(nested) => prime_properties(save, observed, path, nested),
        StructValue::Game(pal) => prime_game_struct_properties(save, observed, path, pal),
        _ => Ok(()),
    }
}

/// A map entry's key/value shares the enclosing map's own path (uesave records
/// no separate `Key`/`Value` path segment), and its own tag lives in the map's
/// `Map{key_type, value_type}` tag rather than a fresh schema entry -- so this
/// only recurses into nested properties, never records `value` itself.
fn prime_map_side(
    save: &mut Save,
    observed: &mut Observations,
    path: &str,
    value: &Property,
) -> Result<(), CoreError> {
    match value {
        Property::Struct(sv) => prime_struct_value(save, observed, path, sv)?,
        Property::Array(ValueVec::Struct(items)) | Property::Set(ValueVec::Struct(items)) => {
            for item in items {
                prime_struct_value(save, observed, path, item)?;
            }
        }
        Property::Map(entries) => {
            for entry in entries {
                prime_map_side(save, observed, path, &entry.key)?;
                prime_map_side(save, observed, path, &entry.value)?;
            }
        }
        _ => {}
    }
    Ok(())
}

/// The three Palworld game structs that embed a nested `Properties` field
/// (`PalCharacterData.object`, `PalDynamicItemType::Egg.object`,
/// `PalMapConcreteModelVariant::HatchingEgg.hatched_character_save_parameter`)
/// -- the same three `<Palworld as Game>::deserialize_struct` installs a
/// properties context for. Every other Palworld struct is plain fixed-shape
/// data with no dynamic property bag, so nothing else needs recursion here.
fn prime_game_struct_properties(
    save: &mut Save,
    observed: &mut Observations,
    path: &str,
    pal: &PalStruct<crate::ue::Arch>,
) -> Result<(), CoreError> {
    match pal {
        PalStruct::CharacterData(data) => prime_properties(save, observed, path, &data.object),
        PalStruct::DynamicItem(item) => {
            if let PalDynamicItemType::Egg { object, .. } = &item.item_type {
                prime_properties(save, observed, path, object)?;
            }
            Ok(())
        }
        PalStruct::MapConcreteModel(model) => {
            if let PalMapConcreteModelVariant::HatchingEgg(egg) = &model.model_data {
                prime_properties(save, observed, path, &egg.hatched_character_save_parameter)?;
            }
            Ok(())
        }
        _ => Ok(()),
    }
}

type TaggedData = (PropertyTagDataPartial, bool);

/// Derives a property's tag from its own value, plus whether that derivation
/// is a guess (`true`) rather than a real observation. See `prime`'s doc for
/// why the distinction matters.
fn tag_data_for(prop: &Property, path: &str, key: &str) -> Result<TaggedData, CoreError> {
    use PropertyTagDataPartial as Data;
    Ok(match prop {
        Property::Int8(_) => (Data::Other(PropertyType::Int8Property), false),
        Property::Int16(_) => (Data::Other(PropertyType::Int16Property), false),
        Property::Int(_) => (Data::Other(PropertyType::IntProperty), false),
        Property::Int64(_) => (Data::Other(PropertyType::Int64Property), false),
        Property::UInt8(_) => (Data::Other(PropertyType::UInt8Property), false),
        Property::UInt16(_) => (Data::Other(PropertyType::UInt16Property), false),
        Property::UInt32(_) => (Data::Other(PropertyType::UInt32Property), false),
        Property::UInt64(_) => (Data::Other(PropertyType::UInt64Property), false),
        Property::Float(_) => (Data::Other(PropertyType::FloatProperty), false),
        Property::Double(_) => (Data::Other(PropertyType::DoubleProperty), false),
        Property::Bool(_) => (Data::Other(PropertyType::BoolProperty), false),
        // A ByteProperty's tag decides the WIRE SHAPE of its value: uesave's
        // reader takes `Byte::Byte(u8)` when the tag names no enum type and
        // `Byte::Label(String)` when it does, while the writer dispatches on
        // the variant it actually holds. Tagging a labelled byte as untyped
        // therefore writes a length-prefixed string and reads back a single
        // byte, misaligning everything after it.
        Property::Byte(Byte::Byte(_)) => (Data::Byte(None), false),
        Property::Byte(Byte::Label(_)) => (Data::Byte(Some(key.to_string())), false),
        Property::Enum(_) => (Data::Enum(key.to_string(), None), false),
        Property::Str(_) => (Data::Other(PropertyType::StrProperty), false),
        Property::FieldPath(_) => (Data::Other(PropertyType::FieldPathProperty), false),
        Property::SoftObject(_) => (Data::Other(PropertyType::SoftObjectProperty), false),
        Property::Name(_) => (Data::Other(PropertyType::NameProperty), false),
        Property::Object(_) => (Data::Other(PropertyType::ObjectProperty), false),
        Property::Text(_) => (Data::Other(PropertyType::TextProperty), false),
        Property::Delegate(_) => (Data::Other(PropertyType::DelegateProperty), false),
        Property::MulticastDelegate(_) => {
            (Data::Other(PropertyType::MulticastDelegateProperty), false)
        }
        Property::MulticastInlineDelegate(_) => {
            (Data::Other(PropertyType::MulticastInlineDelegateProperty), false)
        }
        Property::MulticastSparseDelegate(_) => {
            (Data::Other(PropertyType::MulticastSparseDelegateProperty), false)
        }
        Property::Set(values) => {
            let (inner, weak) = value_vec_tag(values, path, key)?;
            (Data::Set { key_type: Box::new(inner) }, weak)
        }
        Property::Map(entries) => {
            let ((key_type, key_weak), (value_type, value_weak)) =
                map_entry_tags(entries, path, key)?;
            (
                Data::Map { key_type: Box::new(key_type), value_type: Box::new(value_type) },
                key_weak || value_weak,
            )
        }
        Property::Struct(value) => {
            (Data::Struct { struct_type: struct_type_of(value, key), id: FGuid::nil() }, false)
        }
        Property::Array(values) => {
            let (inner, weak) = value_vec_tag(values, path, key)?;
            (Data::Array(Box::new(inner)), weak)
        }
        Property::Raw(_) => {
            (Data::Struct { struct_type: StructType::Raw(key.to_string()), id: FGuid::nil() }, false)
        }
    })
}

/// Like `tag_data_for` but for a `ValueVec` (an array/set element type). A
/// byte array is marked a guess because it may be an embedded Palworld struct
/// that stayed unparsed (empty, or a parse that failed and was captured raw);
/// a struct array with no elements has nothing to observe, so its element type
/// is guessed from the property name. EVERY element is observed, not just the
/// first: one element tag covers the whole array, so heterogeneity inside it
/// is exactly as unencodable as a conflict between two arrays.
fn value_vec_tag(values: &ValueVec, path: &str, key: &str) -> Result<TaggedData, CoreError> {
    use PropertyTagDataPartial as Data;
    Ok(match values {
        ValueVec::Int8(_) => (Data::Other(PropertyType::Int8Property), false),
        ValueVec::Int16(_) => (Data::Other(PropertyType::Int16Property), false),
        ValueVec::Int(_) => (Data::Other(PropertyType::IntProperty), false),
        ValueVec::Int64(_) => (Data::Other(PropertyType::Int64Property), false),
        ValueVec::UInt8(_) => (Data::Other(PropertyType::UInt8Property), false),
        ValueVec::UInt16(_) => (Data::Other(PropertyType::UInt16Property), false),
        ValueVec::UInt32(_) => (Data::Other(PropertyType::UInt32Property), false),
        ValueVec::UInt64(_) => (Data::Other(PropertyType::UInt64Property), false),
        ValueVec::Float(_) => (Data::Other(PropertyType::FloatProperty), false),
        ValueVec::Double(_) => (Data::Other(PropertyType::DoubleProperty), false),
        ValueVec::Bool(_) => (Data::Other(PropertyType::BoolProperty), false),
        ValueVec::Byte(_) => (Data::Byte(None), true),
        ValueVec::Enum(_) => (Data::Enum(key.to_string(), None), false),
        ValueVec::Str(_) => (Data::Other(PropertyType::StrProperty), false),
        ValueVec::Text(_) => (Data::Other(PropertyType::TextProperty), false),
        ValueVec::SoftObject(_) => (Data::Other(PropertyType::SoftObjectProperty), false),
        ValueVec::Name(_) => (Data::Other(PropertyType::NameProperty), false),
        ValueVec::Object(_) => (Data::Other(PropertyType::ObjectProperty), false),
        ValueVec::Box(_) => (Data::Struct { struct_type: StructType::Box, id: FGuid::nil() }, false),
        ValueVec::Box2D(_) => {
            (Data::Struct { struct_type: StructType::Box2D, id: FGuid::nil() }, false)
        }
        ValueVec::Struct(items) => {
            let mut observed: Option<TaggedData> = None;
            for item in items {
                let incoming = (
                    Data::Struct { struct_type: struct_type_of(item, key), id: FGuid::nil() },
                    false,
                );
                observed = Some(match observed {
                    None => incoming,
                    Some(existing) => merge_observation(path, existing, incoming)?,
                });
            }
            observed.unwrap_or((
                Data::Struct { struct_type: crate::ue::struct_type_for(key), id: FGuid::nil() },
                true,
            ))
        }
    })
}

/// The key and value element tags of a whole map. Every entry is observed, for
/// the same reason `value_vec_tag` observes every element: the map carries one
/// key tag and one value tag for all of them.
fn map_entry_tags(
    entries: &[MapEntry],
    path: &str,
    key: &str,
) -> Result<(TaggedData, TaggedData), CoreError> {
    let mut key_tag: Option<TaggedData> = None;
    let mut value_tag: Option<TaggedData> = None;
    for entry in entries {
        let incoming_key = tag_data_for(&entry.key, path, key)?;
        key_tag = Some(match key_tag {
            None => incoming_key,
            Some(existing) => {
                merge_observation(&format!("{path} (map key)"), existing, incoming_key)?
            }
        });
        let incoming_value = tag_data_for(&entry.value, path, key)?;
        value_tag = Some(match value_tag {
            None => incoming_value,
            Some(existing) => {
                merge_observation(&format!("{path} (map value)"), existing, incoming_value)?
            }
        });
    }
    let fallback = || {
        (
            PropertyTagDataPartial::Struct {
                struct_type: crate::ue::struct_type_for(key),
                id: FGuid::nil(),
            },
            true,
        )
    };
    Ok((key_tag.unwrap_or_else(fallback), value_tag.unwrap_or_else(fallback)))
}

/// The `StructType` this already-typed `StructValue` was (or, for a fresh
/// generic struct, should be) tagged with. Builtin variants (`Guid`, `Vector`,
/// ...) map 1:1 by construction. A `Game` struct needs its exact bare type
/// name -- that is what routes it through `<Palworld as Game>::deserialize_struct`
/// and back -- so it comes from `pal_struct_name`, not from `key`. A generic
/// `Struct(Properties)` carries no name of its own (see the amendment: that is
/// exactly what makes deriving `Deserialize` over it unsound), so this format
/// is free to name it after the property key -- self-consistent, since nothing
/// outside this blueprint format ever reads the name back.
fn struct_type_of(value: &StructValue, key: &str) -> StructType {
    match value {
        StructValue::Guid(_) => StructType::Guid,
        StructValue::DateTime(_) => StructType::DateTime,
        StructValue::Timespan(_) => StructType::Timespan,
        StructValue::Vector2D(_) => StructType::Vector2D,
        StructValue::Vector(_) => StructType::Vector,
        StructValue::Vector4(_) => StructType::Vector4,
        StructValue::IntVector(_) => StructType::IntVector,
        StructValue::Box(_) => StructType::Box,
        StructValue::Box2D(_) => StructType::Box2D,
        StructValue::IntPoint(_) => StructType::IntPoint,
        StructValue::Quat(_) => StructType::Quat,
        StructValue::LinearColor(_) => StructType::LinearColor,
        StructValue::Color(_) => StructType::Color,
        StructValue::Rotator(_) => StructType::Rotator,
        StructValue::SoftObjectPath(_) => StructType::SoftObjectPath,
        StructValue::SoftClassPath(_) => StructType::SoftClassPath,
        StructValue::GameplayTagContainer(_) => StructType::GameplayTagContainer,
        StructValue::UniqueNetIdRepl(_) => StructType::UniqueNetIdRepl,
        StructValue::KeyHandleMap(_) => StructType::KeyHandleMap,
        StructValue::RichCurveKey(_) => StructType::RichCurveKey,
        StructValue::SkeletalMeshSamplingLODBuiltData(_) => {
            StructType::SkeletalMeshSamplingLODBuiltData
        }
        StructValue::PerPlatformFloat(_) => StructType::PerPlatformFloat,
        StructValue::MovieSceneFrameRange(_) => StructType::MovieSceneFrameRange,
        StructValue::MovieSceneFloatChannel(_) => StructType::MovieSceneFloatChannel,
        StructValue::FrameNumber(_) => StructType::FrameNumber,
        StructValue::ExpressionInput(_) => StructType::ExpressionInput,
        StructValue::MaterialAttributesInput(_) => StructType::MaterialAttributesInput,
        StructValue::ColorMaterialInput(_) => StructType::ColorMaterialInput,
        StructValue::ScalarMaterialInput(_) => StructType::ScalarMaterialInput,
        StructValue::ShadingModelMaterialInput(_) => StructType::ShadingModelMaterialInput,
        StructValue::VectorMaterialInput(_) => StructType::VectorMaterialInput,
        StructValue::Vector2MaterialInput(_) => StructType::Vector2MaterialInput,
        StructValue::MovieSceneSequenceID(_) => StructType::MovieSceneSequenceID,
        StructValue::MovieSceneTrackIdentifier(_) => StructType::MovieSceneTrackIdentifier,
        StructValue::MovieSceneEvaluationKey(_) => StructType::MovieSceneEvaluationKey,
        StructValue::MovieSceneEvaluationFieldEntityTree(_) => {
            StructType::MovieSceneEvaluationFieldEntityTree
        }
        StructValue::NiagaraDataInterfaceGeneratedFunction(_) => {
            StructType::NiagaraDataInterfaceGeneratedFunction
        }
        StructValue::NiagaraDataInterfaceGPUParamInfo(_) => {
            StructType::NiagaraDataInterfaceGPUParamInfo
        }
        StructValue::FontData(_) => StructType::FontData,
        StructValue::ClothLODDataCommon(_) => StructType::ClothLODDataCommon,
        StructValue::NiagaraVariable(_) => StructType::NiagaraVariable,
        StructValue::NiagaraVariableBase(_) => StructType::NiagaraVariableBase,
        StructValue::NiagaraVariableWithOffset(_) => StructType::NiagaraVariableWithOffset,
        StructValue::Game(pal) => StructType::Game(pal_struct_name(pal).to_string()),
        StructValue::Raw(_) => StructType::Raw(key.to_string()),
        StructValue::Struct(_) => crate::ue::struct_type_for(key),
    }
}

/// The bare `PalXxx` name matching this variant, as `Palworld::is_game_struct_type`
/// and `<Palworld as Game>::write_struct`'s dispatch expect.
fn pal_struct_name(value: &PalStruct<crate::ue::Arch>) -> &'static str {
    match value {
        PalStruct::CharacterData(_) => "PalCharacterData",
        PalStruct::ItemContainer(_) => "PalItemContainer",
        PalStruct::GroupData(_) => "PalGroupData",
        PalStruct::DynamicItem(_) => "PalDynamicItem",
        PalStruct::BuildProcess(_) => "PalBuildProcess",
        PalStruct::GuildItemStorage(_) => "PalGuildItemStorage",
        PalStruct::GuildLab(_) => "PalGuildLab",
        PalStruct::ItemContainerSlots(_) => "PalItemContainerSlots",
        PalStruct::CharacterContainer(_) => "PalCharacterContainer",
        PalStruct::Connector(_) => "PalConnector",
        PalStruct::BaseCamp(_) => "PalBaseCamp",
        PalStruct::Work(_) => "PalWork",
        PalStruct::WorkAssign(_) => "PalWorkAssign",
        PalStruct::MapModel(_) => "PalMapModel",
        PalStruct::MapConcreteModel(_) => "PalMapConcreteModel",
        PalStruct::MapConcreteModelModule(_) => "PalMapConcreteModelModule",
    }
}
