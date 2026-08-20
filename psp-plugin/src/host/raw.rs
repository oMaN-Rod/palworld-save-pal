use std::ffi::c_int;

use psp_core::domain::raw_path::{NodeKind, RawNodeInfo, RawPath, RawScalar, RawScope, VisitAction};
use psp_core::error::CoreError;
use psp_lua_sys::ffi::*;
use uuid::Uuid;

use super::api_def::{ApiFunction, ApiParam, ApiType};
use super::marshal::{arg_string, check_args, push_str, read_string_at, type_name};
use super::{register_table, with_context, HostError, PushHostFn};
use crate::context::RunContext;
use crate::host_fn;
use crate::manifest::Capability;

fn core_error(error: CoreError) -> HostError {
    HostError::new(error.to_string())
}

fn parse_path(text: &str) -> Result<RawPath, HostError> {
    RawPath::parse(text).map_err(core_error)
}

fn parse_target(target: &str, ctx: &RunContext<'_>) -> Result<RawScope, HostError> {
    let uid_of = |target: &str, rest: &str| {
        Uuid::parse_str(rest).map_err(|_| HostError::new(format!("invalid player uid in raw target {target:?}")))
    };
    let requires_players = |ctx: &RunContext<'_>| -> Result<(), HostError> {
        if ctx.grants(Capability::Players) {
            Ok(())
        } else {
            Err(HostError::new("raw target requires the players capability"))
        }
    };

    if target == "level" {
        return Ok(RawScope::Level);
    }
    if let Some(rest) = target.strip_prefix("player_dps:") {
        requires_players(ctx)?;
        return Ok(RawScope::PlayerDps(uid_of(target, rest)?));
    }
    if let Some(rest) = target.strip_prefix("player:") {
        requires_players(ctx)?;
        return Ok(RawScope::Player(uid_of(target, rest)?));
    }
    Err(HostError::new(format!(
        "unknown raw target {target:?}; expected \"level\", \"player:<uid>\" or \"player_dps:<uid>\""
    )))
}

unsafe fn push_scalar(state: *mut lua_State, scalar: &RawScalar) {
    match scalar {
        RawScalar::Int(n) => lua_pushinteger(state, *n),
        RawScalar::Float(f) => lua_pushnumber(state, *f),
        RawScalar::Bool(b) => lua_pushboolean(state, c_int::from(*b)),
        RawScalar::Text(t) => push_str(state, t),
        RawScalar::Guid(g) => push_str(state, &g.to_string()),
        RawScalar::Empty => lua_pushnil(state),
    }
}

unsafe fn read_string_if(state: *mut lua_State, index: c_int) -> Option<String> {
    if lua_type(state, index) != LUA_TSTRING {
        return None;
    }
    read_string_at(state, index)
}

unsafe fn read_scalar_arg(state: *mut lua_State, index: c_int) -> Result<RawScalar, HostError> {
    match lua_type(state, index) {
        LUA_TNUMBER => {
            if lua_isinteger(state, index) != 0 {
                Ok(RawScalar::Int(lua_tointeger(state, index)))
            } else {
                Ok(RawScalar::Float(lua_tonumber(state, index)))
            }
        }
        LUA_TBOOLEAN => Ok(RawScalar::Bool(lua_toboolean(state, index) != 0)),
        LUA_TSTRING => {
            let text = arg_string(state, index, "value")?;
            Ok(match Uuid::parse_str(&text) {
                Ok(uid) => RawScalar::Guid(uid),
                Err(_) => RawScalar::Text(text),
            })
        }
        _ => {
            let actual = type_name(state, index);
            Err(HostError::new(format!("unsupported value type for raw.set: {actual}")))
        }
    }
}

fn node_kind_label(kind: NodeKind) -> &'static str {
    match kind {
        NodeKind::Scalar => "scalar",
        NodeKind::Struct => "struct",
        NodeKind::Map => "map",
        NodeKind::Array => "array",
        NodeKind::Entry => "entry",
        NodeKind::Opaque => "opaque",
    }
}

