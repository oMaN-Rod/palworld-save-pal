use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use psp_core::domain::player;
use psp_core::dto::ordered_map::OrderedMap;
use psp_core::error::CoreError;
use psp_core::gamedata::GameData;
use psp_core::progress::null_progress;
use psp_core::session::{PlayerFileData, SaveKind, SaveSession};

use psp_app::plugin_registry::BUNDLED;
use psp_plugin::manifest::Manifest;
use psp_plugin::runtime::{run_command, RunOutcome, RunRequest, RunServices};
use psp_plugin::sandbox::{Cancel, Limits};
use psp_plugin::status::RunStatus;

use uuid::Uuid;

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("psp-app has a parent directory")
        .to_path_buf()
}

fn fixture_dir() -> PathBuf {
    repo_root().join("tests/fixtures/saves/v1_relics")
}

fn collect_player_file_refs(dir: &Path) -> BTreeMap<Uuid, PlayerFileData> {
    let mut player_file_refs = BTreeMap::new();
    if let Ok(entries) = std::fs::read_dir(dir.join("Players")) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) != Some("sav") {
                continue;
            }
            let stem = path.file_stem().and_then(|s| s.to_str()).unwrap_or_default();
            let (stem, is_dps) = match stem.strip_suffix("_dps") {
                Some(base) => (base, true),
                None => (stem, false),
            };
            let Ok(uid) = Uuid::parse_str(stem) else { continue };
            let slot = player_file_refs
                .entry(uid)
                .or_insert(PlayerFileData::Paths { sav: None, dps: None });
            if let PlayerFileData::Paths { sav, dps } = slot {
                if is_dps { *dps = Some(path) } else { *sav = Some(path) }
            }
        }
    }
    player_file_refs
}

fn load_fixture() -> SaveSession {
    let dir = fixture_dir();
    let level = std::fs::read(dir.join("Level.sav")).expect("the corpus fixture is checked in");
    reparse_with_dir(&dir, &level).expect("the corpus fixture must load; a failure here is a repo bug")
}

fn reparse_with_dir(dir: &Path, level_bytes: &[u8]) -> Result<SaveSession, CoreError> {
    let meta = std::fs::read(dir.join("LevelMeta.sav")).ok();
    let player_file_refs = collect_player_file_refs(dir);
    SaveSession::load(
        SaveKind::Steam { level_path: dir.join("Level.sav") },
        dir.to_string_lossy().into_owned(),
        "steam",
        level_bytes,
        meta.as_deref(),
        None,
        player_file_refs,
        None,
        false,
        &null_progress(),
    )
}

fn reparse(level_bytes: &[u8]) -> Result<SaveSession, CoreError> {
    reparse_with_dir(&fixture_dir(), level_bytes)
}

fn assert_round_trips(session: &SaveSession) {
    let bytes = session.level_sav_bytes().expect("the level serializes");
    reparse(&bytes).unwrap_or_else(|e| panic!("the written save did not reparse: {e}"));
}

fn load_game_data() -> GameData {
    GameData::load(&repo_root().join("data/json")).expect("game data is checked in")
}

struct Harness {
    session: SaveSession,
    game_data: GameData,
    manifest: Manifest,
    sources: BTreeMap<String, String>,
}

impl Harness {
    fn new() -> Self {
        let plugin = BUNDLED
            .iter()
            .find(|p| p.id == "pst.repair")
            .expect("pst.repair is a bundled plugin");
        let manifest = Manifest::parse(plugin.manifest)
            .expect("the bundled manifest must parse");
        let sources = plugin
            .sources
            .iter()
            .map(|(name, content)| (name.to_string(), content.to_string()))
            .collect();
        Self { session: load_fixture(), game_data: load_game_data(), manifest, sources }
    }

