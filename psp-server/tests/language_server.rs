use psp_server::services::language_server as ls;

#[test]
fn every_supported_host_has_a_pinned_release() {
    let release = ls::release_for_host();
    if cfg!(any(
        all(target_os = "windows", target_arch = "x86_64"),
        all(target_os = "linux", target_arch = "x86_64"),
        target_os = "macos",
    )) {
        let release = release.expect("this host must have a pinned release");
        assert!(release.url.starts_with("https://github.com/LuaLS/"));
        assert_eq!(release.sha256.len(), 64, "a sha256 is 64 hex characters");
        assert!(release.sha256.chars().all(|c| c.is_ascii_hexdigit()));
        assert!(!release.exe_relative.is_empty());
    }
}

// Restates every pinned version, digest, asset name, and exe path verbatim
// so an unreviewed edit to any of the five entries fails the suite
// immediately on any host, rather than only on the one host whose entry
// `release_for_host` happens to select, or only on a network run CI never
// performs.
#[test]
fn every_pinned_release_matches_the_verified_table() {
    let expected: &[(&str, &str, &str)] = &[
        (
            "lua-language-server-3.19.1-win32-x64.zip",
            "fdb9a59108cf62517813c97fa5549b0e16d1ef0688306bac728b08434db7e4cd",
            "bin/lua-language-server.exe",
        ),
        (
            "lua-language-server-3.19.1-linux-x64.tar.gz",
            "e9235d2d72ef55bc41cf8c99cda2ed64777682024b4bb81f5dea425060c5cbb8",
            "bin/lua-language-server",
        ),
        (
            "lua-language-server-3.19.1-linux-arm64.tar.gz",
            "abd2572e8fc929dc838a81ffb8473c5bce0bf39bfe8edb4b120b3b623176ce83",
            "bin/lua-language-server",
        ),
        (
            "lua-language-server-3.19.1-darwin-arm64.tar.gz",
            "0bc077f4447f076b4c92c14e9fd303f5b569eda2ec74b4dca2b55f75fae2e90c",
            "bin/lua-language-server",
        ),
        (
            "lua-language-server-3.19.1-darwin-x64.tar.gz",
            "eb373c159cbe556711d7cd316315de2dce969bfd54b31edb7eb9cab2937f2cca",
            "bin/lua-language-server",
        ),
    ];

    let releases = ls::all_releases();
    assert_eq!(
        releases.len(),
        expected.len(),
        "the release table must have exactly one entry per supported host"
    );

    for (asset_name, expected_sha256, expected_exe_relative) in expected {
        let release = releases
            .iter()
            .find(|release| release.url.ends_with(asset_name))
            .unwrap_or_else(|| panic!("no pinned release for asset {asset_name}"));
        assert_eq!(release.version, "3.19.1");
        assert_eq!(release.sha256, *expected_sha256);
        assert_eq!(release.exe_relative, *expected_exe_relative);
        assert!(
            release.url.contains(release.version),
            "url must contain the pinned version: {}",
            release.url
        );
    }
}

#[test]
fn each_release_platform_field_matches_its_asset_name() {
    for release in ls::all_releases() {
        let asset = release
            .url
            .rsplit('/')
            .next()
            .expect("url must have a final path segment");
        let (expected_os, expected_arch) = if asset.contains("win32-x64") {
            (ls::HostOs::Windows, ls::HostArch::X86_64)
        } else if asset.contains("linux-x64") {
            (ls::HostOs::Linux, ls::HostArch::X86_64)
        } else if asset.contains("linux-arm64") {
            (ls::HostOs::Linux, ls::HostArch::Aarch64)
        } else if asset.contains("darwin-arm64") {
            (ls::HostOs::Macos, ls::HostArch::Aarch64)
        } else if asset.contains("darwin-x64") {
            (ls::HostOs::Macos, ls::HostArch::X86_64)
        } else {
            panic!("unrecognized pinned asset name: {asset}");
        };
        assert_eq!(release.os, expected_os, "wrong os for asset {asset}");
        assert_eq!(release.arch, expected_arch, "wrong arch for asset {asset}");
    }
}

