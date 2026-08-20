use psp_lua_sys::ffi::{
    lua_gethook, lua_pcall, lua_sethook, lua_settop, luaL_loadbufferx, LUA_OK,
};
use psp_plugin::sandbox::{Cancel, Limits, Sandbox};
use psp_plugin::status::RunStatus;

struct Workload {
    label: &'static str,
    source: &'static str,
}

const WORKLOADS: [Workload; 3] = [
    Workload {
        label: "numeric loop",
        source: "local s = 0 for i = 1, 5000000 do s = s + i % 7 end return tostring(s)",
    },
    Workload {
        label: "call-heavy loop",
        source: "local function f(x) return x + 1 end \
                  local s = 0 for i = 1, 2000000 do s = f(s) end return tostring(s)",
    },
    Workload {
        label: "pcall-heavy loop",
        source: "local function f(x) return x + 1 end \
                  local s = 0 for i = 1, 500000 do local _, v = pcall(f, s) s = v end \
                  return tostring(s)",
    },
];

/// Runs `source` with no hook installed at all, timing only the load+call.
///
/// This cannot go through `Sandbox::eval`: `eval` calls `arm()`, and `arm()`
/// correctly reinstalls the hook before every run (round 2's Fix 5) — a real
/// run depends on that. Calling `eval` here would put the hook straight back
/// after clearing it, silently measuring the same hooked cost as every other
/// row. That happened twice already; asserting `lua_gethook` is `None` right
/// before timing means a third regression fails loudly instead of quietly
/// flattening the table.
fn time_unhooked(sb: &Sandbox, source: &str) -> i64 {
    unsafe {
        lua_sethook(sb.as_ptr(), None, 0, 0);
        assert!(
            lua_gethook(sb.as_ptr()).is_none(),
            "baseline row still has a hook installed"
        );

        let started = chrono::Utc::now();
        let name = c"=bench";
        let loaded = luaL_loadbufferx(
            sb.as_ptr(),
            source.as_ptr().cast(),
            source.len(),
            name.as_ptr(),
            c"t".as_ptr(),
        );
        assert_eq!(loaded, LUA_OK);
        let called = lua_pcall(sb.as_ptr(), 0, 1, 0);
        assert_eq!(called, LUA_OK);
        let elapsed = chrono::Utc::now() - started;
        lua_settop(sb.as_ptr(), 0);
        elapsed.num_milliseconds()
    }
}

/// Not a pass/fail test — it prints the throughput cost of each candidate hook
/// interval, against a genuine no-hook baseline, so the default can be chosen
/// on data. Run with --nocapture.
#[test]
fn hook_interval_throughput() {
    for workload in WORKLOADS {
        let limits = Limits { wall_clock_ms: 120_000, ..Limits::default() };
        let baseline_sb = Sandbox::new(limits, Cancel::new()).expect("a sandbox must open");
        let baseline_ms = time_unhooked(&baseline_sb, workload.source);
        println!("{:<17} / hook interval  no hook: {baseline_ms} ms", workload.label);
        drop(baseline_sb);

        for interval in [1_000, 10_000, 100_000] {
            let limits = Limits { hook_interval: interval, wall_clock_ms: 120_000, ..Limits::default() };
            let mut sb = Sandbox::new(limits, Cancel::new()).expect("a sandbox must open");

            let started = chrono::Utc::now();
            assert_eq!(sb.eval("=bench", workload.source), RunStatus::Ok);
            let elapsed = chrono::Utc::now() - started;

            println!(
                "{:<17} / hook interval {interval:>8}: {} ms",
                workload.label,
                elapsed.num_milliseconds()
            );
        }
    }
}
