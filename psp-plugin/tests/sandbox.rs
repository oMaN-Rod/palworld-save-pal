use psp_plugin::sandbox::{Cancel, Limits, Sandbox};
use psp_plugin::status::RunStatus;

fn sandbox(limits: Limits) -> Sandbox {
    Sandbox::new(limits, Cancel::new()).expect("a sandbox must open")
}

#[test]
fn a_trivial_chunk_runs_to_completion() {
    let mut sb = sandbox(Limits::default());
    assert_eq!(sb.eval("=test", "return tostring(1 + 1)"), RunStatus::Ok);
    assert_eq!(sb.take_return_string().as_deref(), Some("2"));
}

#[test]
fn the_six_permitted_libraries_are_present() {
    let mut sb = sandbox(Limits::default());
    let probe = "return table.concat({\
        type(string), type(table), type(math), type(coroutine), type(utf8), type(pcall)\
    }, ',')";
    assert_eq!(sb.eval("=test", probe), RunStatus::Ok);
    assert_eq!(
        sb.take_return_string().as_deref(),
        Some("table,table,table,table,table,function")
    );
}

#[test]
fn the_excluded_libraries_are_absent() {
    let mut sb = sandbox(Limits::default());
    let probe = "return table.concat({\
        type(io), type(os), type(package), type(debug), type(require)\
    }, ',')";
    assert_eq!(sb.eval("=test", probe), RunStatus::Ok);
    assert_eq!(sb.take_return_string().as_deref(), Some("nil,nil,nil,nil,nil"));
}

#[test]
fn the_base_library_loaders_are_removed() {
    let mut sb = sandbox(Limits::default());
    let probe = "return table.concat({\
        type(load), type(loadfile), type(dofile), type(xpcall)\
    }, ',')";
    assert_eq!(sb.eval("=test", probe), RunStatus::Ok);
    assert_eq!(sb.take_return_string().as_deref(), Some("nil,nil,nil,nil"));
}

#[test]
fn precompiled_bytecode_is_refused_at_load() {
    let mut sb = sandbox(Limits::default());
    let status = sb.eval("=bytecode", "\u{1b}Lua\u{54}\u{0}");
    match status {
        RunStatus::Error(message) => assert!(
            message.contains("text chunk") || message.contains("binary"),
            "expected a chunk-mode rejection, got: {message}"
        ),
        other => panic!("expected Error, got {other:?}"),
    }
}

#[test]
fn an_infinite_loop_hits_the_wall_clock_limit() {
    let mut sb = sandbox(Limits { wall_clock_ms: 250, ..Limits::default() });
    assert_eq!(sb.eval("=spin", "while true do end"), RunStatus::Timeout);
}

#[test]
fn an_allocation_bomb_hits_the_memory_ceiling() {
    let mut sb = sandbox(Limits { memory_bytes: 8 * 1024 * 1024, ..Limits::default() });
    let bomb = "local t = {} while true do t[#t + 1] = string.rep('x', 4096) end";
    assert_eq!(sb.eval("=bomb", bomb), RunStatus::MemoryExceeded);
}

#[test]
fn a_cancelled_run_stops_and_reports_cancelled() {
    let cancel = Cancel::new();
    let mut sb = Sandbox::new(Limits::default(), cancel.clone()).expect("a sandbox must open");
    cancel.cancel();
    assert_eq!(sb.eval("=spin", "while true do end"), RunStatus::Cancelled);
}

#[test]
fn a_script_error_is_reported_without_panicking() {
    let mut sb = sandbox(Limits::default());
    match sb.eval("=boom", "error('deliberate')") {
        RunStatus::Error(message) => assert!(message.contains("deliberate")),
        other => panic!("expected Error, got {other:?}"),
    }
}

#[test]
fn a_script_cannot_escape_the_timeout_with_pcall() {
    let mut sb = sandbox(Limits { wall_clock_ms: 250, ..Limits::default() });
    let evasive = "while true do pcall(function() while true do end end) end";
    assert_eq!(sb.eval("=evasive", evasive), RunStatus::Timeout);
}

#[test]
fn a_stack_overflow_is_survived_and_reported() {
    let mut sb = sandbox(Limits::default());
    let recurse = "local function f() return f() + 1 end return f()";
    match sb.eval("=deep", recurse) {
        RunStatus::Error(_) => {}
        other => panic!("expected Error, got {other:?}"),
    }
}

