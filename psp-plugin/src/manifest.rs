use serde::{Deserialize, Serialize};
use std::collections::HashSet;

pub const SUPPORTED_API_VERSION: u32 = 1;
pub const ENTITY_KINDS: &[&str] = &["pal", "player", "guild", "base"];
const MAX_ID_LEN: usize = 64;

const LUA_RESERVED_WORDS: &[&str] = &[
    "and", "break", "do", "else", "elseif", "end", "false", "for", "function", "goto", "if",
    "in", "local", "nil", "not", "or", "repeat", "return", "then", "true", "until", "while",
];

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Capability {
    #[serde(rename = "save.read")]
    SaveRead,
    #[serde(rename = "save.write")]
    SaveWrite,
    #[serde(rename = "save.raw")]
    SaveRaw,
    #[serde(rename = "players")]
    Players,
    #[serde(rename = "gamedata")]
    GameData,
    #[serde(rename = "ui.dialog")]
    UiDialog,
    #[serde(rename = "storage")]
    Storage,
    #[serde(rename = "log")]
    Log,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ParamType {
    Int,
    Float,
    String,
    Bool,
    Enum,
    Entity,
    Multiselect,
}

impl ParamType {
    fn name(self) -> &'static str {
        match self {
            ParamType::Int => "int",
            ParamType::Float => "float",
            ParamType::String => "string",
            ParamType::Bool => "bool",
            ParamType::Enum => "enum",
            ParamType::Entity => "entity",
            ParamType::Multiselect => "multiselect",
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum ParamValue {
    Int(i64),
    Float(f64),
    Text(String),
    Bool(bool),
    List(Vec<String>),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParamDef {
    pub id: String,
    #[serde(rename = "type")]
    pub param_type: ParamType,
    pub label: String,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub default: Option<serde_json::Value>,
    #[serde(default)]
    pub min: Option<f64>,
    #[serde(default)]
    pub max: Option<f64>,
    #[serde(default)]
    pub options: Vec<String>,
    #[serde(default)]
    pub entity: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommandDef {
    pub id: String,
    pub title: String,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub destructive: bool,
    #[serde(default)]
    pub params: Vec<ParamDef>,
}

pub const WIDGET_KINDS: &[&str] = &[
    "entity_select",
    "text_input",
    "number_input",
    "toggle",
    "select",
    "multiselect",
    "table",
    "list",
    "text",
    "button",
];

pub const INPUT_WIDGET_KINDS: &[&str] = &[
    "entity_select",
    "text_input",
    "number_input",
    "toggle",
    "select",
    "multiselect",
];

fn one_column() -> u32 {
    1
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UiSection {
    #[serde(default)]
    pub title: Option<String>,
    #[serde(default = "one_column")]
    pub columns: u32,
    /// The function this section is part of. Sections sharing a group are one
    /// entry in the view's list, however far apart they are declared.
    #[serde(default)]
    pub group: Option<String>,
    #[serde(default)]
    pub widgets: Vec<UiWidget>,
}

/// `widget_type` is a plain string, not an enum: an unknown type must install
/// and be skipped at render, so a plugin written against a newer host keeps
/// working here.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UiWidget {
    #[serde(rename = "type")]
    pub widget_type: String,
    #[serde(default)]
    pub id: Option<String>,
    #[serde(default)]
    pub label: Option<String>,
    #[serde(default)]
    pub entity: Option<String>,
    #[serde(default)]
    pub from: Option<String>,
    #[serde(default)]
    pub path: Option<String>,
    #[serde(default)]
    pub command: Option<String>,
    #[serde(default)]
    pub columns: Vec<String>,
    #[serde(default)]
    pub selectable: bool,
    #[serde(default)]
    pub span: Option<String>,
    #[serde(default)]
    pub args: std::collections::BTreeMap<String, String>,
    #[serde(default)]
    pub text: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Manifest {
    pub id: String,
    pub api_version: u32,
    pub name: String,
    pub version: String,
    #[serde(default)]
    pub author: Option<String>,
    #[serde(default)]
    pub license: Option<String>,
    pub entry: String,
    #[serde(default)]
    pub capabilities: Vec<Capability>,
    #[serde(default)]
    pub commands: Vec<CommandDef>,
    #[serde(default)]
    pub ui: Vec<UiSection>,
}

#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum ManifestError {
    #[error("manifest is not valid JSON or is missing a required field: {0}")]
    Malformed(String),
    #[error(
        "this build implements plugin API version {supported}, but the plugin declares {found}"
    )]
    UnsupportedApiVersion { found: u32, supported: u32 },
    #[error(
        "plugin id {0:?} must be 1-64 characters of lowercase letters, digits, and single . _ or - separators"
    )]
    InvalidId(String),
    #[error("entry {0:?} must be a plain .lua filename with no path separators")]
    InvalidEntry(String),
    #[error("{0:?} is not usable as a Lua global function name")]
    InvalidCommandId(String),
    #[error("command id {0:?} is declared more than once")]
    DuplicateCommandId(String),
    #[error("capability {0:?} is declared more than once")]
    DuplicateCapability(String),
    #[error("save.write requires save.read")]
    WriteRequiresRead,
    #[error("parameter {id:?} on command {command:?}: {reason}")]
    InvalidParam {
        command: String,
        id: String,
        reason: String,
    },
    #[error("argument {0:?} is not declared by this command")]
    UndeclaredArgument(String),
    #[error("argument {id:?} expects {expected}, got {found}")]
    ArgumentType {
        id: String,
        expected: &'static str,
        found: String,
    },
    #[error("argument {id:?} must be between {min} and {max}")]
    ArgumentOutOfRange { id: String, min: f64, max: f64 },
    #[error("argument {id:?} must be one of: {options}")]
    ArgumentNotAnOption { id: String, options: String },
    #[error("view {at}: {reason}")]
    InvalidView { at: String, reason: String },
}

fn is_valid_id(id: &str) -> bool {
    if id.is_empty() || id.len() > MAX_ID_LEN {
        return false;
    }
    let bytes = id.as_bytes();
    let is_alnum = |b: u8| b.is_ascii_lowercase() || b.is_ascii_digit();
    let is_sep = |b: u8| b == b'.' || b == b'_' || b == b'-';
    let Some(&first) = bytes.first() else {
        return false;
    };
    let Some(&last) = bytes.last() else {
        return false;
    };
    if !is_alnum(first) || !is_alnum(last) {
        return false;
    }
    let mut previous_was_sep = false;
    for &b in bytes {
        if is_alnum(b) {
            previous_was_sep = false;
        } else if is_sep(b) {
            if previous_was_sep {
                return false;
            }
            previous_was_sep = true;
        } else {
            return false;
        }
    }
    true
}

fn is_valid_entry(entry: &str) -> bool {
    if entry.is_empty() || entry == "." || entry == ".." {
        return false;
    }
    if entry.contains('/') || entry.contains('\\') || entry.contains(':') {
        return false;
    }
    entry.ends_with(".lua") && entry.len() > ".lua".len()
}

fn is_valid_lua_global(id: &str) -> bool {
    if id.is_empty() {
        return false;
    }
    if LUA_RESERVED_WORDS.contains(&id) {
        return false;
    }
    let mut chars = id.chars();
    let Some(first) = chars.next() else {
        return false;
    };
    if !(first.is_ascii_alphabetic() || first == '_') {
        return false;
    }
    chars.all(|c| c.is_ascii_alphanumeric() || c == '_')
}

fn is_valid_param_id(id: &str) -> bool {
    let mut chars = id.chars();
    let Some(first) = chars.next() else {
        return false;
    };
    if !(first.is_ascii_alphabetic() || first == '_') {
        return false;
    }
    chars.all(|c| c.is_ascii_alphanumeric() || c == '_')
}

impl Manifest {
    pub fn parse(json: &str) -> Result<Manifest, ManifestError> {
        let manifest: Manifest =
            serde_json::from_str(json).map_err(|error| ManifestError::Malformed(error.to_string()))?;

        if manifest.api_version != SUPPORTED_API_VERSION {
            return Err(ManifestError::UnsupportedApiVersion {
                found: manifest.api_version,
                supported: SUPPORTED_API_VERSION,
            });
        }

        if !is_valid_id(&manifest.id) {
            return Err(ManifestError::InvalidId(manifest.id.clone()));
        }

        if manifest.name.trim().is_empty() {
            return Err(ManifestError::Malformed("name must not be empty".to_string()));
        }
        if manifest.version.trim().is_empty() {
            return Err(ManifestError::Malformed(
                "version must not be empty".to_string(),
            ));
        }

        if !is_valid_entry(&manifest.entry) {
            return Err(ManifestError::InvalidEntry(manifest.entry.clone()));
        }

        let mut seen_capabilities: HashSet<Capability> = HashSet::new();
        for capability in &manifest.capabilities {
            if !seen_capabilities.insert(*capability) {
                return Err(ManifestError::DuplicateCapability(format!(
                    "{capability:?}"
                )));
            }
        }
        if seen_capabilities.contains(&Capability::SaveWrite)
            && !seen_capabilities.contains(&Capability::SaveRead)
        {
            return Err(ManifestError::WriteRequiresRead);
        }

        let mut seen_command_ids: HashSet<&str> = HashSet::new();
        for command in &manifest.commands {
            if !is_valid_lua_global(&command.id) {
                return Err(ManifestError::InvalidCommandId(command.id.clone()));
            }
            if !seen_command_ids.insert(command.id.as_str()) {
                return Err(ManifestError::DuplicateCommandId(command.id.clone()));
            }

            let mut seen_param_ids: HashSet<&str> = HashSet::new();
            for param in &command.params {
                validate_param(&command.id, param, &mut seen_param_ids)?;
            }
        }

        validate_view(&manifest)?;

        Ok(manifest)
    }

    pub fn command(&self, id: &str) -> Option<&CommandDef> {
        self.commands.iter().find(|command| command.id == id)
    }

    pub fn grants(&self, capability: Capability) -> bool {
        self.capabilities.contains(&capability)
    }
}

fn is_widget_reference(reference: &str) -> Option<&str> {
    let (widget, suffix) = reference.split_once('.')?;
    if suffix != "selection" && suffix != "value" {
        return None;
    }
    if !is_valid_param_id(widget) {
        return None;
    }
    Some(widget)
}

fn validate_view(manifest: &Manifest) -> Result<(), ManifestError> {
    let command_ids: HashSet<&str> =
        manifest.commands.iter().map(|command| command.id.as_str()).collect();
    let param_ids: HashSet<&str> = manifest
        .commands
        .iter()
        .flat_map(|command| command.params.iter().map(|param| param.id.as_str()))
        .collect();

    let mut widget_ids: HashSet<&str> = HashSet::new();
    for (section_index, section) in manifest.ui.iter().enumerate() {
        let section_at = format!("section {section_index}");
        if !(1..=3).contains(&section.columns) {
            return Err(ManifestError::InvalidView {
                at: section_at,
                reason: format!("columns must be 1, 2 or 3, got {}", section.columns),
            });
        }
        if section.group.as_deref().map(str::trim) == Some("") {
            return Err(ManifestError::InvalidView {
                at: section_at,
                reason: "group must be a title, not blank".to_string(),
            });
        }
        for (widget_index, widget) in section.widgets.iter().enumerate() {
            let at = format!(
                "section {section_index}, widget {widget_index} ({})",
                widget.widget_type
            );
            let refuse = |reason: String| ManifestError::InvalidView { at: at.clone(), reason };

            if let Some(id) = &widget.id {
                if !is_valid_param_id(id) {
                    return Err(refuse(format!("id {id:?} must be a Lua identifier")));
                }
                if !widget_ids.insert(id.as_str()) {
                    return Err(refuse(format!("id {id:?} is declared more than once in this view")));
                }
            }

            if let Some(span) = &widget.span {
                if span != "full" {
                    return Err(refuse(format!("span {span:?} is not a span; the only one is \"full\"")));
                }
            }

            if INPUT_WIDGET_KINDS.contains(&widget.widget_type.as_str()) {
                let Some(id) = &widget.id else {
                    return Err(refuse("an input widget needs an id naming the parameter it feeds".to_string()));
                };
                if !param_ids.contains(id.as_str()) {
                    return Err(refuse(format!("id {id:?} names no parameter declared by any command")));
                }
            }

            if widget.widget_type == "entity_select"
                && !manifest.capabilities.contains(&Capability::SaveRead)
            {
                return Err(refuse(
                    "an entity_select reads the loaded save, so the plugin must declare save.read"
                        .to_string(),
                ));
            }

            if widget.widget_type == "table" && widget.selectable && widget.id.is_none() {
                return Err(refuse(
                    "a selectable table needs an id, or nothing can reference its selection".to_string(),
                ));
            }

            if let Some(from) = &widget.from {
                if !command_ids.contains(from.as_str()) {
                    return Err(refuse(format!("from {from:?} names no command this plugin declares")));
                }
            }

            let button_params: HashSet<&str> = if widget.widget_type == "button" {
                let Some(command_id) = &widget.command else {
                    return Err(refuse("a button needs a command to run".to_string()));
                };
                let Some(command) = manifest.command(command_id) else {
                    return Err(refuse(format!(
                        "command {command_id:?} names no command this plugin declares"
                    )));
                };
                command.params.iter().map(|param| param.id.as_str()).collect()
            } else {
                HashSet::new()
            };

            for (key, reference) in &widget.args {
                if widget.widget_type != "button" {
                    return Err(refuse("args belongs on a button".to_string()));
                }
                if !button_params.contains(key.as_str()) {
                    return Err(refuse(format!("args key {key:?} is not a parameter of this button's command")));
                }
                let Some(target) = is_widget_reference(reference) else {
                    return Err(refuse(format!(
                        "args value {reference:?} must be <widget>.selection or <widget>.value"
                    )));
                };
                if !view_declares_widget(manifest, target) {
                    return Err(refuse(format!("args value {reference:?} names no widget in this view")));
                }
            }
        }
    }
    Ok(())
}

/// Scans the whole view rather than only the widgets seen so far: a button
/// may legitimately sit above the table it reads, and nothing in the layout
/// implies an order of evaluation.
fn view_declares_widget(manifest: &Manifest, id: &str) -> bool {
    manifest
        .ui
        .iter()
        .flat_map(|section| section.widgets.iter())
        .any(|widget| widget.id.as_deref() == Some(id))
}

fn validate_param<'a>(
    command_id: &str,
    param: &'a ParamDef,
    seen_param_ids: &mut HashSet<&'a str>,
) -> Result<(), ManifestError> {
    let invalid = |reason: &str| ManifestError::InvalidParam {
        command: command_id.to_string(),
        id: param.id.clone(),
        reason: reason.to_string(),
    };

    if LUA_RESERVED_WORDS.contains(&param.id.as_str()) {
        return Err(invalid(
            "id is a Lua reserved word and cannot be used as a ctx.args field name",
        ));
    }
    if !is_valid_param_id(&param.id) {
        return Err(invalid("id must be a non-empty Lua identifier"));
    }
    if !seen_param_ids.insert(param.id.as_str()) {
        return Err(invalid("id is declared more than once on this command"));
    }

    if let (Some(min), Some(max)) = (param.min, param.max) {
        if min > max {
            return Err(invalid("min must not be greater than max"));
        }
    }

    if param.param_type == ParamType::Enum && param.options.is_empty() {
        return Err(invalid("an enum parameter must offer at least one option"));
    }

    if param.param_type == ParamType::Entity {
        match param.entity.as_deref() {
            Some(kind) if ENTITY_KINDS.contains(&kind) => {}
            _ => {
                return Err(invalid(&format!(
                    "an entity parameter must name one of: {}",
                    ENTITY_KINDS.join(", ")
                )));
            }
        }
    }

    if let Some(default) = &param.default {
        match param.param_type {
            ParamType::Int => {
                if json_as_int(default).is_none() {
                    return Err(invalid("default must be an integer"));
                }
            }
            ParamType::Float => {
                if default.as_f64().is_none() {
                    return Err(invalid("default must be a number"));
                }
            }
            ParamType::String => {
                if default.as_str().is_none() {
                    return Err(invalid("default must be a string"));
                }
            }
            ParamType::Bool => {
                if default.as_bool().is_none() {
                    return Err(invalid("default must be a boolean"));
                }
            }
            ParamType::Enum => {
                let Some(text) = default.as_str() else {
                    return Err(invalid("default must be a string"));
                };
                if !param.options.iter().any(|option| option == text) {
                    return Err(invalid("default must be one of the declared options"));
                }
            }
            ParamType::Entity => {
                if default.as_str().is_none() {
                    return Err(invalid("default must be a string"));
                }
            }
            ParamType::Multiselect => {
                let Some(items) = default.as_array() else {
                    return Err(invalid("default must be an array of strings"));
                };
                for item in items {
                    let Some(text) = item.as_str() else {
                        return Err(invalid("default must be an array of strings"));
                    };
                    if !param.options.is_empty() && !param.options.iter().any(|option| option == text) {
                        return Err(invalid("default must only contain declared options"));
                    }
                }
            }
        }
    }

    Ok(())
}

