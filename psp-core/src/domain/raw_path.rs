//! Path addressing, key deletion and a bounded walk over a raw GVAS tree: a
//! dotted/indexed path syntax, in-place scalar mutation, order-preserving deletion,
//! and a traversal that survives across host-call boundaries (see `RawWalk`).

use std::fmt;

use uuid::Uuid;

use crate::error::CoreError;
use crate::props;
use crate::session::SaveSession;
use crate::ue::games::palworld::{PalDynamicItemType, PalMapConcreteModelVariant};
use crate::ue::{
    Byte, Double, FGuid, Float, MapEntry, PalStruct, Properties, Property, PropertyKey, StructValue, ValueVec,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RawScope {
    Level,
    Player(Uuid),
    PlayerDps(Uuid),
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Segment {
    Key(String),
    Index(usize),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RawPath(Vec<Segment>);

impl RawPath {
    pub fn parse(text: &str) -> Result<RawPath, CoreError> {
        fn invalid(text: &str, reason: &str) -> CoreError {
            CoreError::Other(format!("invalid raw path {text:?}: {reason}"))
        }

        fn read_key(chars: &[char], pos: &mut usize) -> String {
            let start = *pos;
            while *pos < chars.len() && !matches!(chars[*pos], '.' | '[' | ']') {
                *pos += 1;
            }
            chars[start..*pos].iter().collect()
        }

        let chars: Vec<char> = text.chars().collect();
        if chars.is_empty() {
            return Err(invalid(text, "path is empty"));
        }

        let mut pos = 0usize;
        let mut segments = Vec::new();

        let first_key = read_key(&chars, &mut pos);
        if first_key.is_empty() {
            return Err(invalid(text, "path must start with a key"));
        }
        segments.push(Segment::Key(first_key));

        while pos < chars.len() {
            match chars[pos] {
                '.' => {
                    pos += 1;
                    let key = read_key(&chars, &mut pos);
                    if key.is_empty() {
                        return Err(invalid(text, "expected a key after '.'"));
                    }
                    segments.push(Segment::Key(key));
                }
                '[' => {
                    pos += 1;
                    let digit_start = pos;
                    while pos < chars.len() && chars[pos].is_ascii_digit() {
                        pos += 1;
                    }
                    let digits: String = chars[digit_start..pos].iter().collect();
                    if digits.is_empty() {
                        return Err(invalid(text, "expected digits inside '[]'"));
                    }
                    if pos >= chars.len() || chars[pos] != ']' {
                        return Err(invalid(text, "unterminated '['"));
                    }
                    pos += 1;
                    let index: usize = digits
                        .parse()
                        .map_err(|_| invalid(text, "index out of range"))?;
                    segments.push(Segment::Index(index));
                }
                _ => return Err(invalid(text, "unexpected character")),
            }
        }

        Ok(RawPath(segments))
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

impl fmt::Display for RawPath {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", render_segments(&self.0))
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum RawScalar {
    Int(i64),
    Float(f64),
    Bool(bool),
    Text(String),
    Guid(Uuid),
    Empty,
}

impl fmt::Display for RawScalar {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            RawScalar::Int(_) => write!(f, "Int"),
            RawScalar::Float(_) => write!(f, "Float"),
            RawScalar::Bool(_) => write!(f, "Bool"),
            RawScalar::Text(_) => write!(f, "Text"),
            RawScalar::Guid(_) => write!(f, "Guid"),
            RawScalar::Empty => write!(f, "Empty"),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NodeKind {
    Scalar,
    Struct,
    Map,
    Array,
    Entry,
    Opaque,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VisitAction {
    Keep,
    Remove,
    Stop,
}

#[derive(Debug, Clone, Copy)]
pub struct VisitStats {
    pub visited: usize,
    pub removed: usize,
    pub stopped_early: bool,
    /// Removals that failed to apply at `raw_walk_finish`. Nonzero means `removed` overcounts the tree.
    pub removal_errors: usize,
}

#[derive(Debug, Clone)]
pub struct RawNodeInfo {
    pub key: Option<String>,
    pub index: Option<usize>,
    pub depth: usize,
    pub kind: NodeKind,
    pub scalar: Option<RawScalar>,
    /// This node's address in [`RawPath::parse`] syntax, resolving back to this exact
    /// node. `None`, never a best-effort guess, when the address cannot render
    /// faithfully (a segment containing `.`/`[`/`]`, an empty key, or a duplicate-name
    /// key with no index syntax to express it): a lying path is worse than no path.
    pub path: Option<String>,
}

pub struct RawNodeMut<'a> {
    key: Option<String>,
    index: Option<usize>,
    depth: usize,
    kind: NodeKind,
    node: NodeMut<'a>,
}

impl RawNodeMut<'_> {
    pub fn key(&self) -> Option<&str> {
        self.key.as_deref()
    }

    pub fn index(&self) -> Option<usize> {
        self.index
    }

    pub fn depth(&self) -> usize {
        self.depth
    }

    pub fn kind(&self) -> NodeKind {
        self.kind
    }

    pub fn scalar(&self) -> Option<RawScalar> {
        match &self.node {
            NodeMut::Prop(p) => property_scalar(p),
            NodeMut::Struct(sv) => struct_value_scalar(sv),
            NodeMut::Props(_) | NodeMut::Entry(_) => None,
        }
    }

    pub fn set_scalar(&mut self, value: RawScalar) -> Result<(), CoreError> {
        set_scalar_on_node(&mut self.node, value)
    }
}

fn scope_tree(session: &mut SaveSession, scope: RawScope) -> Result<&mut crate::ue::Save, CoreError> {
    match scope {
        RawScope::Level => Ok(&mut session.level),
        RawScope::Player(uid) => {
            session.ensure_player_loaded(uid)?;
            session
                .loaded_players
                .get_mut(&uid)
                .map(|loaded| &mut loaded.sav)
                .ok_or(CoreError::PlayerNotFound(uid))
        }
        RawScope::PlayerDps(uid) => {
            session.ensure_player_loaded(uid)?;
            let loaded = session
                .loaded_players
                .get_mut(&uid)
                .ok_or(CoreError::PlayerNotFound(uid))?;
            loaded
                .dps
                .as_mut()
                .ok_or_else(|| CoreError::Other(format!("player {uid} has no DPS save")))
        }
    }
}

#[derive(Clone, Copy)]
enum NodeRef<'a> {
    Props(&'a Properties),
    Prop(&'a Property),
    Entry(&'a MapEntry),
    Struct(&'a StructValue),
}

enum NodeMut<'a> {
    Props(&'a mut Properties),
    Prop(&'a mut Property),
    Entry(&'a mut MapEntry),
    Struct(&'a mut StructValue),
}

/// The `StructValue::Game` variants that themselves hold a `Properties` bag reachable
/// during a walk: a character entry, an unhatched egg's pal, a hatching egg's pal.
fn struct_value_properties(sv: &StructValue) -> Option<&Properties> {
    match sv {
        StructValue::Struct(p) => Some(p),
        StructValue::Game(PalStruct::CharacterData(data)) => Some(&data.object),
        StructValue::Game(PalStruct::DynamicItem(item)) => match &item.item_type {
            PalDynamicItemType::Egg { object, .. } => Some(object),
            _ => None,
        },
        StructValue::Game(PalStruct::MapConcreteModel(model)) => match &model.model_data {
            PalMapConcreteModelVariant::HatchingEgg(hatching) => {
                Some(&hatching.hatched_character_save_parameter)
            }
            _ => None,
        },
        _ => None,
    }
}

fn struct_value_properties_mut(sv: &mut StructValue) -> Option<&mut Properties> {
    match sv {
        StructValue::Struct(p) => Some(p),
        StructValue::Game(PalStruct::CharacterData(data)) => Some(&mut data.object),
        StructValue::Game(PalStruct::DynamicItem(item)) => match &mut item.item_type {
            PalDynamicItemType::Egg { object, .. } => Some(object),
            _ => None,
        },
        StructValue::Game(PalStruct::MapConcreteModel(model)) => match &mut model.model_data {
            PalMapConcreteModelVariant::HatchingEgg(hatching) => {
                Some(&mut hatching.hatched_character_save_parameter)
            }
            _ => None,
        },
        _ => None,
    }
}

fn property_struct_properties(property: &Property) -> Option<&Properties> {
    match property {
        Property::Struct(sv) => struct_value_properties(sv),
        _ => None,
    }
}

fn property_struct_properties_mut(property: &mut Property) -> Option<&mut Properties> {
    match property {
        Property::Struct(sv) => struct_value_properties_mut(sv),
        _ => None,
    }
}

fn step_ref<'a>(cursor: NodeRef<'a>, seg: &Segment) -> Option<NodeRef<'a>> {
    match (cursor, seg) {
        (NodeRef::Props(p), Segment::Key(k)) => {
            p.0.get(&PropertyKey::from(k.as_str())).map(NodeRef::Prop)
        }
        (NodeRef::Prop(p), Segment::Key(k)) => property_struct_properties(p)?
            .0
            .get(&PropertyKey::from(k.as_str()))
            .map(NodeRef::Prop),
        (NodeRef::Prop(p), Segment::Index(n)) => match p {
            Property::Map(entries) => entries.get(*n).map(NodeRef::Entry),
            Property::Array(ValueVec::Struct(vs)) => vs.get(*n).map(NodeRef::Struct),
            _ => None,
        },
        (NodeRef::Entry(e), Segment::Key(k)) if k == "key" => Some(NodeRef::Prop(&e.key)),
        (NodeRef::Entry(e), Segment::Key(k)) if k == "value" => Some(NodeRef::Prop(&e.value)),
        (NodeRef::Struct(sv), Segment::Key(k)) => struct_value_properties(sv)?
            .0
            .get(&PropertyKey::from(k.as_str()))
            .map(NodeRef::Prop),
        _ => None,
    }
}

fn resolve_ref<'a>(root: &'a Properties, segments: &[Segment]) -> Option<NodeRef<'a>> {
    let mut cursor = NodeRef::Props(root);
    for seg in segments {
        cursor = step_ref(cursor, seg)?;
    }
    Some(cursor)
}

fn step_mut<'a>(cursor: NodeMut<'a>, seg: &Segment) -> Option<NodeMut<'a>> {
    match cursor {
        NodeMut::Props(p) => match seg {
            Segment::Key(k) => p.0.get_mut(&PropertyKey::from(k.as_str())).map(NodeMut::Prop),
            Segment::Index(_) => None,
        },
        NodeMut::Prop(p) => match seg {
            Segment::Key(k) => property_struct_properties_mut(p)?
                .0
                .get_mut(&PropertyKey::from(k.as_str()))
                .map(NodeMut::Prop),
            Segment::Index(n) => match p {
                Property::Map(entries) => entries.get_mut(*n).map(NodeMut::Entry),
                Property::Array(ValueVec::Struct(vs)) => vs.get_mut(*n).map(NodeMut::Struct),
                _ => None,
            },
        },
        NodeMut::Entry(e) => match seg {
            Segment::Key(k) if k == "key" => Some(NodeMut::Prop(&mut e.key)),
            Segment::Key(k) if k == "value" => Some(NodeMut::Prop(&mut e.value)),
            _ => None,
        },
        NodeMut::Struct(sv) => match seg {
            Segment::Key(k) => struct_value_properties_mut(sv)?
                .0
                .get_mut(&PropertyKey::from(k.as_str()))
                .map(NodeMut::Prop),
            Segment::Index(_) => None,
        },
    }
}

fn resolve_mut<'a>(root: &'a mut Properties, segments: &[Segment]) -> Option<NodeMut<'a>> {
    let mut cursor = NodeMut::Props(root);
    for seg in segments {
        cursor = step_mut(cursor, seg)?;
    }
    Some(cursor)
}

fn property_scalar(property: &Property) -> Option<RawScalar> {
    match property {
        Property::Int8(v) => Some(RawScalar::Int(*v as i64)),
        Property::Int16(v) => Some(RawScalar::Int(*v as i64)),
        Property::Int(v) => Some(RawScalar::Int(*v as i64)),
        Property::Int64(v) => Some(RawScalar::Int(*v)),
        Property::UInt8(v) => Some(RawScalar::Int(*v as i64)),
        Property::UInt16(v) => Some(RawScalar::Int(*v as i64)),
        Property::UInt32(v) => Some(RawScalar::Int(*v as i64)),
        Property::UInt64(v) => Some(RawScalar::Int(*v as i64)),
        Property::Float(Float(v)) => Some(RawScalar::Float(*v as f64)),
        Property::Double(Double(v)) => Some(RawScalar::Float(*v)),
        Property::Bool(v) => Some(RawScalar::Bool(*v)),
        Property::Byte(Byte::Byte(v)) => Some(RawScalar::Int(*v as i64)),
        Property::Byte(Byte::Label(s)) => Some(RawScalar::Text(s.clone())),
        Property::Str(s) | Property::Name(s) | Property::Enum(s) => Some(RawScalar::Text(s.clone())),
        Property::Struct(StructValue::Guid(g)) => Some(RawScalar::Guid(props::guid_to_uuid(g))),
        _ => None,
    }
}

fn struct_value_scalar(sv: &StructValue) -> Option<RawScalar> {
    match sv {
        StructValue::Guid(g) => Some(RawScalar::Guid(props::guid_to_uuid(g))),
        _ => None,
    }
}

fn property_kind_label(property: &Property) -> &'static str {
    match property {
        Property::Int8(_) => "Int8",
        Property::Int16(_) => "Int16",
        Property::Int(_) => "Int",
        Property::Int64(_) => "Int64",
        Property::UInt8(_) => "UInt8",
        Property::UInt16(_) => "UInt16",
        Property::UInt32(_) => "UInt32",
        Property::UInt64(_) => "UInt64",
        Property::Float(_) => "Float",
        Property::Double(_) => "Double",
        Property::Bool(_) => "Bool",
        Property::Byte(Byte::Byte(_)) => "Byte",
        Property::Byte(Byte::Label(_)) => "ByteLabel",
        Property::Str(_) => "Str",
        Property::Name(_) => "Name",
        Property::Enum(_) => "Enum",
        Property::Struct(StructValue::Guid(_)) => "Guid",
        _ => "non-scalar",
    }
}

fn mismatch(given: &RawScalar, existing: &str) -> CoreError {
    CoreError::Other(format!("cannot assign {given} to a {existing} property"))
}

fn overflow(kind: &str, given: i64) -> CoreError {
    CoreError::Other(format!("value {given} does not fit in a {kind} property"))
}

/// What assigning a validated `RawScalar` would concretely write. The only place
/// variant-matching and overflow checks live, so `raw_set` and `raw_can_set` cannot drift.
enum ScalarWrite {
    Int8(i8),
    Int16(i16),
    Int(i32),
    Int64(i64),
    UInt8(u8),
    UInt16(u16),
    UInt32(u32),
    UInt64(u64),
    Float(Float),
    Double(Double),
    Bool(bool),
    Byte(u8),
    ByteLabel(String),
    Text(String),
    Guid(FGuid),
}

fn convert_scalar_for_property(property: &Property, value: RawScalar) -> Result<ScalarWrite, CoreError> {
    let existing_label = property_kind_label(property);
    match property {
        Property::Int8(_) => {
            let RawScalar::Int(n) = value else {
                return Err(mismatch(&value, existing_label));
            };
            Ok(ScalarWrite::Int8(i8::try_from(n).map_err(|_| overflow("Int8", n))?))
        }
        Property::Int16(_) => {
            let RawScalar::Int(n) = value else {
                return Err(mismatch(&value, existing_label));
            };
            Ok(ScalarWrite::Int16(i16::try_from(n).map_err(|_| overflow("Int16", n))?))
        }
        Property::Int(_) => {
            let RawScalar::Int(n) = value else {
                return Err(mismatch(&value, existing_label));
            };
            Ok(ScalarWrite::Int(i32::try_from(n).map_err(|_| overflow("Int", n))?))
        }
        Property::Int64(_) => {
            let RawScalar::Int(n) = value else {
                return Err(mismatch(&value, existing_label));
            };
            Ok(ScalarWrite::Int64(n))
        }
        Property::UInt8(_) => {
            let RawScalar::Int(n) = value else {
                return Err(mismatch(&value, existing_label));
            };
            Ok(ScalarWrite::UInt8(u8::try_from(n).map_err(|_| overflow("UInt8", n))?))
        }
        Property::UInt16(_) => {
            let RawScalar::Int(n) = value else {
                return Err(mismatch(&value, existing_label));
            };
            Ok(ScalarWrite::UInt16(u16::try_from(n).map_err(|_| overflow("UInt16", n))?))
        }
        Property::UInt32(_) => {
            let RawScalar::Int(n) = value else {
                return Err(mismatch(&value, existing_label));
            };
            Ok(ScalarWrite::UInt32(u32::try_from(n).map_err(|_| overflow("UInt32", n))?))
        }
        Property::UInt64(_) => {
            let RawScalar::Int(n) = value else {
                return Err(mismatch(&value, existing_label));
            };
            Ok(ScalarWrite::UInt64(u64::try_from(n).map_err(|_| overflow("UInt64", n))?))
        }
        Property::Float(_) => {
            let RawScalar::Float(f) = value else {
                return Err(mismatch(&value, existing_label));
            };
            Ok(ScalarWrite::Float(Float(f as f32)))
        }
        Property::Double(_) => {
            let RawScalar::Float(f) = value else {
                return Err(mismatch(&value, existing_label));
            };
            Ok(ScalarWrite::Double(Double(f)))
        }
        Property::Bool(_) => {
            let RawScalar::Bool(b) = value else {
                return Err(mismatch(&value, existing_label));
            };
            Ok(ScalarWrite::Bool(b))
        }
        Property::Byte(Byte::Byte(_)) => {
            let RawScalar::Int(n) = value else {
                return Err(mismatch(&value, existing_label));
            };
            Ok(ScalarWrite::Byte(u8::try_from(n).map_err(|_| overflow("Byte", n))?))
        }
        Property::Byte(Byte::Label(_)) => {
            let RawScalar::Text(t) = value else {
                return Err(mismatch(&value, existing_label));
            };
            Ok(ScalarWrite::ByteLabel(t))
        }
        Property::Str(_) => {
            let RawScalar::Text(t) = value else {
                return Err(mismatch(&value, existing_label));
            };
            Ok(ScalarWrite::Text(t))
        }
        Property::Name(_) => {
            let RawScalar::Text(t) = value else {
                return Err(mismatch(&value, existing_label));
            };
            Ok(ScalarWrite::Text(t))
        }
        Property::Enum(_) => {
            let RawScalar::Text(t) = value else {
                return Err(mismatch(&value, existing_label));
            };
            Ok(ScalarWrite::Text(t))
        }
        Property::Struct(StructValue::Guid(_)) => {
            let RawScalar::Guid(u) = value else {
                return Err(mismatch(&value, existing_label));
            };
            Ok(ScalarWrite::Guid(props::uuid_to_guid(u)))
        }
        _ => Err(mismatch(&value, existing_label)),
    }
}

fn convert_scalar_for_struct_value(sv: &StructValue, value: RawScalar) -> Result<ScalarWrite, CoreError> {
    match sv {
        StructValue::Guid(_) => {
            let RawScalar::Guid(u) = value else {
                return Err(mismatch(&value, "Guid"));
            };
            Ok(ScalarWrite::Guid(props::uuid_to_guid(u)))
        }
        _ => Err(mismatch(&value, "non-scalar")),
    }
}

/// The fallback arm is unreachable in practice -- converters only return a variant
/// matching the property's kind -- but errors, in case a future edit desyncs the two.
fn assign_scalar_write(property: &mut Property, write: ScalarWrite) -> Result<(), CoreError> {
    match (property, write) {
        (Property::Int8(v), ScalarWrite::Int8(n)) => *v = n,
        (Property::Int16(v), ScalarWrite::Int16(n)) => *v = n,
        (Property::Int(v), ScalarWrite::Int(n)) => *v = n,
        (Property::Int64(v), ScalarWrite::Int64(n)) => *v = n,
        (Property::UInt8(v), ScalarWrite::UInt8(n)) => *v = n,
        (Property::UInt16(v), ScalarWrite::UInt16(n)) => *v = n,
        (Property::UInt32(v), ScalarWrite::UInt32(n)) => *v = n,
        (Property::UInt64(v), ScalarWrite::UInt64(n)) => *v = n,
        (Property::Float(v), ScalarWrite::Float(f)) => *v = f,
        (Property::Double(v), ScalarWrite::Double(d)) => *v = d,
        (Property::Bool(v), ScalarWrite::Bool(b)) => *v = b,
        (Property::Byte(v @ Byte::Byte(_)), ScalarWrite::Byte(n)) => *v = Byte::Byte(n),
        (Property::Byte(v @ Byte::Label(_)), ScalarWrite::ByteLabel(t)) => *v = Byte::Label(t),
        (Property::Str(v), ScalarWrite::Text(t)) => *v = t,
        (Property::Name(v), ScalarWrite::Text(t)) => *v = t,
        (Property::Enum(v), ScalarWrite::Text(t)) => *v = t,
        (Property::Struct(StructValue::Guid(g)), ScalarWrite::Guid(guid)) => *g = guid,
        _ => return Err(CoreError::Other("scalar conversion produced a mismatched write kind".to_string())),
    }
    Ok(())
}

fn assign_scalar_write_on_struct_value(sv: &mut StructValue, write: ScalarWrite) -> Result<(), CoreError> {
    match (sv, write) {
        (StructValue::Guid(g), ScalarWrite::Guid(guid)) => *g = guid,
        _ => return Err(CoreError::Other("scalar conversion produced a mismatched write kind".to_string())),
    }
    Ok(())
}

fn set_scalar_on_property(property: &mut Property, value: RawScalar) -> Result<(), CoreError> {
    let write = convert_scalar_for_property(property, value)?;
    assign_scalar_write(property, write)
}

fn set_scalar_on_struct_value(sv: &mut StructValue, value: RawScalar) -> Result<(), CoreError> {
    let write = convert_scalar_for_struct_value(sv, value)?;
    assign_scalar_write_on_struct_value(sv, write)
}

fn set_scalar_on_node(node: &mut NodeMut, value: RawScalar) -> Result<(), CoreError> {
    match node {
        NodeMut::Prop(p) => set_scalar_on_property(p, value),
        NodeMut::Struct(sv) => set_scalar_on_struct_value(sv, value),
        NodeMut::Props(_) | NodeMut::Entry(_) => Err(mismatch(&value, "non-scalar")),
    }
}

fn can_set_scalar_on_node(node: NodeRef, value: &RawScalar) -> Result<(), CoreError> {
    match node {
        NodeRef::Prop(p) => convert_scalar_for_property(p, value.clone()).map(|_| ()),
        NodeRef::Struct(sv) => convert_scalar_for_struct_value(sv, value.clone()).map(|_| ()),
        NodeRef::Props(_) | NodeRef::Entry(_) => Err(mismatch(value, "non-scalar")),
    }
}

/// `Array(ValueVec::Struct(_))` is the only array shape `[n]` can step into; every other
/// array/set variant reports `Opaque`, so `NodeKind::Array` never promises what it can't deliver.
fn property_kind(property: &Property) -> NodeKind {
    if property_scalar(property).is_some() {
        return NodeKind::Scalar;
    }
    match property {
        Property::Map(_) => NodeKind::Map,
        Property::Array(ValueVec::Struct(_)) => NodeKind::Array,
        _ => {
            if property_struct_properties(property).is_some() {
                NodeKind::Struct
            } else {
                NodeKind::Opaque
            }
        }
    }
}

fn struct_value_kind(sv: &StructValue) -> NodeKind {
    if struct_value_scalar(sv).is_some() {
        return NodeKind::Scalar;
    }
    if struct_value_properties(sv).is_some() {
        NodeKind::Struct
    } else {
        NodeKind::Opaque
    }
}

fn node_kind(node: NodeRef) -> NodeKind {
    match node {
        NodeRef::Props(_) => NodeKind::Struct,
        NodeRef::Entry(_) => NodeKind::Entry,
        NodeRef::Prop(p) => property_kind(p),
        NodeRef::Struct(sv) => struct_value_kind(sv),
    }
}

fn node_scalar(node: NodeRef) -> Option<RawScalar> {
    match node {
        NodeRef::Prop(p) => property_scalar(p),
        NodeRef::Struct(sv) => struct_value_scalar(sv),
        NodeRef::Props(_) | NodeRef::Entry(_) => None,
    }
}

fn value_vec_len(vv: &ValueVec) -> usize {
    match vv {
        ValueVec::Int8(v) => v.len(),
        ValueVec::Int16(v) => v.len(),
        ValueVec::Int(v) => v.len(),
        ValueVec::Int64(v) => v.len(),
        ValueVec::UInt8(v) => v.len(),
        ValueVec::UInt16(v) => v.len(),
        ValueVec::UInt32(v) => v.len(),
        ValueVec::UInt64(v) => v.len(),
        ValueVec::Float(v) => v.len(),
        ValueVec::Double(v) => v.len(),
        ValueVec::Bool(v) => v.len(),
        ValueVec::Byte(b) => match b {
            crate::ue::ByteArray::Byte(v) => v.len(),
            crate::ue::ByteArray::Label(v) => v.len(),
        },
        ValueVec::Enum(v) => v.len(),
        ValueVec::Str(v) => v.len(),
        ValueVec::Text(v) => v.len(),
        ValueVec::SoftObject(v) => v.len(),
        ValueVec::Name(v) => v.len(),
        ValueVec::Object(v) => v.len(),
        ValueVec::Box(v) => v.len(),
        ValueVec::Box2D(v) => v.len(),
        ValueVec::Struct(v) => v.len(),
    }
}

fn node_len(node: NodeRef) -> Option<usize> {
    match node {
        NodeRef::Props(p) => Some(p.0.len()),
        NodeRef::Prop(Property::Map(entries)) => Some(entries.len()),
        NodeRef::Prop(Property::Array(vv)) | NodeRef::Prop(Property::Set(vv)) => Some(value_vec_len(vv)),
        _ => None,
    }
}

fn node_to_json(node: NodeRef) -> Option<serde_json::Value> {
    match node {
        NodeRef::Props(p) => serde_json::to_value(p).ok(),
        NodeRef::Prop(p) => serde_json::to_value(p).ok(),
        NodeRef::Entry(e) => serde_json::to_value(e).ok(),
        NodeRef::Struct(s) => serde_json::to_value(s).ok(),
    }
}

/// Whether a key segment survives a render-then-reparse round trip: `render_segments`
/// has no escaping syntax, so a name containing `.`/`[`/`]`, an empty name, or a nonzero
/// `dup_index` would render ambiguously or resolve to the wrong node.
fn key_segment_is_faithful(name: &str, dup_index: u32) -> bool {
    dup_index == 0 && !name.is_empty() && !name.contains(['.', '[', ']'])
}

fn keyed_children(props: &Properties) -> Vec<(Segment, bool)> {
    props
        .0
        .keys()
        .map(|k| (Segment::Key(k.1.clone()), key_segment_is_faithful(&k.1, k.0)))
        .collect()
}

fn children_of(node: NodeRef) -> Vec<(Segment, bool)> {
    match node {
        NodeRef::Props(p) => keyed_children(p),
        NodeRef::Entry(_) => vec![
            (Segment::Key("key".to_string()), true),
            (Segment::Key("value".to_string()), true),
        ],
        NodeRef::Prop(p) => match p {
            Property::Map(entries) => (0..entries.len()).map(|i| (Segment::Index(i), true)).collect(),
            Property::Array(ValueVec::Struct(vs)) => (0..vs.len()).map(|i| (Segment::Index(i), true)).collect(),
            _ => match property_struct_properties(p) {
                Some(props) => keyed_children(props),
                None => Vec::new(),
            },
        },
        NodeRef::Struct(sv) => match struct_value_properties(sv) {
            Some(props) => keyed_children(props),
            None => Vec::new(),
        },
    }
}

fn render_segments(address: &[Segment]) -> String {
    let mut out = String::new();
    for (position, segment) in address.iter().enumerate() {
        match segment {
            Segment::Key(key) => {
                if position > 0 {
                    out.push('.');
                }
                out.push_str(key);
            }
            Segment::Index(index) => {
                out.push('[');
                out.push_str(&index.to_string());
                out.push(']');
            }
        }
    }
    out
}

fn info_from_address(address: &[Segment], depth: usize, node: NodeRef, path_ok: bool) -> RawNodeInfo {
    let (key, index) = match address.last() {
        Some(Segment::Key(k)) => (Some(k.clone()), None),
        Some(Segment::Index(i)) => (None, Some(*i)),
        None => (None, None),
    };
    RawNodeInfo {
        key,
        index,
        depth,
        kind: node_kind(node),
        scalar: node_scalar(node),
        path: if path_ok { Some(render_segments(address)) } else { None },
    }
}

enum Removable<'a> {
    PropsKey(&'a mut Properties, PropertyKey),
    MapIndex(&'a mut Vec<MapEntry>, usize),
    ArrayIndex(&'a mut Vec<StructValue>, usize),
}

fn resolve_removable<'a>(root: &'a mut Properties, segments: &[Segment]) -> Option<Removable<'a>> {
    let (last, init) = segments.split_last()?;
    let parent = resolve_mut(root, init)?;
    match (parent, last) {
        (NodeMut::Props(p), Segment::Key(k)) => {
            Some(Removable::PropsKey(p, PropertyKey::from(k.as_str())))
        }
        (NodeMut::Prop(p), Segment::Key(k)) => {
            let props = property_struct_properties_mut(p)?;
            Some(Removable::PropsKey(props, PropertyKey::from(k.as_str())))
        }
        (NodeMut::Prop(p), Segment::Index(n)) => match p {
            Property::Map(entries) => Some(Removable::MapIndex(entries, *n)),
            Property::Array(ValueVec::Struct(vs)) => Some(Removable::ArrayIndex(vs, *n)),
            _ => None,
        },
        (NodeMut::Struct(sv), Segment::Key(k)) => {
            let props = struct_value_properties_mut(sv)?;
            Some(Removable::PropsKey(props, PropertyKey::from(k.as_str())))
        }
        _ => None,
    }
}

/// Removes the final segment via `shift_remove`/`Vec::remove` -- never the
/// swap forms -- so removing one element never reorders its neighbours.
fn remove_at(root: &mut Properties, segments: &[Segment]) -> bool {
    match resolve_removable(root, segments) {
        Some(Removable::PropsKey(props, key)) => props.0.shift_remove(&key).is_some(),
        Some(Removable::MapIndex(entries, n)) => {
            if n < entries.len() {
                entries.remove(n);
                true
            } else {
                false
            }
        }
        Some(Removable::ArrayIndex(vs, n)) => {
            if n < vs.len() {
                vs.remove(n);
                true
            } else {
                false
            }
        }
        None => false,
    }
}

fn apply_removal_at(session: &mut SaveSession, scope: RawScope, address: &[Segment]) -> bool {
    match scope_tree(session, scope) {
        Ok(save) => remove_at(&mut save.root.properties, address),
        Err(_) => false,
    }
}

struct Frame {
    address: Vec<Segment>,
    depth: usize,
    path_ok: bool,
}

/// Traversal state: pending stack, deferred removals, counters. Frames store an address
/// rather than a reference, since every `raw_walk_*` method re-borrows the session fresh.
///
/// Every `VisitAction::Remove` is only recorded here; nothing is removed until
/// `raw_walk_finish`. A mid-walk removal could shift an index a queued frame still
/// addresses, so removals are deferred to one descending-address pass at the end.
pub struct RawWalk {
    scope: RawScope,
    max_depth: usize,
    stack: Vec<Frame>,
    pending_removals: Vec<Vec<Segment>>,
    current: Option<Frame>,
    visited: usize,
    removed: usize,
    stopped_early: bool,
    stopped: bool,
}

impl SaveSession {
    /// `Err` when `path` resolves to nothing; `Ok(None)` when it resolves to a non-scalar
    /// node -- use [`SaveSession::raw_kind`] to tell the two apart.
    pub fn raw_get(&mut self, scope: RawScope, path: &RawPath) -> Result<Option<RawScalar>, CoreError> {
        let save = scope_tree(self, scope)?;
        let node = resolve_ref(&save.root.properties, &path.0)
            .ok_or_else(|| CoreError::Other(format!("raw path not found: {path:?}")))?;
        Ok(node_scalar(node))
    }

    pub fn raw_kind(&mut self, scope: RawScope, path: &RawPath) -> Result<Option<NodeKind>, CoreError> {
        let save = scope_tree(self, scope)?;
        Ok(resolve_ref(&save.root.properties, &path.0).map(node_kind))
    }

    pub fn raw_get_json(
        &mut self,
        scope: RawScope,
        path: &RawPath,
    ) -> Result<Option<serde_json::Value>, CoreError> {
        let save = scope_tree(self, scope)?;
        Ok(resolve_ref(&save.root.properties, &path.0).and_then(node_to_json))
    }

    pub fn raw_set(&mut self, scope: RawScope, path: &RawPath, value: RawScalar) -> Result<(), CoreError> {
        {
            let save = scope_tree(self, scope)?;
            let mut node = resolve_mut(&mut save.root.properties, &path.0)
                .ok_or_else(|| CoreError::Other(format!("raw path not found: {path:?}")))?;
            set_scalar_on_node(&mut node, value)?;
        }
        if matches!(scope, RawScope::Level) {
            self.invalidate_performance_caches();
        }
        Ok(())
    }

    pub fn raw_can_set(&mut self, scope: RawScope, path: &RawPath, value: &RawScalar) -> Result<(), CoreError> {
        let save = scope_tree(self, scope)?;
        let node = resolve_ref(&save.root.properties, &path.0)
            .ok_or_else(|| CoreError::Other(format!("raw path not found: {path:?}")))?;
        can_set_scalar_on_node(node, value)
    }

    pub fn raw_delete(&mut self, scope: RawScope, path: &RawPath) -> Result<bool, CoreError> {
        let removed = {
            let save = scope_tree(self, scope)?;
            remove_at(&mut save.root.properties, &path.0)
        };
        if removed && matches!(scope, RawScope::Level) {
            self.invalidate_performance_caches();
        }
        Ok(removed)
    }

    pub fn raw_len(&mut self, scope: RawScope, path: &RawPath) -> Result<Option<usize>, CoreError> {
        let save = scope_tree(self, scope)?;
        let node = resolve_ref(&save.root.properties, &path.0)
            .ok_or_else(|| CoreError::Other(format!("raw path not found: {path:?}")))?;
        Ok(node_len(node))
    }

    /// Closure-driven walk, used by the Rust tests. Not usable from a Lua host function; see `raw_walk_begin`.
    pub fn raw_visit<F>(
        &mut self,
        scope: RawScope,
        path: &RawPath,
        max_depth: usize,
        mut f: F,
    ) -> Result<VisitStats, CoreError>
    where
        F: FnMut(&mut RawNodeMut<'_>) -> VisitAction,
    {
        let mut walk = self.raw_walk_begin(scope, path, max_depth)?;
        while let Some(info) = self.raw_walk_next(&mut walk) {
            let address = walk.current.as_ref().map(|frame| frame.address.clone());
            let action = match address {
                Some(addr) => match self.resolve_node_mut(scope, &addr) {
                    Some(node_mut) => {
                        let mut node = RawNodeMut {
                            key: info.key.clone(),
                            index: info.index,
                            depth: info.depth,
                            kind: info.kind,
                            node: node_mut,
                        };
                        f(&mut node)
                    }
                    None => VisitAction::Keep,
                },
                None => VisitAction::Keep,
            };
            self.raw_walk_act(&mut walk, action);
        }
        Ok(self.raw_walk_finish(&mut walk))
    }

    /// Host-driven walk: the caller owns the `RawWalk` and can keep it somewhere that survives a Lua `longjmp`.
    pub fn raw_walk_begin(
        &mut self,
        scope: RawScope,
        path: &RawPath,
        max_depth: usize,
    ) -> Result<RawWalk, CoreError> {
        scope_tree(self, scope)?;
        Ok(RawWalk {
            scope,
            max_depth,
            stack: vec![Frame {
                address: path.0.clone(),
                depth: 0,
                path_ok: true,
            }],
            pending_removals: Vec::new(),
            current: None,
            visited: 0,
            removed: 0,
            stopped_early: false,
            stopped: false,
        })
    }

    pub fn raw_walk_next(&mut self, walk: &mut RawWalk) -> Option<RawNodeInfo> {
        if walk.stopped {
            return None;
        }
        loop {
            let frame = walk.stack.pop()?;
            let save = scope_tree(self, walk.scope).ok()?;
            match resolve_ref(&save.root.properties, &frame.address) {
                Some(node) => {
                    let info = info_from_address(&frame.address, frame.depth, node, frame.path_ok);
                    walk.current = Some(frame);
                    return Some(info);
                }
                None => continue,
            }
        }
    }

    pub fn raw_walk_act(&mut self, walk: &mut RawWalk, action: VisitAction) {
        self.raw_walk_act_impl(walk, action, true);
    }

    /// Like `raw_walk_act`, but `VisitAction::Remove` prunes the subtree from traversal
    /// without queueing it -- a dry run that visits and counts what a real run would.
    pub fn raw_walk_act_preview(&mut self, walk: &mut RawWalk, action: VisitAction) {
        self.raw_walk_act_impl(walk, action, false);
    }

    fn raw_walk_act_impl(&mut self, walk: &mut RawWalk, action: VisitAction, apply_removal: bool) {
        let Some(frame) = walk.current.take() else {
            return;
        };
        walk.visited += 1;
        match action {
            VisitAction::Keep => {
                if frame.depth < walk.max_depth {
                    if let Ok(save) = scope_tree(self, walk.scope) {
                        if let Some(node) = resolve_ref(&save.root.properties, &frame.address) {
                            for (seg, seg_ok) in children_of(node).into_iter().rev() {
                                let mut child_addr = frame.address.clone();
                                child_addr.push(seg);
                                walk.stack.push(Frame {
                                    address: child_addr,
                                    depth: frame.depth + 1,
                                    path_ok: frame.path_ok && seg_ok,
                                });
                            }
                        }
                    }
                }
            }
            VisitAction::Remove => {
                walk.removed += 1;
                if apply_removal {
                    walk.pending_removals.push(frame.address);
                }
            }
            VisitAction::Stop => {
                walk.stopped_early = true;
                walk.stopped = true;
                walk.stack.clear();
            }
        }
    }

    /// Applies every deferred removal in one pass, addresses sorted descending, so no
    /// removal can invalidate a resolution still to come.
    pub fn raw_walk_finish(&mut self, walk: &mut RawWalk) -> VisitStats {
        let mut addresses = std::mem::take(&mut walk.pending_removals);
        addresses.sort_by(|a, b| b.cmp(a));

        let mut removal_errors = 0usize;
        let mut applied_any = false;
        for address in &addresses {
            if apply_removal_at(self, walk.scope, address) {
                applied_any = true;
            } else {
                removal_errors += 1;
            }
        }

        if applied_any && matches!(walk.scope, RawScope::Level) {
            self.invalidate_performance_caches();
        }

        VisitStats {
            visited: walk.visited,
            removed: walk.removed,
            stopped_early: walk.stopped_early,
            removal_errors,
        }
    }

    fn resolve_node_mut(&mut self, scope: RawScope, address: &[Segment]) -> Option<NodeMut<'_>> {
        let save = scope_tree(self, scope).ok()?;
        resolve_mut(&mut save.root.properties, address)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// No fixture contains a labelled `ByteProperty`, so this builds one directly.
    fn labelled() -> Property {
        Property::Byte(Byte::Label("EPalWorkSuitability::Handcraft".to_string()))
    }

    fn numeric() -> Property {
        Property::Byte(Byte::Byte(7))
    }

    #[test]
    fn a_parsed_path_renders_back_to_the_same_text() {
        for text in [
            "worldSaveData",
            "worldSaveData.CharacterSaveParameterMap",
            "worldSaveData.CharacterSaveParameterMap[0].value.RawData.SaveParameter.LastOnlineRealTime",
            "SaveData.RecordData.TowerBossDefeatFlag[3]",
            "a[0][1]",
        ] {
            let parsed = RawPath::parse(text).unwrap_or_else(|e| panic!("{text:?} must parse: {e}"));
            assert_eq!(parsed.to_string(), text, "rendering must exactly invert parsing");
        }
    }

    #[test]
    fn a_key_segment_is_faithful_only_when_it_can_render_unambiguously() {
        assert!(key_segment_is_faithful("Level", 0));
        assert!(!key_segment_is_faithful("Level", 1), "a nonzero dup index has no path syntax");
        assert!(!key_segment_is_faithful("Weird.Name", 0), "a dot would be read back as a separator");
        assert!(!key_segment_is_faithful("Weird[Name]", 0), "brackets would be read back as an index");
        assert!(!key_segment_is_faithful("", 0), "RawPath::parse refuses an empty key");
    }

    #[test]
    fn children_of_marks_a_dotted_or_duplicate_key_as_unfaithful() {
        let mut props = Properties::default();
        props.0.insert(PropertyKey(0, "Plain".to_string()), numeric());
        props.0.insert(PropertyKey(0, "Weird.Name".to_string()), numeric());
        props.0.insert(PropertyKey(1, "Dup".to_string()), numeric());

        let children = children_of(NodeRef::Props(&props));
        let faithful_of = |name: &str| {
            children.iter().find_map(|(segment, ok)| match segment {
                Segment::Key(k) if k == name => Some(*ok),
                _ => None,
            })
        };

        assert_eq!(faithful_of("Plain"), Some(true));
        assert_eq!(faithful_of("Weird.Name"), Some(false));
        assert_eq!(faithful_of("Dup"), Some(false));
    }

    #[test]
    fn info_from_address_nils_the_path_exactly_when_told_to() {
        let address = vec![Segment::Key("Weird.Name".to_string())];
        let prop = numeric();
        let node = NodeRef::Prop(&prop);

        let unfaithful = info_from_address(&address, 1, node, false);
        assert!(unfaithful.path.is_none());

        let faithful = info_from_address(&address, 1, node, true);
        assert_eq!(faithful.path.as_deref(), Some("Weird.Name"));
    }

    #[test]
    fn a_labelled_byte_reads_as_text_and_a_numeric_byte_reads_as_an_integer() {
        assert_eq!(
            property_scalar(&labelled()),
            Some(RawScalar::Text("EPalWorkSuitability::Handcraft".to_string()))
        );
        assert_eq!(property_scalar(&numeric()), Some(RawScalar::Int(7)));
    }

    #[test]
    fn a_labelled_byte_stays_labelled_across_a_write() {
        let mut property = labelled();
        set_scalar_on_property(
            &mut property,
            RawScalar::Text("EPalWorkSuitability::Mining".to_string()),
        )
        .expect("a label accepts a new label");
        match property {
            Property::Byte(Byte::Label(text)) => {
                assert_eq!(text, "EPalWorkSuitability::Mining");
            }
            other => panic!("a labelled byte must stay a labelled byte, got {other:?}"),
        }
    }

    #[test]
    fn the_two_byte_forms_cannot_be_written_into_each_other() {
        let mut label = labelled();
        assert!(set_scalar_on_property(&mut label, RawScalar::Int(3)).is_err());
        assert!(matches!(label, Property::Byte(Byte::Label(_))));

        let mut byte = numeric();
        assert!(set_scalar_on_property(&mut byte, RawScalar::Text("x".to_string())).is_err());
        assert!(matches!(byte, Property::Byte(Byte::Byte(7))));
    }

    #[test]
    fn a_numeric_byte_accepts_its_bounds_and_refuses_anything_outside_them() {
        for accepted in [0, 255] {
            let mut property = numeric();
            set_scalar_on_property(&mut property, RawScalar::Int(accepted))
                .unwrap_or_else(|error| panic!("{accepted} is in range, got {error}"));
            assert!(matches!(property, Property::Byte(Byte::Byte(v)) if v as i64 == accepted));
        }

        for refused in [-1, 256, 1000, i64::MAX, i64::MIN] {
            let mut property = numeric();
            assert!(
                set_scalar_on_property(&mut property, RawScalar::Int(refused)).is_err(),
                "{refused} is outside a byte and must be refused"
            );
            assert!(
                matches!(property, Property::Byte(Byte::Byte(7))),
                "a refused write must leave the property untouched"
            );
        }
    }
}