fn map_visit_action(text: Option<&str>) -> VisitAction {
    match text {
        Some("remove") => VisitAction::Remove,
        Some("stop") => VisitAction::Stop,
        _ => VisitAction::Keep,
    }
}

/// Consumes `info`: no droppable value may be live when a call that can
/// `longjmp` runs, so each owned field is dropped right after its `lua_setfield`.
unsafe fn push_node_table(state: *mut lua_State, info: RawNodeInfo) {
    let RawNodeInfo { key, index, depth, kind, scalar, path } = info;

    lua_createtable(state, 0, 6);
    match key {
        Some(k) => push_str(state, &k),
        None => lua_pushnil(state),
    }
    lua_setfield(state, -2, c"key".as_ptr());
    match scalar {
        Some(s) => push_scalar(state, &s),
        None => lua_pushnil(state),
    }
    lua_setfield(state, -2, c"value".as_ptr());
    match path {
        Some(p) => push_str(state, &p),
        None => lua_pushnil(state),
    }
    lua_setfield(state, -2, c"path".as_ptr());
    match index {
        Some(i) => lua_pushinteger(state, i64::try_from(i).unwrap_or(i64::MAX)),
        None => lua_pushnil(state),
    }
    lua_setfield(state, -2, c"index".as_ptr());
    lua_pushinteger(state, i64::try_from(depth).unwrap_or(i64::MAX));
    lua_setfield(state, -2, c"depth".as_ptr());
    push_str(state, node_kind_label(kind));
    lua_setfield(state, -2, c"kind".as_ptr());
}

/// An unresolvable path errors; `raw.exists`/`raw.kind` are the probes that don't.
fn raw_get(state: *mut lua_State) -> Result<c_int, HostError> {
    unsafe {
        check_args(state, 2, "raw.get")?;
        let target = arg_string(state, 1, "target")?;
        let path = parse_path(&arg_string(state, 2, "path")?)?;

        let value = with_context(state, |ctx| {
            let scope = parse_target(&target, ctx)?;
            ctx.session.raw_get(scope, &path).map_err(core_error)
        })?;

        match &value {
            Some(scalar) => push_scalar(state, scalar),
            None => lua_pushnil(state),
        }
        Ok(1)
    }
}

fn raw_exists(state: *mut lua_State) -> Result<c_int, HostError> {
    unsafe {
        check_args(state, 2, "raw.exists")?;
        let target = arg_string(state, 1, "target")?;
        let path = parse_path(&arg_string(state, 2, "path")?)?;

        let exists = with_context(state, |ctx| {
            let scope = parse_target(&target, ctx)?;
            Ok(ctx.session.raw_kind(scope, &path).map_err(core_error)?.is_some())
        })?;

        lua_pushboolean(state, c_int::from(exists));
        Ok(1)
    }
}

fn raw_kind(state: *mut lua_State) -> Result<c_int, HostError> {
    unsafe {
        check_args(state, 2, "raw.kind")?;
        let target = arg_string(state, 1, "target")?;
        let path = parse_path(&arg_string(state, 2, "path")?)?;

        let kind = with_context(state, |ctx| {
            let scope = parse_target(&target, ctx)?;
            ctx.session.raw_kind(scope, &path).map_err(core_error)
        })?;

        match kind {
            Some(k) => push_str(state, node_kind_label(k)),
            None => lua_pushnil(state),
        }
        Ok(1)
    }
}

