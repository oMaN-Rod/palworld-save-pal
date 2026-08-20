use super::api_def::{ApiDefinition, ApiField, ApiFunction, ApiGlobal, ApiHandle, ApiParam, ApiType};
use crate::manifest::Capability;

fn capability_str(capability: Capability) -> &'static str {
    match capability {
        Capability::SaveRead => "save.read",
        Capability::SaveWrite => "save.write",
        Capability::SaveRaw => "save.raw",
        Capability::Players => "players",
        Capability::GameData => "gamedata",
        Capability::UiDialog => "ui.dialog",
        Capability::Storage => "storage",
        Capability::Log => "log",
    }
}

fn effective_capability(owner: Option<Capability>, own: Option<Capability>) -> Option<Capability> {
    own.or(owner)
}

fn lua_type(ty: &ApiType) -> String {
    match ty {
        ApiType::Nil => "nil".to_string(),
        ApiType::Boolean => "boolean".to_string(),
        ApiType::Integer => "integer".to_string(),
        ApiType::Number => "number".to_string(),
        ApiType::String => "string".to_string(),
        ApiType::Table => "table".to_string(),
        ApiType::Handle(name) => (*name).to_string(),
        ApiType::Iterator(name) => format!("fun(): {name}|nil"),
        ApiType::Union(members) => members.iter().map(lua_type).collect::<Vec<_>>().join("|"),
        ApiType::Any => "any".to_string(),
    }
}

fn doc_oneline(doc: &str) -> String {
    doc.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn param_type_annotation(param: &ApiParam) -> String {
    if param.optional {
        format!("---@param {}? {}\n", param.name, lua_type(&param.ty))
    } else {
        format!("---@param {} {}\n", param.name, lua_type(&param.ty))
    }
}

fn param_signature_fragment(param: &ApiParam) -> String {
    if param.optional {
        format!("{}?: {}", param.name, lua_type(&param.ty))
    } else {
        format!("{}: {}", param.name, lua_type(&param.ty))
    }
}

/// A return type that is itself a `fun(...)` is parenthesised: written flat,
/// LuaLS cannot tell which function's return union a trailing `|nil` closes.
fn function_signature_type(f: &ApiFunction) -> String {
    let params = f.params.iter().map(param_signature_fragment).collect::<Vec<_>>().join(", ");
    let returns = lua_type(&f.returns);
    let returns = if returns.starts_with("fun(") { format!("({returns})") } else { returns };
    format!("fun({params}): {returns}")
}

fn emit_field(out: &mut String, field: &ApiField) {
    out.push_str(&format!("---@field {} {} {}\n", field.name, lua_type(&field.ty), doc_oneline(field.doc)));
}

fn emit_method_field(out: &mut String, owner_capability: Option<Capability>, method: &ApiFunction) {
    let mut doc = doc_oneline(method.doc);
    if let Some(capability) = effective_capability(owner_capability, method.capability) {
        doc.push_str(&format!(" Requires capability: {}.", capability_str(capability)));
    }
    out.push_str(&format!("---@field {} {} {}\n", method.name, function_signature_type(method), doc));
}

fn emit_handle(out: &mut String, handle: &ApiHandle) {
    if let Some(capability) = handle.capability {
        out.push_str(&format!("---Requires capability: {}.\n", capability_str(capability)));
        out.push_str("---\n");
    }
    out.push_str(&format!("---@class {}\n", handle.name));
    for field in handle.fields {
        emit_field(out, field);
    }
    for method in handle.methods {
        emit_method_field(out, handle.capability, method);
    }
    out.push('\n');
}

fn emit_global_function(out: &mut String, global: &ApiGlobal, function: &ApiFunction) {
    out.push_str(&format!("---{}\n", doc_oneline(function.doc)));
    if let Some(capability) = effective_capability(global.capability, function.capability) {
        out.push_str("---\n");
        out.push_str(&format!("---Requires capability: {}.\n", capability_str(capability)));
    }
    for param in function.params {
        out.push_str(&param_type_annotation(param));
    }
    out.push_str(&format!("---@return {}\n", lua_type(&function.returns)));
    let params = function.params.iter().map(|p| p.name).collect::<Vec<_>>().join(", ");
    out.push_str(&format!("function {}.{}({params}) end\n", global.name, function.name));
    out.push('\n');
}

fn emit_global(out: &mut String, global: &ApiGlobal) {
    if let Some(capability) = global.capability {
        out.push_str(&format!("---Requires capability: {}.\n", capability_str(capability)));
        out.push_str("---\n");
    }
    out.push_str(&format!("---@class {}\n", global.name));
    for field in global.fields {
        emit_field(out, field);
    }
    out.push('\n');
    out.push_str(&format!("---@type {}\n", global.name));
    out.push_str(&format!("{} = {{}}\n", global.name));
    out.push('\n');

    for function in global.functions {
        emit_global_function(out, global, function);
    }
}

pub fn lua_meta(def: &ApiDefinition) -> String {
    let mut out = String::from("---@meta\n\n");

    for handle in &def.handles {
        emit_handle(&mut out, handle);
    }

    for global in &def.globals {
        emit_global(&mut out, global);
    }

    while out.ends_with('\n') {
        out.pop();
    }
    out.push('\n');

    out
}
