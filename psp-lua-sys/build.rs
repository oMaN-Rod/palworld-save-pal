use std::path::{Path, PathBuf};
use std::process::Command;

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
    println!("cargo:rerun-if-env-changed=WASI_SDK");

    let target = std::env::var("TARGET").expect("cargo sets TARGET");
    if target.starts_with("wasm32") {
        build_wasm(&source_dir);
    } else {
        build_native(&source_dir);
    }
}

fn build_native(source_dir: &Path) {
    let mut build = cc::Build::new();
    build.include(source_dir).warnings(false);
    for unit in LUA_SOURCES {
        build.file(source_dir.join(format!("{unit}.c")));
    }
    // No LUA_USE_* define: Lua then restricts itself to ISO C, which means no
    // dlopen and no readline. That is what we want on every target.
    build.compile("lua");
}

/// Compiles Lua for `wasm32-unknown-unknown` using wasi-sdk's clang.
///
/// Three flags carry all the weight, and none of them is optional:
///   `-mllvm -wasm-enable-sjlj`  provides setjmp/longjmp, which is Lua's error
///                               mechanism. Without it `pcall` cannot unwind.
///   `-D_WASI_EMULATED_SIGNAL`   `lstate.h` includes <signal.h>, which is a hard
///                               #error on wasi without this.
///   `--target=wasm32-wasip1`    the C objects are wasip1 while Rust is
///                               wasm32-unknown-unknown; same ABI, links fine.
///
/// Lua is compiled as plain C, never as C++, and never with `-fwasm-exceptions`:
/// that combination pulls in `libc++abi` and `libunwind`, whose prebuilt
/// exception-handling dialects disagree with each other, and the resulting
/// module gets rejected as mixing legacy and new exception-handling
/// instructions. Plain C plus wasi-sdk's `libsetjmp` has no C++ runtime
/// anywhere and keeps `wasm-opt` working.
fn build_wasm(source_dir: &Path) {
    let out_dir = PathBuf::from(std::env::var("OUT_DIR").expect("cargo sets OUT_DIR"));

    let Some(sdk) = std::env::var_os("WASI_SDK").map(PathBuf::from) else {
        link_prebuilt();
        return;
    };
    let sysroot = sdk.join("share/wasi-sysroot");
    let clang = clang_path(&sdk);

    let mut objects = Vec::new();
    for unit in LUA_SOURCES {
        let object = out_dir.join(format!("{unit}.o"));
        let status = Command::new(&clang)
            .arg("--target=wasm32-wasip1")
            .arg(format!("--sysroot={}", sysroot.display()))
            .args(["-mllvm", "-wasm-enable-sjlj"])
            .args(["-O2", "-D_WASI_EMULATED_SIGNAL", "-c"])
            .arg(source_dir.join(format!("{unit}.c")))
            .arg("-o")
            .arg(&object)
            .status()
            .unwrap_or_else(|e| {
                panic!("failed to invoke wasi-sdk clang at {}: {e}", clang.display())
            });
        assert!(status.success(), "wasi-sdk clang failed on {unit}.c");
        objects.push(object);
    }

    let archive = out_dir.join("liblua.a");
    let ar = ar_path(&sdk);
    let status = Command::new(&ar)
        .arg("rcs")
        .arg(&archive)
        .args(&objects)
        .status()
        .unwrap_or_else(|e| panic!("failed to invoke wasi-sdk ar at {}: {e}", ar.display()));
    assert!(status.success(), "wasi-sdk ar failed");

    println!("cargo:rustc-link-search=native={}", out_dir.display());
    emit_wasm_link_flags(&sysroot.join("lib/wasm32-wasip1"));
}

/// `wasi-sdk` ships its tools without a `.exe` suffix in its own scripts, but
/// on Windows the binaries on disk are `clang.exe`; `Command` does not apply
/// PATHEXT resolution to an explicit path, so the suffix has to be added here.
fn clang_path(sdk: &Path) -> PathBuf {
    tool_path(sdk, "clang")
}

fn ar_path(sdk: &Path) -> PathBuf {
    tool_path(sdk, "ar")
}

fn tool_path(sdk: &Path, name: &str) -> PathBuf {
    let base = sdk.join("bin").join(name);
    if cfg!(windows) {
        base.with_extension("exe")
    } else {
        base
    }
}

/// Links the committed archive, so a normal checkout needs no wasi-sdk.
fn link_prebuilt() {
    let prebuilt = PathBuf::from("prebuilt/wasm32-unknown-unknown");
    assert!(
        prebuilt.join("liblua.a").exists(),
        "no prebuilt Lua archive at {}, and WASI_SDK is not set.\n\
         Either set WASI_SDK to a wasi-sdk 33+ install, or regenerate the archive \
         with scripts/build-wasm-lua.sh",
        prebuilt.display()
    );
    println!(
        "cargo:rustc-link-search=native={}",
        prebuilt.canonicalize().expect("prebuilt dir").display()
    );
    // The prebuilt archive still needs wasi-libc's static pieces at link time.
    // They are vendored alongside it by scripts/build-wasm-lua.sh.
    println!("cargo:rustc-link-lib=static=lua");
    println!("cargo:rustc-link-lib=static=setjmp");
    println!("cargo:rustc-link-lib=static=c");
    println!("cargo:rustc-link-lib=static=wasi-emulated-signal");
}

fn emit_wasm_link_flags(lib_dir: &Path) {
    println!("cargo:rustc-link-search=native={}", lib_dir.display());
    println!("cargo:rustc-link-lib=static=lua");
    println!("cargo:rustc-link-lib=static=setjmp");
    println!("cargo:rustc-link-lib=static=c");
    println!("cargo:rustc-link-lib=static=wasi-emulated-signal");
}
