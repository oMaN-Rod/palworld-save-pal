use serde::{Deserialize, Serialize};
use std::collections::HashSet;

pub const SUPPORTED_API_VERSION: u32 = 1;
const MAX_ID_LEN: usize = 64;

const LUA_RESERVED_WORDS: &[&str] = &[
    "and", "break", "do", "else", "elseif", "end", "false", "for", "function", "goto", "if",
    "in", "local", "nil", "not", "or", "repeat", "return", "then", "true", "until", "while",
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Origin {
    Bundled,
    User,
}

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
}

impl ParamType {
    fn name(self) -> &'static str {
        match self {
            ParamType::Int => "int",
            ParamType::Float => "float",
            ParamType::String => "string",
            ParamType::Bool => "bool",
            ParamType::Enum => "enum",
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum ParamValue {
    Int(i64),
    Float(f64),
    Text(String),
    Bool(bool),
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
    #[error("save.raw is available to bundled plugins only")]
    RawIsBundledOnly,
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
    pub fn parse(json: &str, origin: Origin) -> Result<Manifest, ManifestError> {
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
        if seen_capabilities.contains(&Capability::SaveRaw) && origin == Origin::User {
            return Err(ManifestError::RawIsBundledOnly);
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

        Ok(manifest)
    }

    pub fn command(&self, id: &str) -> Option<&CommandDef> {
        self.commands.iter().find(|command| command.id == id)
    }

    pub fn grants(&self, capability: Capability) -> bool {
        self.capabilities.contains(&capability)
    }
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