    fn run(&mut self, command_id: &str, args: serde_json::Value, dry_run: bool) -> RunOutcome {
        let granted = self.manifest.capabilities.clone();
        run_command(
            RunRequest {
                manifest: &self.manifest,
                sources: &self.sources,
                command_id,
                args: &args,
                dry_run,
                granted: &granted,
            },
            RunServices {
                session: &mut self.session,
                game_data: &self.game_data,
                progress: None,
                storage: &BTreeMap::new(),
                confirm: None,
                limits: Limits::default(),
                cancel: Cancel::new(),
            },
        )
    }
}

#[test]
fn the_bundled_manifest_parses_and_declares_its_commands() {
    let plugin = BUNDLED
        .iter()
        .find(|p| p.id == "pst.repair")
        .expect("pst.repair is a bundled plugin");
    let manifest = Manifest::parse(plugin.manifest).expect("the bundled manifest must parse");
    let mut ids: Vec<&str> = manifest.commands.iter().map(|c| c.id.as_str()).collect();
    ids.sort_unstable();
    assert_eq!(
        ids,
        vec![
            "fix_illegal_pals",
            "fix_illegal_players",
            "fix_invalid_pal_active_skills",
            "repair_structures",
            "scan_illegal_pals",
            "scan_illegal_players",
        ]
    );
}

/// The three widgets the scan-then-pick-then-apply shape is made of. Without
/// all three, this plugin demonstrates nothing.
#[test]
fn the_bundled_view_wires_an_entity_select_a_button_and_a_selectable_table() {
    let plugin = BUNDLED.iter().find(|p| p.id == "pst.repair").expect("pst.repair");
    let manifest = Manifest::parse(plugin.manifest).expect("the manifest parses");
    let widgets: Vec<&psp_plugin::manifest::UiWidget> =
        manifest.ui.iter().flat_map(|section| section.widgets.iter()).collect();

    assert!(
        widgets.iter().any(|w| w.widget_type == "entity_select" && w.entity.as_deref() == Some("player")),
        "the view must offer a player to scan for"
    );
    assert!(
        widgets.iter().any(|w| w.widget_type == "button" && w.command.as_deref() == Some("scan_illegal_pals")),
        "the view must have a button that scans"
    );
    let table = widgets
        .iter()
        .find(|w| w.widget_type == "table" && w.selectable)
        .expect("the view must have a selectable table");
    assert_eq!(table.from.as_deref(), Some("scan_illegal_pals"));

    let fix = widgets
        .iter()
        .find(|w| w.widget_type == "button" && w.command.as_deref() == Some("fix_illegal_pals"))
        .expect("the view must have a button that fixes");
    assert_eq!(
        fix.args.get("ids").map(String::as_str),
        Some("rows.selection"),
        "the fix button must take its ids from the table's selection"
    );
}

#[test]
fn the_scan_finds_nothing_when_every_pal_is_within_the_thresholds() {
    let mut h = Harness::new();
    let outcome = h.run(
        "scan_illegal_pals",
        serde_json::json!({ "owner": "", "max_level": 255, "max_rank": 255 }),
        false,
    );
    assert_eq!(outcome.status, RunStatus::Ok, "{:?}", outcome.status);
    assert_eq!(outcome.counts.get("illegal").copied(), Some(0));
    assert!(
        outcome.counts.get("examined").copied().unwrap_or(0) > 0,
        "the fixture must have pals to examine, or this test is vacuous"
    );
}

#[test]
fn the_scan_reports_a_pal_above_the_threshold_and_names_the_problem() {
    let mut h = Harness::new();
    let outcome =
        h.run("scan_illegal_pals", serde_json::json!({ "owner": "", "max_level": 1, "max_rank": 0 }), false);
    assert_eq!(outcome.status, RunStatus::Ok, "{:?}", outcome.status);
    let illegal = outcome.counts.get("illegal").copied().unwrap_or(0);
    assert!(illegal > 0, "at level 1 the fixture must have illegal pals; counts {:?}", outcome.counts);

    let result = outcome.result.as_ref().expect("the scan returns its rows");
    let pals = result.get("pals").and_then(|v| v.as_array()).expect("a pals array");
    assert_eq!(pals.len() as i64, illegal, "every counted pal must be in the result");
    let first = pals.first().expect("at least one row");
    assert!(first.get("instance_id").and_then(|v| v.as_str()).is_some());
    assert!(
        first.get("problems").and_then(|v| v.as_str()).map(|p| p.contains("level")).unwrap_or(false),
        "the row must say what is wrong with it: {first:?}"
    );
}

