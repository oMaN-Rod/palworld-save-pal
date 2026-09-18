use std::path::Path;

use ps_core::mods::{
    build_manifest, parse_modinfo, AnalyzeInput, ArchiveEntry, Platform, TargetKind,
};
use serde::Deserialize;

#[derive(Deserialize)]
struct Fixture {
    archive_name: String,
    platform: Platform,
    #[serde(default)]
    modinfo: Option<serde_json::Value>,
    entries: Vec<String>,
    expect: Expect,
}

#[derive(Deserialize)]
struct Expect {
    mod_type: ps_core::mods::ModType,
    folder_name: String,
    #[serde(default)]
    platform_filtered: Option<Platform>,
    routes: Vec<(String, ps_core::mods::RouteKind, String)>,
    #[serde(default)]
    decisions: Vec<String>,
}

fn run_fixture(path: &Path) {
    let text = std::fs::read_to_string(path).unwrap();
    let fx: Fixture =
        serde_json::from_str(&text).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    let entries: Vec<ArchiveEntry> = fx
        .entries
        .iter()
        .map(|p| ArchiveEntry {
            path: p.clone(),
            size: 1,
        })
        .collect();
    let modinfo = fx
        .modinfo
        .as_ref()
        .map(|v| parse_modinfo(&v.to_string()).unwrap());
    let manifest = build_manifest(&AnalyzeInput {
        entries: &entries,
        archive_name: &fx.archive_name,
        target_platform: fx.platform,
        target_kind: TargetKind::Client,
        modinfo: modinfo.as_ref(),
        workshop_info: None,
        custom_name: None,
    });
    let name = path.file_name().unwrap().to_string_lossy();
    assert_eq!(manifest.mod_type, fx.expect.mod_type, "{name}: mod_type");
    assert_eq!(
        manifest.folder_name, fx.expect.folder_name,
        "{name}: folder_name"
    );
    assert_eq!(
        manifest.platform_filtered, fx.expect.platform_filtered,
        "{name}: platform_filtered"
    );
    // Sorted vectors, not sets: a duplicated route must fail the fixture rather
    // than collapse on both sides.
    let mut got: Vec<(String, String, String)> = manifest
        .routes
        .iter()
        .map(|r| {
            (
                r.archive_path.clone(),
                serde_json::to_string(&r.kind).unwrap(),
                r.rel_path.clone(),
            )
        })
        .collect();
    got.sort();
    let mut want: Vec<(String, String, String)> = fx
        .expect
        .routes
        .iter()
        .map(|(a, k, r)| (a.clone(), serde_json::to_string(k).unwrap(), r.clone()))
        .collect();
    want.sort();
    assert_eq!(got, want, "{name}: routes");
    let got_decisions: Vec<String> = manifest
        .decisions
        .iter()
        .map(|d| {
            serde_json::to_value(d).unwrap()["kind"]
                .as_str()
                .unwrap()
                .to_string()
        })
        .collect();
    assert_eq!(got_decisions, fx.expect.decisions, "{name}: decisions");
}

#[test]
fn every_fixture_matches() {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/mods");
    let mut paths: Vec<_> = std::fs::read_dir(&dir)
        .unwrap()
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|e| e == "json"))
        .collect();
    paths.sort();
    assert!(!paths.is_empty(), "no fixtures in {}", dir.display());
    for p in paths {
        run_fixture(&p);
    }
}
