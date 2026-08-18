use std::path::PathBuf;

/// The Lua translation units we compile.
///
/// Deliberately excluded, and why:
/// - `lua.c`, `luac.c`  — the standalone interpreter and bytecode compiler binaries.
/// - `linit.c`          — opens every standard library at once. `psp-plugin` opens
///                        libraries individually so it can leave some out, so this
///                        file is never used. Excluding it also means we never have
///                        to patch it to drop io/os.
/// - `liolib.c`, `loslib.c`, `loadlib.c`, `ldblib.c`
///                      — the io, os, package and debug libraries. Plugins are never
///                        granted these, and leaving them out of the build removes
///                        their libc dependencies (file handles, environment,
///                        dynamic loading) entirely rather than stubbing them.
///
/// Note `ldebug.c` IS built: it holds `lua_sethook`, which the sandbox needs.
/// Only `ldblib.c`, the scriptable `debug` table, is excluded.
const LUA_SOURCES: &[&str] = &[
    "lapi", "lauxlib", "lbaselib", "lcode", "lcorolib", "lctype", "ldebug", "ldo",
    "ldump", "lfunc", "lgc", "llex", "lmathlib", "lmem", "lobject", "lopcodes",
    "lparser", "lstate", "lstring", "lstrlib", "ltable", "ltablib", "ltm",
    "lundump", "lutf8lib", "lvm", "lzio",
];

fn main() {
    let source_dir = PathBuf::from("vendor/lua-5.4.8/src");
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed={}", source_dir.display());

    let mut build = cc::Build::new();
    build.include(&source_dir).warnings(false);
    for unit in LUA_SOURCES {
        build.file(source_dir.join(format!("{unit}.c")));
    }
    // No LUA_USE_* define: Lua then restricts itself to ISO C, which means no
    // dlopen and no readline. That is what we want on every target.
    build.compile("lua");
}