fn raw_set(state: *mut lua_State) -> Result<c_int, HostError> {
    unsafe {
        check_args(state, 3, "raw.set")?;
        let target = arg_string(state, 1, "target")?;
        let path = parse_path(&arg_string(state, 2, "path")?)?;
        let value = read_scalar_arg(state, 3)?;

        with_context(state, |ctx| {
            let scope = parse_target(&target, ctx)?;
            if ctx.dry_run {
                // Validate only — never write, not even transiently.
                ctx.session.raw_can_set(scope, &path, &value).map_err(core_error)?;
                ctx.bump("raw.set", 1);
            } else {
                ctx.session.raw_set(scope, &path, value).map_err(core_error)?;
                // A raw write cannot know whether it touched pal data, so it
                // must assume it did and drop `ctx.pals`.
                ctx.note_pal_field_write();
            }
            Ok(())
        })?;
        Ok(0)
    }
}

fn raw_delete(state: *mut lua_State) -> Result<c_int, HostError> {
    unsafe {
        check_args(state, 2, "raw.delete")?;
        let target = arg_string(state, 1, "target")?;
        let path = parse_path(&arg_string(state, 2, "path")?)?;

        let removed = with_context(state, |ctx| {
            let scope = parse_target(&target, ctx)?;
            if ctx.dry_run {
                let existed = ctx.session.raw_get_json(scope, &path).map_err(core_error)?.is_some();
                ctx.bump("raw.delete", 1);
                Ok(existed)
            } else {
                let removed = ctx.session.raw_delete(scope, &path).map_err(core_error)?;
                if removed {
                    ctx.note_mutation();
                }
                Ok(removed)
            }
        })?;

        lua_pushboolean(state, c_int::from(removed));
        Ok(1)
    }
}

fn raw_len(state: *mut lua_State) -> Result<c_int, HostError> {
    unsafe {
        check_args(state, 2, "raw.len")?;
        let target = arg_string(state, 1, "target")?;
        let path = parse_path(&arg_string(state, 2, "path")?)?;

        let len = with_context(state, |ctx| {
            let scope = parse_target(&target, ctx)?;
            ctx.session.raw_len(scope, &path).map_err(core_error)
        })?;

        match len {
            Some(n) => lua_pushinteger(state, i64::try_from(n).unwrap_or(i64::MAX)),
            None => lua_pushnil(state),
        }
        Ok(1)
    }
}