#[test]
fn the_scan_narrows_to_one_owner() {
    let mut h = Harness::new();
    let all = h.run(
        "scan_illegal_pals",
        serde_json::json!({ "owner": "", "max_level": 1, "max_rank": 0 }),
        false,
    );
    let all_count = all.counts.get("examined").copied().unwrap_or(0);

    let owner = h.session.player_summary_order[0].to_string();
    let one = h.run(
        "scan_illegal_pals",
        serde_json::json!({ "owner": owner, "max_level": 1, "max_rank": 0 }),
        false,
    );
    let one_count = one.counts.get("examined").copied().unwrap_or(0);

    assert!(one_count > 0, "the first fixture player must own at least one pal");
    assert!(one_count < all_count, "one owner's pals must be fewer than every pal ({one_count} vs {all_count})");
}

/// End to end: pick a player, scan, select rows, apply -- and check the save,
/// not the widget.
#[test]
fn scanning_then_fixing_the_selection_writes_the_clamps_to_the_save() {
    let mut h = Harness::new();
    let owner = h.session.player_summary_order[0].to_string();

    let scan = h.run(
        "scan_illegal_pals",
        serde_json::json!({ "owner": owner, "max_level": 1, "max_rank": 0 }),
        false,
    );
    assert_eq!(scan.status, RunStatus::Ok, "{:?}", scan.status);
    let pals = scan
        .result
        .as_ref()
        .and_then(|v| v.get("pals"))
        .and_then(|v| v.as_array())
        .expect("the scan returns rows")
        .clone();
    assert!(!pals.is_empty(), "the first fixture player must own a pal above level 1");

    // The selection: exactly what a user picking rows in the table would send.
    let selected: Vec<String> = pals
        .iter()
        .filter_map(|row| row.get("instance_id").and_then(|v| v.as_str()).map(str::to_string))
        .collect();
    let spared_levels_before: Vec<(Uuid, i64)> = {
        let all = psp_core::domain::pal::pal_summaries(&h.session, &h.game_data)
            .expect("pal summaries");
        all.iter()
            .filter(|s| !selected.iter().any(|chosen| chosen == &s.instance_id.to_string()))
            .map(|s| (s.instance_id, s.level))
            .collect()
    };
    assert!(
        !spared_levels_before.is_empty(),
        "the fixture must have pals this owner does not own, or nothing proves the fix was selective"
    );

    let fix = h.run(
        "fix_illegal_pals",
        serde_json::json!({ "ids": selected, "max_level": 1, "max_rank": 0 }),
        false,
    );
    assert_eq!(fix.status, RunStatus::Ok, "{:?}", fix.status);
    assert_eq!(
        fix.counts.get("missing").copied(),
        Some(0),
        "every selected pal must have been found: {:?}",
        fix.counts
    );
    assert!(fix.counts.get("clamps").copied().unwrap_or(0) > 0);

    // Read the save back, not the widget.
    let after = psp_core::domain::pal::pal_summaries(&h.session, &h.game_data)
        .expect("pal summaries after the fix");
    for id in &selected {
        let summary = after
            .iter()
            .find(|s| &s.instance_id.to_string() == id)
            .unwrap_or_else(|| panic!("pal {id} must still exist"));
        assert!(summary.level <= 1, "pal {id} must have been clamped, got level {}", summary.level);
    }
    for (id, before) in spared_levels_before {
        let now = after.iter().find(|s| s.instance_id == id).map(|s| s.level);
        assert_eq!(now, Some(before), "pal {id} was not selected and must be untouched");
    }
}