fn json_as_int(value: &serde_json::Value) -> Option<i64> {
    if let Some(int) = value.as_i64() {
        return Some(int);
    }
    let float = value.as_f64()?;
    if !float.is_finite() || float.fract() != 0.0 {
        return None;
    }
    // `as i64` saturates instead of failing, so the bound check must run first.
    let min = i64::MIN as f64;
    let max_exclusive = -(i64::MIN as f64);
    if float < min || float >= max_exclusive {
        return None;
    }
    let candidate = float as i64;
    if candidate as f64 == float {
        Some(candidate)
    } else {
        None
    }
}

impl CommandDef {
    pub fn coerce_args(
        &self,
        supplied: &serde_json::Value,
    ) -> Result<Vec<(String, ParamValue)>, ManifestError> {
        let supplied_object = supplied.as_object();
        let mut claimed: HashSet<&str> = HashSet::new();
        let mut coerced = Vec::with_capacity(self.params.len());

        for param in &self.params {
            claimed.insert(param.id.as_str());

            let value = supplied_object
                .and_then(|object| object.get(&param.id))
                .filter(|value| !value.is_null());

            let value = match value {
                Some(value) => value,
                None => match &param.default {
                    Some(default) => default,
                    None => {
                        return Err(ManifestError::ArgumentType {
                            id: param.id.clone(),
                            expected: param.param_type.name(),
                            found: "nothing".to_string(),
                        });
                    }
                },
            };

            let coerced_value = coerce_param_value(param, value)?;
            coerced.push((param.id.clone(), coerced_value));
        }

        if let Some(object) = supplied_object {
            for key in object.keys() {
                if !claimed.contains(key.as_str()) {
                    return Err(ManifestError::UndeclaredArgument(key.clone()));
                }
            }
        }

        Ok(coerced)
    }
}