/// The walk lives in `RunContext` so it survives a `longjmp` past this frame;
/// its borrow is released before each call into Lua so re-entry cannot alias it.
fn raw_visit(state: *mut lua_State) -> Result<c_int, HostError> {
    unsafe {
        check_args(state, 3, "raw.visit")?;
        let target = arg_string(state, 1, "target")?;
        let path = parse_path(&arg_string(state, 2, "path")?)?;
        if !lua_isfunction(state, 3) {
            let actual = type_name(state, 3);
            return Err(HostError::new(format!(
                "raw.visit expects a function for its third argument, got {actual}"
            )));
        }
        let callback_index: c_int = 3;

        let dry_run = with_context(state, |ctx| {
            if ctx.raw_walk.is_some() {
                return Err(HostError::new(
                    "a raw.visit is already running; nested visits are refused",
                ));
            }
            let scope = parse_target(&target, ctx)?;
            let walk = ctx
                .session
                .raw_walk_begin(scope, &path, usize::MAX)
                .map_err(core_error)?;
            ctx.raw_walk = Some(walk);
            Ok(ctx.dry_run)
        })?;
        drop(target);
        drop(path);

        let mut callback_error: Option<String> = None;

        loop {
            let info = with_context(state, |ctx| {
                let walk = ctx.raw_walk.as_mut().ok_or_else(|| HostError::new("raw walk is missing"))?;
                Ok(ctx.session.raw_walk_next(walk))
            })?;
            let Some(info) = info else { break };

            push_node_table(state, info);

            lua_pushvalue(state, callback_index);
            lua_pushvalue(state, -2);
            let call_status = lua_pcall(state, 1, 1, 0);
            if call_status != LUA_OK {
                let message = read_string_if(state, -1)
                    .unwrap_or_else(|| "the raw.visit callback raised a non-string error".to_string());
                lua_pop(state, 2);
                callback_error = Some(message);
                with_context(state, |ctx| {
                    if let Some(walk) = ctx.raw_walk.as_mut() {
                        ctx.session.raw_walk_act(walk, VisitAction::Stop);
                    }
                    Ok(())
                })?;
                break;
            }

            let action = map_visit_action(read_string_if(state, -1).as_deref());
            lua_pop(state, 2);

            with_context(state, |ctx| {
                let dry_run = ctx.dry_run;
                let walk = ctx.raw_walk.as_mut().ok_or_else(|| HostError::new("raw walk is missing"))?;
                if dry_run {
                    // Prune as a real removal would, so a dry run stops exactly
                    // where the real run would, but queue nothing to apply.
                    ctx.session.raw_walk_act_preview(walk, action);
                } else {
                    ctx.session.raw_walk_act(walk, action);
                }
                Ok(())
            })?;
        }

        let stats = with_context(state, |ctx| {
            let mut walk = ctx.raw_walk.take().ok_or_else(|| HostError::new("raw walk is missing"))?;
            Ok(ctx.session.raw_walk_finish(&mut walk))
        })?;

        // Deliberately unconditional: a removal already queued before the
        // callback failed still applies. A partly-applied visit is expected.
        if !dry_run && stats.removed > 0 {
            with_context(state, |ctx| {
                ctx.note_mutation();
                Ok(())
            })?;
        }

        if let Some(message) = callback_error {
            return Err(HostError::new(message));
        }

        lua_createtable(state, 0, 4);
        lua_pushinteger(state, i64::try_from(stats.visited).unwrap_or(i64::MAX));
        lua_setfield(state, -2, c"visited".as_ptr());
        lua_pushinteger(state, i64::try_from(stats.removed).unwrap_or(i64::MAX));
        lua_setfield(state, -2, c"removed".as_ptr());
        lua_pushboolean(state, c_int::from(stats.stopped_early));
        lua_setfield(state, -2, c"stopped".as_ptr());
        lua_pushinteger(state, i64::try_from(stats.removal_errors).unwrap_or(i64::MAX));
        lua_setfield(state, -2, c"removal_errors".as_ptr());
        Ok(1)
    }
}

host_fn!(push_get, raw_get);
host_fn!(push_exists, raw_exists);
host_fn!(push_kind, raw_kind);
host_fn!(push_set, raw_set);
host_fn!(push_delete, raw_delete);
host_fn!(push_len, raw_len);
host_fn!(push_visit, raw_visit);

const RAW_SCALAR: ApiType =
    ApiType::Union(&[ApiType::Nil, ApiType::Boolean, ApiType::Integer, ApiType::Number, ApiType::String]);