/// The preview must change nothing, and must predict what the real run does.
#[test]
fn a_dry_run_changes_nothing_and_predicts_the_real_counts() {
    let ids: Vec<String> = {
        let mut h = Harness::new();
        let scan =
            h.run("scan_illegal_pals", serde_json::json!({ "owner": "", "max_level": 1, "max_rank": 0 }), false);
        scan.result
            .as_ref()
            .and_then(|v| v.get("pals"))
            .and_then(|v| v.as_array())
            .expect("rows")
            .iter()
            .filter_map(|row| row.get("instance_id").and_then(|v| v.as_str()).map(str::to_string))
            .collect()
    };
    assert!(!ids.is_empty(), "the fixture must have pals above level 1");

    let mut h = Harness::new();
    let args = serde_json::json!({ "ids": ids, "max_level": 1, "max_rank": 0 });

    let before = h.session.level_sav_bytes().expect("level_sav_bytes before the dry run");
    let dry = h.run("fix_illegal_pals", args.clone(), true);
    assert_eq!(dry.status, RunStatus::Ok, "{:?}", dry.status);
    let after_dry = h.session.level_sav_bytes().expect("level_sav_bytes after the dry run");
    assert_eq!(before, after_dry, "a dry run must not change level_sav_bytes()");

    let real = h.run("fix_illegal_pals", args, false);
    assert_eq!(real.status, RunStatus::Ok, "{:?}", real.status);
    for (key, value) in &real.counts {
        assert_eq!(dry.counts.get(key), Some(value), "dry/real mismatch for {key:?}");
    }
}

#[test]
fn fixing_an_id_that_is_not_in_the_save_reports_it_rather_than_failing() {
    let mut h = Harness::new();
    let outcome = h.run(
        "fix_illegal_pals",
        serde_json::json!({ "ids": ["00000000-0000-0000-0000-000000000000"], "max_level": 1, "max_rank": 0 }),
        false,
    );
    assert_eq!(outcome.status, RunStatus::Ok, "{:?}", outcome.status);
    assert_eq!(outcome.counts.get("requested").copied(), Some(1));
    assert_eq!(outcome.counts.get("missing").copied(), Some(1));
    assert_eq!(outcome.counts.get("pals").copied(), Some(0));
}

#[test]
fn fixing_nothing_is_a_no_op_rather_than_an_error() {
    let mut h = Harness::new();
    let before = h.session.level_sav_bytes().expect("level_sav_bytes");
    let outcome = h.run(
        "fix_illegal_pals",
        serde_json::json!({ "ids": [], "max_level": 1, "max_rank": 0 }),
        false,
    );
    assert_eq!(outcome.status, RunStatus::Ok, "{:?}", outcome.status);
    assert_eq!(outcome.counts.get("pals").copied(), Some(0));
    assert_eq!(before, h.session.level_sav_bytes().expect("level_sav_bytes"));
}

#[test]
fn the_fixed_save_reparses() {
    let ids: Vec<String> = {
        let mut h = Harness::new();
        let scan =
            h.run("scan_illegal_pals", serde_json::json!({ "owner": "", "max_level": 1, "max_rank": 0 }), false);
        scan.result
            .as_ref()
            .and_then(|v| v.get("pals"))
            .and_then(|v| v.as_array())
            .expect("rows")
            .iter()
            .filter_map(|row| row.get("instance_id").and_then(|v| v.as_str()).map(str::to_string))
            .collect()
    };

    let mut h = Harness::new();
    let outcome = h.run(
        "fix_illegal_pals",
        serde_json::json!({ "ids": ids, "max_level": 1, "max_rank": 0 }),
        false,
    );
    assert_eq!(outcome.status, RunStatus::Ok, "{:?}", outcome.status);
    let bytes = h.session.level_sav_bytes().expect("level_sav_bytes");
    reparse(&bytes).expect("the written save must reparse");
}