fn coerce_param_value(
    param: &ParamDef,
    value: &serde_json::Value,
) -> Result<ParamValue, ManifestError> {
    let type_error = || ManifestError::ArgumentType {
        id: param.id.clone(),
        expected: param.param_type.name(),
        found: describe_json_type(value),
    };

    match param.param_type {
        ParamType::Int => {
            let int = json_as_int(value).ok_or_else(type_error)?;
            check_range(param, int as f64)?;
            Ok(ParamValue::Int(int))
        }
        ParamType::Float => {
            let float = value.as_f64().ok_or_else(type_error)?;
            check_range(param, float)?;
            Ok(ParamValue::Float(float))
        }
        ParamType::String => {
            let text = value.as_str().ok_or_else(type_error)?;
            Ok(ParamValue::Text(text.to_string()))
        }
        ParamType::Bool => {
            let boolean = value.as_bool().ok_or_else(type_error)?;
            Ok(ParamValue::Bool(boolean))
        }
        ParamType::Enum => {
            let text = value.as_str().ok_or_else(type_error)?;
            if !param.options.iter().any(|option| option == text) {
                return Err(ManifestError::ArgumentNotAnOption {
                    id: param.id.clone(),
                    options: param.options.join(", "),
                });
            }
            Ok(ParamValue::Text(text.to_string()))
        }
        ParamType::Entity => {
            let text = value.as_str().ok_or_else(type_error)?;
            Ok(ParamValue::Text(text.to_string()))
        }
        ParamType::Multiselect => {
            let items = value.as_array().ok_or_else(type_error)?;
            let mut out = Vec::with_capacity(items.len());
            for item in items {
                let text = item.as_str().ok_or_else(type_error)?;
                if !param.options.is_empty() && !param.options.iter().any(|option| option == text) {
                    return Err(ManifestError::ArgumentNotAnOption {
                        id: param.id.clone(),
                        options: param.options.join(", "),
                    });
                }
                out.push(text.to_string());
            }
            Ok(ParamValue::List(out))
        }
    }
}

fn check_range(param: &ParamDef, value: f64) -> Result<(), ManifestError> {
    if !value.is_finite() {
        return Err(ManifestError::ArgumentType {
            id: param.id.clone(),
            expected: param.param_type.name(),
            found: "a non-finite number".to_string(),
        });
    }
    if let Some(min) = param.min {
        if value < min {
            return Err(range_error(param));
        }
    }
    if let Some(max) = param.max {
        if value > max {
            return Err(range_error(param));
        }
    }
    Ok(())
}

fn range_error(param: &ParamDef) -> ManifestError {
    ManifestError::ArgumentOutOfRange {
        id: param.id.clone(),
        min: param.min.unwrap_or(f64::NEG_INFINITY),
        max: param.max.unwrap_or(f64::INFINITY),
    }
}

fn describe_json_type(value: &serde_json::Value) -> String {
    match value {
        serde_json::Value::Null => "null",
        serde_json::Value::Bool(_) => "bool",
        serde_json::Value::Number(_) => "number",
        serde_json::Value::String(_) => "string",
        serde_json::Value::Array(_) => "array",
        serde_json::Value::Object(_) => "object",
    }
    .to_string()
}