#[test]
fn a_gc_metamethod_cannot_be_registered() {
    let mut sb = sandbox(Limits { wall_clock_ms: 500, ..Limits::default() });
    let probe = "local t = setmetatable({}, { __gc = function() while true do end end })
                 collectgarbage() collectgarbage()
                 return 'survived'";
    assert_eq!(sb.eval("=gc", probe), RunStatus::Ok);
    assert_eq!(sb.take_return_string().as_deref(), Some("survived"));
}

#[test]
fn a_memory_ceiling_too_small_to_initialise_returns_none() {
    assert!(Sandbox::new(Limits { memory_bytes: 4992, ..Limits::default() }, Cancel::new()).is_none());
}

#[test]
fn a_trip_is_reported_even_when_the_call_returns_normally() {
    let mut sb = sandbox(Limits { wall_clock_ms: 250, ..Limits::default() });
    let cascading = "return pcall(pcall, pcall, function() while true do end end)";
    assert_eq!(sb.eval("=cascade", cascading), RunStatus::Timeout);
}

#[test]
fn a_zero_hook_interval_still_times_out() {
    let mut sb = sandbox(Limits { wall_clock_ms: 250, hook_interval: 0, ..Limits::default() });
    assert_eq!(sb.eval("=spin", "while true do end"), RunStatus::Timeout);
}

#[test]
fn a_gc_metamethod_cannot_be_reinstalled_by_a_newindex_hook() {
    let mut sb = sandbox(Limits { wall_clock_ms: 500, ..Limits::default() });
    let probe = "local mt = setmetatable({}, {
                     __newindex = function(t, k, v) rawset(t, k, function() while true do end end) end
                 })
                 local t = setmetatable({}, mt)
                 collectgarbage() collectgarbage()
                 return tostring(rawget(mt, '__gc'))";
    assert_eq!(sb.eval("=gc2", probe), RunStatus::Ok);
    assert_eq!(sb.take_return_string().as_deref(), Some("nil"));
}

#[test]
fn a_gc_metamethod_cannot_hang_the_teardown() {
    let mut sb = sandbox(Limits { wall_clock_ms: 500, ..Limits::default() });
    let probe = "local mt = setmetatable({}, {
                     __newindex = function(t, k, v) rawset(t, k, function() while true do end end) end
                 })
                 for _ = 1, 50 do setmetatable({}, mt) end
                 return 'built'";
    assert_eq!(sb.eval("=gc3", probe), RunStatus::Ok);
    assert_eq!(sb.take_return_string().as_deref(), Some("built"));
}

#[test]
fn a_trip_inside_a_coroutine_still_stops_the_main_thread() {
    let mut sb = sandbox(Limits { wall_clock_ms: 250, ..Limits::default() });
    let evasive = "local co = coroutine.create(function() while true do end end)
                   coroutine.resume(co)
                   while true do pcall(function() while true do end end) end";
    assert_eq!(sb.eval("=coro", evasive), RunStatus::Timeout);
}

#[test]
fn setmetatable_argument_errors_match_stock_lua() {
    let mut sb = sandbox(Limits::default());
    let probe = "local out = {}
                 local ok, err = pcall(setmetatable)
                 out[#out+1] = tostring(ok)
                 local t = setmetatable({}, {})
                 local ok2 = pcall(setmetatable, t)
                 out[#out+1] = tostring(ok2)
                 out[#out+1] = tostring(getmetatable(t) ~= nil)
                 return table.concat(out, ',')";
    assert_eq!(sb.eval("=smt", probe), RunStatus::Ok);
    assert_eq!(sb.take_return_string().as_deref(), Some("false,false,true"));
}

#[test]
fn setmetatable_still_works_for_the_normal_cases() {
    let mut sb = sandbox(Limits::default());
    let probe = "local t = setmetatable({}, { tag = 'mt' })
                 local had_mt = getmetatable(t) ~= nil
                 setmetatable(t, nil)
                 local removed = getmetatable(t) == nil
                 local protected = setmetatable({}, { __metatable = 'locked' })
                 local ok = pcall(setmetatable, protected, {})
                 return table.concat({tostring(had_mt), tostring(removed), tostring(ok)}, ',')";
    assert_eq!(sb.eval("=smt2", probe), RunStatus::Ok);
    assert_eq!(sb.take_return_string().as_deref(), Some("true,true,false"));
}