#[test]
fn repair_structures_raises_every_damaged_map_object_to_full_hp() {
    let mut h = Harness::new();

    // MapObjectView.hp and .max_hp are plain i32, not Option -- verified at
    // psp-core/src/domain/map_object.rs:13. The Lua handle's fields can still
    // read nil, which is what the command's own guard is for.
    let damaged_before = psp_core::domain::map_object::map_object_views(&h.session)
        .expect("map objects read")
        .iter()
        .filter(|v| v.hp < v.max_hp)
        .count();

    let outcome = h.run("repair_structures", serde_json::json!({}), false);
    assert_eq!(outcome.status, RunStatus::Ok, "{:?}", outcome.status);

    let result = outcome.result.expect("a result table");
    assert_eq!(
        result["counts"]["repaired"].as_i64(),
        Some(damaged_before as i64),
        "every damaged object must be repaired, and no undamaged one counted"
    );

    let still_damaged = psp_core::domain::map_object::map_object_views(&h.session)
        .expect("map objects read")
        .iter()
        .filter(|v| v.hp < v.max_hp)
        .count();
    assert_eq!(still_damaged, 0, "no map object may be left below its maximum");

    assert_round_trips(&h.session);
}

#[test]
fn repair_structures_under_a_dry_run_changes_nothing_but_reports_the_same_count() {
    let mut h = Harness::new();
    let before = psp_core::domain::map_object::map_object_views(&h.session)
        .expect("map objects read")
        .iter()
        .map(|v| (v.instance_id, v.hp))
        .collect::<std::collections::BTreeMap<_, _>>();

    let dry = h.run("repair_structures", serde_json::json!({}), true);
    assert_eq!(dry.status, RunStatus::Ok, "{:?}", dry.status);
    let predicted = dry.result.expect("a result")["counts"]["repaired"].as_i64();

    let after = psp_core::domain::map_object::map_object_views(&h.session)
        .expect("map objects read")
        .iter()
        .map(|v| (v.instance_id, v.hp))
        .collect::<std::collections::BTreeMap<_, _>>();
    assert_eq!(before, after, "a dry run must not move a single hp value");

    let real = h.run("repair_structures", serde_json::json!({}), false);
    assert_eq!(
        real.result.expect("a result")["counts"]["repaired"].as_i64(),
        predicted,
        "the dry run's prediction must match what the real run does"
    );
}

/// The corpus fixture is a real save, so it may or may not contain an
/// over-cap player. Both branches are asserted rather than assuming one.
#[test]
fn scan_illegal_players_reports_exactly_the_players_over_the_cap() {
    let mut h = Harness::new();
    let outcome = h.run(
        "scan_illegal_players",
        serde_json::json!({ "max_points": 50 }),
        false,
    );
    assert_eq!(outcome.status, RunStatus::Ok, "{:?}", outcome.status);
    let result = outcome.result.expect("a result table");

    let examined = result["counts"]["examined"].as_i64().expect("examined");
    assert!(examined > 0, "the fixture must contain at least one player");

    // An empty Lua table has no length hint, so the host marshals it as a
    // JSON object rather than an array -- zero rows must still be accepted.
    let empty = Vec::new();
    let rows = result["players"].as_array().unwrap_or(&empty);
    assert_eq!(
        result["counts"]["illegal"].as_i64(),
        Some(rows.len() as i64),
        "the illegal count must equal the number of rows returned"
    );
    for row in rows {
        assert!(row["uid"].is_string(), "every row carries a uid for the fix step");
        assert!(
            row["worst"].as_i64().expect("worst") > 50,
            "a row may only be reported when some stat exceeds the cap"
        );
    }
}