#[test]
fn a_matching_digest_verifies_and_a_mismatched_one_does_not() {
    let empty = "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855";
    assert!(ls::verify_sha256(b"", empty));
    assert!(!ls::verify_sha256(b"x", empty));
}

#[test]
fn digest_comparison_ignores_hex_case() {
    let empty_upper = "E3B0C44298FC1C149AFBF4C8996FB92427AE41E4649B934CA495991B7852B855";
    assert!(ls::verify_sha256(b"", empty_upper));
}

#[test]
fn an_absent_install_root_reports_no_executable() {
    let dir = tempfile::tempdir().expect("a temp dir");
    assert!(ls::installed_exe(&dir.path().join("does-not-exist")).is_none());
}

#[test]
fn an_install_root_holding_the_executable_reports_it() {
    let Some(release) = ls::release_for_host() else {
        return;
    };
    let dir = tempfile::tempdir().expect("a temp dir");
    let exe = dir.path().join(release.exe_relative);
    std::fs::create_dir_all(exe.parent().expect("a parent")).expect("mkdir");
    std::fs::write(&exe, b"stub").expect("write");
    assert_eq!(ls::installed_exe(dir.path()), Some(exe));
}

#[tokio::test]
#[ignore]
async fn ensure_downloads_verifies_and_extracts_the_real_release() {
    let Some(release) = ls::release_for_host() else {
        return;
    };
    let dir = tempfile::tempdir().expect("a temp dir");

    let exe = ls::ensure(dir.path())
        .await
        .expect("ensure should download and extract the pinned release");

    assert!(exe.exists(), "the returned exe path must exist");
    assert_eq!(exe, dir.path().join(release.exe_relative));

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mode = std::fs::metadata(&exe)
            .expect("metadata")
            .permissions()
            .mode();
        assert!(mode & 0o111 != 0, "the extracted binary must be executable");
    }

    let root = dir.path();
    assert!(root.join("bin").is_dir(), "bin/ must exist at the install root");
    assert!(
        root.join("main.lua").is_file(),
        "main.lua must sit directly at the install root, not under a version-prefixed wrapper directory"
    );
    assert!(root.join("meta").is_dir(), "meta/ must exist at the install root");
    assert!(root.join("script").is_dir(), "script/ must exist at the install root");

    #[cfg(windows)]
    {
        let bin_dir = exe.parent().expect("bin dir");
        let has_dll = std::fs::read_dir(bin_dir)
            .expect("read bin dir")
            .filter_map(|entry| entry.ok())
            .any(|entry| {
                entry
                    .path()
                    .extension()
                    .is_some_and(|extension| extension.eq_ignore_ascii_case("dll"))
            });
        assert!(
            has_dll,
            "a Windows install must ship its MSVC runtime DLLs beside the exe"
        );
    }
}

#[tokio::test]
#[ignore]
async fn ensure_leaves_no_files_behind_on_a_digest_mismatch() {
    let Some(mut release) = ls::release_for_host() else {
        return;
    };
    release.sha256 = "0000000000000000000000000000000000000000000000000000000000000000";
    // A dedicated parent, not `root` itself, so the assertion below also
    // reaches the sibling `.partial` staging directory `ensure_release`
    // would create beside `root` — a directory outside this parent could
    // hide a leak the way asserting only `root` is empty would.
    let parent = tempfile::tempdir().expect("a temp dir");
    let root = parent.path().join("install");

    let result = ls::ensure_release(&root, &release).await;

    assert!(result.is_err(), "a mismatched digest must fail ensure");
    let leftover: Vec<_> = std::fs::read_dir(parent.path())
        .expect("read parent dir")
        .filter_map(|entry| entry.ok())
        .map(|entry| entry.file_name())
        .collect();
    assert!(
        leftover.is_empty(),
        "a failed digest check must leave nothing behind, found: {leftover:?}"
    );
}