pub const RAW_FUNCTIONS: &[ApiFunction] = &[
    ApiFunction {
        name: "get",
        params: &[
            ApiParam { name: "target", ty: ApiType::String, optional: false },
            ApiParam { name: "path", ty: ApiType::String, optional: false },
        ],
        returns: RAW_SCALAR,
        doc: "Reads the raw scalar value at path in target (\"level\", \"player:<uid>\" or \
              \"player_dps:<uid>\"). Returns nil when the node exists but is not a scalar (a \
              struct, map, array, or opaque property). Raises if path does not resolve to \
              anything at all -- use raw.exists to probe for that without raising.",
        capability: None,
    },
    ApiFunction {
        name: "exists",
        params: &[
            ApiParam { name: "target", ty: ApiType::String, optional: false },
            ApiParam { name: "path", ty: ApiType::String, optional: false },
        ],
        returns: ApiType::Boolean,
        doc: "Whether path resolves to anything at all under target, scalar or not. Never raises.",
        capability: None,
    },
    ApiFunction {
        name: "kind",
        params: &[
            ApiParam { name: "target", ty: ApiType::String, optional: false },
            ApiParam { name: "path", ty: ApiType::String, optional: false },
        ],
        returns: ApiType::Union(&[ApiType::String, ApiType::Nil]),
        doc: "The shape of the node at path under target -- one of \"scalar\", \"struct\", \
              \"map\", \"array\", \"entry\" or \"opaque\" -- or nil when path does not resolve. \
              Never raises.",
        capability: None,
    },
    ApiFunction {
        name: "set",
        params: &[
            ApiParam { name: "target", ty: ApiType::String, optional: false },
            ApiParam { name: "path", ty: ApiType::String, optional: false },
            ApiParam {
                name: "value",
                ty: ApiType::Union(&[ApiType::Integer, ApiType::Number, ApiType::Boolean, ApiType::String]),
                optional: false,
            },
        ],
        returns: ApiType::Nil,
        doc: "Overwrites the scalar at an EXISTING path in target with value; raises if path \
              does not resolve or value cannot be converted to that node's type. Does not bump \
              the mutation epoch -- nothing moves, so live handles and iterators stay valid -- \
              but does force every cached pal field to be re-read on its next access, since a \
              raw write cannot know whether it touched pal data.",
        capability: None,
    },
    ApiFunction {
        name: "delete",
        params: &[
            ApiParam { name: "target", ty: ApiType::String, optional: false },
            ApiParam { name: "path", ty: ApiType::String, optional: false },
        ],
        returns: ApiType::Boolean,
        doc: "Removes the node at path in target, returning whether anything was actually \
              removed. A true result is a structural write: it invalidates every live handle \
              and iterator across every scope, not only ones touching the same target.",
        capability: None,
    },
    ApiFunction {
        name: "len",
        params: &[
            ApiParam { name: "target", ty: ApiType::String, optional: false },
            ApiParam { name: "path", ty: ApiType::String, optional: false },
        ],
        returns: ApiType::Union(&[ApiType::Integer, ApiType::Nil]),
        doc: "The element count of the array or map at path in target, or nil when the node \
              exists but has no length. Raises if path does not resolve to anything at all.",
        capability: None,
    },
    ApiFunction {
        name: "visit",
        params: &[
            ApiParam { name: "target", ty: ApiType::String, optional: false },
            ApiParam { name: "path", ty: ApiType::String, optional: false },
            ApiParam { name: "callback", ty: ApiType::Any, optional: false },
        ],
        returns: ApiType::Table,
        doc: "Walks every node under path in target depth-first, calling callback(node) for \
              each with a table of { key, value, path, index, depth, kind }. callback may \
              return \"remove\" to delete that node's subtree, \"stop\" to end the walk early, \
              or anything else to keep walking. Returns a { visited, removed, stopped, \
              removal_errors } summary. Any removal is a structural write: it invalidates every \
              live handle and iterator, including ones the walk itself is still using, and even \
              a removal queued before the callback later raises or the walk is stopped is still \
              applied.",
        capability: None,
    },
];

const RAW_PUSH_FNS: [PushHostFn; RAW_FUNCTIONS.len()] =
    [push_get, push_exists, push_kind, push_set, push_delete, push_len, push_visit];

fn raw_bindings() -> [(&'static str, PushHostFn); RAW_FUNCTIONS.len()] {
    std::array::from_fn(|i| (RAW_FUNCTIONS[i].name, RAW_PUSH_FNS[i]))
}

/// `state` must be live with a few free stack slots.
pub unsafe fn install(state: *mut lua_State) {
    register_table(state, "raw", &raw_bindings());
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bindings_names_match_described_functions_in_order() {
        let bindings = raw_bindings();
        let bound_names: Vec<&str> = bindings.iter().map(|(name, _)| *name).collect();
        let described_names: Vec<&str> = RAW_FUNCTIONS.iter().map(|f| f.name).collect();
        assert_eq!(bound_names, described_names);
    }
}