/// Drives the pair end to end against a value this test puts over the cap
/// itself, so it does not depend on the fixture containing bad data. The
/// seeding goes through `psp_core::domain::player` directly -- there is no
/// `player_summaries()` free function, and no `__seed_over_cap` command is
/// shipped in the manifest -- reading and writing the DTO the same way
/// `bundled_tools.rs` does for its own player fixtures.
#[test]
fn fix_illegal_players_clamps_a_stat_the_scan_reported() {
    let mut h = Harness::new();

    let player_id = *h
        .session
        .player_summaries
        .keys()
        .next()
        .expect("the fixture has a player");
    let uid = player_id.to_string();

    let mut dto = player::get_player_details(&mut h.session, &h.game_data, player_id, &null_progress())
        .expect("player read")
        .expect("the player exists");
    dto.status_point_list.insert("max_hp".to_string(), 99);
    let mut modified = OrderedMap::new();
    modified.insert(player_id, dto);
    player::update_players(&mut h.session, &h.game_data, &modified, &null_progress())
        .expect("the seed write must succeed");

    let scan = h.run("scan_illegal_players", serde_json::json!({ "max_points": 50 }), false);
    let rows = scan.result.expect("a result")["players"].as_array().expect("rows").clone();
    assert!(
        rows.iter().any(|r| r["uid"].as_str() == Some(uid.as_str())),
        "the seeded player must be reported"
    );

    let fixed = h.run(
        "fix_illegal_players",
        serde_json::json!({ "ids": [uid.clone()], "max_points": 50 }),
        false,
    );
    assert_eq!(fixed.status, RunStatus::Ok, "{:?}", fixed.status);
    assert_eq!(fixed.result.expect("a result")["counts"]["clamps"].as_i64(), Some(1));

    let rescan = h.run("scan_illegal_players", serde_json::json!({ "max_points": 50 }), false);
    let rescan_result = rescan.result.expect("a result");
    let rows = rescan_result["players"].as_array().cloned().unwrap_or_default();
    assert!(
        !rows.iter().any(|r| r["uid"].as_str() == Some(uid.as_str())),
        "the clamped player must no longer be reported"
    );

    assert_round_trips(&h.session);
}

#[test]
fn fix_invalid_pal_active_skills_never_leaves_a_pal_with_an_unlearnable_skill() {
    let mut h = Harness::new();
    let outcome = h.run("fix_invalid_pal_active_skills", serde_json::json!({}), false);
    assert_eq!(outcome.status, RunStatus::Ok, "{:?}", outcome.status);
    let counts = outcome.result.expect("a result")["counts"].clone();

    let examined = counts["examined"].as_i64().expect("examined");
    let skipped_unknown_species = counts["skipped_unknown_species"].as_i64().expect("skipped_unknown_species");
    assert!(examined > 0);
    assert!(
        skipped_unknown_species * 10 < examined,
        "species resolution must succeed for nearly every pal in the fixture; \
         got {skipped_unknown_species} unresolved of {examined} examined"
    );

    // Re-running must be a no-op: the first run either fixed a pal or
    // deliberately skipped it, and neither is a state a second run improves.
    let again = h.run("fix_invalid_pal_active_skills", serde_json::json!({}), false);
    assert_eq!(
        again.result.expect("a result")["counts"]["removed"].as_i64(),
        Some(0),
        "the command must converge in one run"
    );

    assert_round_trips(&h.session);
}

/// The trap: a save containing a pal whose skills are not in the catalog must
/// not fail the run. This asserts the skip path exists and is counted.
#[test]
fn fix_invalid_pal_active_skills_skips_rather_than_fails_on_an_uncatalogued_skill() {
    let mut h = Harness::new();
    let outcome = h.run("fix_invalid_pal_active_skills", serde_json::json!({}), false);
    assert_eq!(
        outcome.status,
        RunStatus::Ok,
        "a pal holding a skill the catalog does not know must be skipped, never raise: {:?}",
        outcome.status
    );
    let counts = outcome.result.expect("a result")["counts"].clone();
    assert!(
        counts["skipped_uncatalogued"].is_number(),
        "the skip must be counted and reported, not silent"
    );
    assert!(
        counts["skipped_unknown_species"].is_number(),
        "a pal whose species does not resolve must be counted separately"
    );
}
