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
            "fix_all_pals",
            "fix_illegal_pals",
            "fix_illegal_players",
            "fix_invalid_pal_active_skills",
            "repair_items",
            "repair_structures",
            "scan_illegal_pals",
            "scan_illegal_players",
            "trim_overfilled_inventories",
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

/// Sickness markers on NON-PLAYER character entries only. `restore_pals` skips
/// player entries by design, so a world-wide count would assert a behaviour the
/// command never promised.
fn non_player_sickness_markers(session: &SaveSession) -> usize {
    const MARKERS: [&str; 3] = ["WorkerSick", "PhysicalHealth", "PalReviveTimer"];
    psp_core::domain::world::character_map(&session.level)
        .expect("the character map resolves")
        .iter()
        .filter(|entry| !psp_core::domain::world::entry_is_player(entry))
        .filter_map(psp_core::domain::world::entry_save_parameter)
        .map(|params| {
            params
                .into_iter()
                .filter(|(key, _)| MARKERS.contains(&key.1.as_str()))
                .count()
        })
        .sum()
}

/// Marks the first `count` pals sick and starving through the raw property bag,
/// so the heal assertions cannot pass on a fixture that was already healthy.
fn seed_sick_pals(session: &mut SaveSession, count: usize) -> usize {
    let mut seeded = 0;
    for entry in psp_core::domain::world::character_map_mut(&mut session.level)
        .expect("the character map resolves")
        .iter_mut()
    {
        if seeded == count {
            break;
        }
        if psp_core::domain::world::entry_is_player(entry) {
            continue;
        }
        let Some(params) = psp_core::domain::world::entry_save_parameter_mut(entry) else {
            continue;
        };
        params.insert("WorkerSick", psp_core::props::bool_property(true));
        params.insert("SanityValue", psp_core::props::float_property(3.0));
        seeded += 1;
    }
    seeded
}

/// Orphans one pal that a player container really holds, and returns it with the
/// owner it had. Picked through `PlayerDto`'s own pal box and party rather than
/// guessed, so the pal is one the command can actually resolve an owner for.
fn orphan_a_contained_pal(h: &mut Harness) -> (Uuid, Uuid) {
    let player_id = *h.session.player_summaries.keys().next().expect("a player");
    let details =
        player::get_player_details(&mut h.session, &h.game_data, player_id, &null_progress())
            .expect("player read")
            .expect("the player exists");
    let pal_id = details
        .pal_box
        .iter()
        .chain(details.party.iter())
        .flat_map(|container| container.slots.iter())
        .find_map(|slot| slot.pal_id)
        .expect("the first fixture player must hold a pal in its box or party");

    let entries = psp_core::domain::world::character_map_mut(&mut h.session.level)
        .expect("the character map resolves");
    let entry = entries
        .iter_mut()
        .find(|entry| psp_core::domain::world::entry_instance_id(entry) == Some(pal_id))
        .expect("the pal the container names must be in the character map");
    let params = psp_core::domain::world::entry_save_parameter_mut(entry).expect("save parameter");
    let previous = params
        .into_iter()
        .find(|(key, _)| key.1.as_str() == "OwnerPlayerUId")
        .and_then(|(_, value)| psp_core::props::as_uuid(value))
        .expect("a pal in a player container has an owner to begin with");
    params
        .0
        .shift_remove(&psp_core::ue::PropertyKey::from("OwnerPlayerUId"));
    (pal_id, previous)
}

fn owner_of(session: &SaveSession, pal_id: Uuid) -> Option<Uuid> {
    psp_core::domain::world::character_map(&session.level)
        .expect("the character map resolves")
        .iter()
        .find(|entry| psp_core::domain::world::entry_instance_id(entry) == Some(pal_id))
        .and_then(psp_core::domain::world::entry_save_parameter)
        .and_then(|params| {
            params
                .into_iter()
                .find(|(key, _)| key.1.as_str() == "OwnerPlayerUId")
                .and_then(|(_, value)| psp_core::props::as_uuid(value))
        })
}

#[test]
fn fix_all_pals_predicts_under_a_dry_run_exactly_what_it_does_for_real() {
    let mut h = Harness::new();
    assert!(seed_sick_pals(&mut h.session, 5) > 0, "the fixture must have pals to break");
    orphan_a_contained_pal(&mut h);

    let before = h.session.level_sav_bytes().expect("level_sav_bytes before the dry run");
    let dry = h.run("fix_all_pals", serde_json::json!({}), true);
    assert_eq!(dry.status, RunStatus::Ok, "{:?}", dry.status);
    let predicted = dry.result.expect("a result")["counts"].clone();
    assert_eq!(
        before,
        h.session.level_sav_bytes().expect("level_sav_bytes after the dry run"),
        "a dry run must not change level_sav_bytes()"
    );

    let real = h.run("fix_all_pals", serde_json::json!({}), false);
    assert_eq!(real.status, RunStatus::Ok, "{:?}", real.status);
    let actual = real.result.expect("a result")["counts"].clone();

    assert_eq!(predicted["restored"], actual["restored"]);
    assert_eq!(predicted["owners_assigned"], actual["owners_assigned"]);
    assert!(
        actual["restored"].as_i64().unwrap_or(0) > 0,
        "the fixture must have restored something, or the parity above is vacuous"
    );
    assert!(
        actual["owners_assigned"].as_i64().unwrap_or(0) > 0,
        "the pal orphaned above must have been given an owner, or the parity above \
         proves nothing about the owner half"
    );
    assert_round_trips(&h.session);
}

#[test]
fn fix_all_pals_leaves_every_pal_at_full_sanity_and_free_of_sickness() {
    let mut h = Harness::new();
    assert!(seed_sick_pals(&mut h.session, 5) > 0, "the fixture must have pals to break");
    assert!(
        non_player_sickness_markers(&h.session) > 0,
        "the seed must have actually made a pal sick"
    );

    let outcome = h.run("fix_all_pals", serde_json::json!({}), false);
    assert_eq!(outcome.status, RunStatus::Ok, "{:?}", outcome.status);
    let counts = outcome.result.expect("a result")["counts"].clone();
    assert!(counts["restored"].as_i64().unwrap_or(0) > 0, "counts {counts:?}");

    assert_eq!(
        non_player_sickness_markers(&h.session),
        0,
        "no sickness marker may survive a fix_all_pals run"
    );
    assert_round_trips(&h.session);
}

#[test]
fn fix_all_pals_gives_an_orphaned_pal_back_the_owner_of_its_container() {
    let mut h = Harness::new();

    let baseline = h.run("fix_all_pals", serde_json::json!({}), true);
    assert_eq!(baseline.status, RunStatus::Ok, "{:?}", baseline.status);
    let already_ownerless = baseline.result.expect("a result")["counts"]["owners_assigned"]
        .as_i64()
        .expect("owners_assigned");

    let (pal_id, previous_owner) = orphan_a_contained_pal(&mut h);
    assert_eq!(owner_of(&h.session, pal_id), None, "the seed must have cleared the owner");

    let outcome = h.run("fix_all_pals", serde_json::json!({}), false);
    assert_eq!(outcome.status, RunStatus::Ok, "{:?}", outcome.status);
    assert_eq!(
        outcome.result.expect("a result")["counts"]["owners_assigned"].as_i64(),
        Some(already_ownerless + 1),
        "exactly the pal orphaned here is the one extra owner assigned"
    );
    assert_eq!(
        owner_of(&h.session, pal_id),
        Some(previous_owner),
        "the owner must come back from the container that holds the pal"
    );
    assert_round_trips(&h.session);
}

/// The corpus fixture carries no broken link at all, so this makes one: an
/// equipment slot's `DynamicItemSaveData` entry is deleted out from under it,
/// which is exactly the damage `repair_items` exists to undo. Without it every
/// assertion below would be `0 == 0`.
///
/// Returns the container and slot index that were broken.
fn break_one_item_link(session: &mut SaveSession) -> (Uuid, i32) {
    use psp_core::domain::world;
    use psp_core::props;
    use psp_core::ue::{Property, PropertyKey, StructValue};

    let (container_id, slot_index, local_id) = world::item_container_map(&session.level)
        .expect("the container map reads")
        .iter()
        .find_map(|entry| {
            let container_id = props::struct_props(&entry.key)
                .and_then(|key| props::get(key, &["ID"]))
                .and_then(props::as_uuid)?;
            let slots = props::struct_props(&entry.value)
                .and_then(|value| props::get(value, &["Slots"]))
                .and_then(props::struct_values)?;
            slots.iter().find_map(|slot| {
                let StructValue::Struct(slot_props) = slot else { return None };
                let Some(Property::Struct(StructValue::Game(
                    psp_core::ue::PalStruct::ItemContainerSlots(raw),
                ))) = slot_props.0.get(&PropertyKey::from("RawData"))
                else {
                    return None;
                };
                let local_id = props::guid_to_uuid(&raw.item.dynamic_id.local_id_in_created_world);
                (local_id != props::EMPTY_UUID)
                    .then_some((container_id, raw.slot_index, local_id))
            })
        })
        .expect("fixture precondition: the corpus must hold a slot backed by a record");

    let values = world::dynamic_item_values_mut(&mut session.level).expect("the array reads");
    let before = values.len();
    values.retain(|value| {
        let StructValue::Struct(item_props) = value else { return true };
        !matches!(
            item_props.0.get(&PropertyKey::from("RawData")),
            Some(Property::Struct(StructValue::Game(psp_core::ue::PalStruct::DynamicItem(item))))
                if props::guid_to_uuid(&item.id.local_id_in_created_world) == local_id
        )
    });
    assert_eq!(before - values.len(), 1, "exactly one record must be deleted");
    session.caches.dynamic_item_index = None;
    (container_id, slot_index)
}

fn container_holds_slot(session: &mut SaveSession, game_data: &GameData, container_id: Uuid, slot_index: i32) -> bool {
    psp_core::domain::containers::read_item_container(
        &session.level,
        &mut session.caches,
        game_data,
        container_id,
        "CommonContainer",
        None,
    )
    .expect("the container resolves")
    .slots
    .iter()
    .any(|slot| slot.slot_index == slot_index)
}

#[test]
fn repair_items_converges_and_round_trips() {
    let mut h = Harness::new();
    let (container_id, slot_index) = break_one_item_link(&mut h.session);
    assert!(
        !container_holds_slot(&mut h.session, &h.game_data, container_id, slot_index),
        "precondition: a slot whose record is gone is dropped by the reader, not reported"
    );

    let dry = h.run("repair_items", serde_json::json!({}), true);
    assert_eq!(dry.status, RunStatus::Ok, "{:?}", dry.status);
    let predicted = dry.result.expect("a result")["counts"]["repaired"].as_i64();
    assert_eq!(predicted, Some(1), "the dry run must see the one broken link");
    assert!(
        !container_holds_slot(&mut h.session, &h.game_data, container_id, slot_index),
        "a dry run must not have repaired anything"
    );

    let real = h.run("repair_items", serde_json::json!({}), false);
    assert_eq!(real.status, RunStatus::Ok, "{:?}", real.status);
    assert_eq!(
        real.result.expect("a result")["counts"]["repaired"].as_i64(),
        predicted,
        "the dry run must predict exactly what the real run does"
    );

    // The user-visible point of the command: the item comes back instead of
    // being silently deleted the next time its container is written.
    assert!(
        container_holds_slot(&mut h.session, &h.game_data, container_id, slot_index),
        "the repaired slot must survive the reader again"
    );

    let again = h.run("repair_items", serde_json::json!({}), false);
    assert_eq!(
        again.result.expect("a result")["counts"]["repaired"].as_i64(),
        Some(0),
        "a second run must find nothing left to repair"
    );

    assert_round_trips(&h.session);
}

/// An intact save must come out untouched: the command may not mint a record
/// for a slot that never claimed one.
#[test]
fn repair_items_finds_nothing_in_an_intact_save() {
    let mut h = Harness::new();
    let outcome = h.run("repair_items", serde_json::json!({}), false);
    assert_eq!(outcome.status, RunStatus::Ok, "{:?}", outcome.status);
    assert_eq!(
        outcome.result.expect("a result")["counts"]["repaired"].as_i64(),
        Some(0)
    );
    assert_round_trips(&h.session);
}

/// A minimal ad-hoc plugin used only to break a fixture's own correct sizing,
/// so `trim_overfilled_inventories` has something to fix. Enlarges the first
/// player's main inventory by 3 empty slots -- a change that never refuses,
/// since growing a container can never strand an occupied slot.
const MISRESIZE_MANIFEST: &str = r#"{
  "id": "test.misresize", "api_version": 1, "name": "Test", "version": "1.0.0",
  "entry": "main.lua",
  "capabilities": ["save.read", "save.write", "players"],
  "commands": [ { "id": "misresize_first_player", "title": "Misresize" } ]
}"#;

const MISRESIZE_SOURCE: &str = r#"
function misresize_first_player()
  local target_id = nil
  for player in save.players() do
    target_id = player.common_container_id
    break
  end
  local by_id = {}
  for container in save.containers() do
    by_id[container.id] = container
  end
  local target = by_id[target_id]
  if target == nil then return { resized = false } end
  return { resized = target.set_slot_count(target.slot_count + 3) }
end
"#;

fn misresize_a_players_common_container(h: &mut Harness) {
    let manifest = Manifest::parse(MISRESIZE_MANIFEST).expect("the ad-hoc manifest must parse");
    let mut sources = BTreeMap::new();
    sources.insert("main.lua".to_string(), MISRESIZE_SOURCE.to_string());
    let granted = manifest.capabilities.clone();
    let outcome = run_command(
        RunRequest {
            manifest: &manifest,
            sources: &sources,
            command_id: "misresize_first_player",
            args: &serde_json::json!({}),
            dry_run: false,
            granted: &granted,
        },
        RunServices {
            session: &mut h.session,
            game_data: &h.game_data,
            progress: None,
            storage: &BTreeMap::new(),
            confirm: None,
            limits: Limits::default(),
            cancel: Cancel::new(),
        },
    );
    assert_eq!(outcome.status, RunStatus::Ok, "misresize setup must itself succeed: {:?}", outcome.status);
}

/// A second ad-hoc plugin that manufactures a genuine refusal instead of
/// merely asserting the `refused` key exists. The `v1_relics` fixture already
/// has a player (`e1530496...`) with two `AdditionalInventory_` entries in
/// their essential container and, correspondingly, a 48-slot common container
/// whose own top three slots (indices 45-47) are occupied. Clearing one
/// `AdditionalInventory_` entry drops the expansion count to 1 -- so
/// `trim_overfilled_inventories` computes a target of 45 for a container that
/// still has real items sitting at 45, 46 and 47, and `set_slot_count(45)`
/// must refuse rather than drop them.
///
/// `slot.clear()` on the essential container never touches the paired common
/// container's own size (that auto-resize is specific to a real
/// `EssentialContainer` DTO write, which this is not), so the mismatch this
/// sets up survives untouched until `trim_overfilled_inventories` runs.
const OVERFILL_MANIFEST: &str = r#"{
  "id": "test.overfill", "api_version": 1, "name": "Test", "version": "1.0.0",
  "entry": "main.lua",
  "capabilities": ["save.read", "save.write", "players"],
  "commands": [ { "id": "shrink_a_players_expansions", "title": "Shrink" } ]
}"#;

const OVERFILL_SOURCE: &str = r#"
function shrink_a_players_expansions()
  local essential_id = nil
  for player in save.players() do
    if player.uid == "e1530496-0000-0000-0000-000000000000" then
      essential_id = player.essential_container_id
    end
  end
  if essential_id == nil then return { cleared = false } end

  local essential = nil
  for container in save.containers() do
    if container.id == essential_id then essential = container end
  end
  if essential == nil then return { cleared = false } end

  local target_index = nil
  for slot in essential.slots() do
    if slot.item_id == "AdditionalInventory_002" then
      target_index = slot.index
    end
  end
  if target_index == nil then return { cleared = false } end

  -- A fresh handle set: the read-only pass above must finish before the
  -- structural clear below, or the iteration that found `target_index` would
  -- itself be invalidated by it.
  local essential2 = nil
  for container in save.containers() do
    if container.id == essential_id then essential2 = container end
  end
  local cleared = false
  for slot in essential2.slots() do
    if slot.index == target_index then
      slot.clear()
      cleared = true
      break
    end
  end
  return { cleared = cleared }
end
"#;

fn shrink_a_players_expansions(h: &mut Harness) {
    let manifest = Manifest::parse(OVERFILL_MANIFEST).expect("the ad-hoc manifest must parse");
    let mut sources = BTreeMap::new();
    sources.insert("main.lua".to_string(), OVERFILL_SOURCE.to_string());
    let granted = manifest.capabilities.clone();
    let outcome = run_command(
        RunRequest {
            manifest: &manifest,
            sources: &sources,
            command_id: "shrink_a_players_expansions",
            args: &serde_json::json!({}),
            dry_run: false,
            granted: &granted,
        },
        RunServices {
            session: &mut h.session,
            game_data: &h.game_data,
            progress: None,
            storage: &BTreeMap::new(),
            confirm: None,
            limits: Limits::default(),
            cancel: Cancel::new(),
        },
    );
    assert_eq!(outcome.status, RunStatus::Ok, "shrink setup must itself succeed: {:?}", outcome.status);
    let result = outcome.result.expect("a result");
    assert_eq!(
        result["cleared"].as_bool(),
        Some(true),
        "the setup must clear an AdditionalInventory_ entry, or the refusal it sets up never happens"
    );
}

#[test]
fn trim_overfilled_inventories_sizes_each_common_container_to_the_expansion_formula() {
    let mut h = Harness::new();
    misresize_a_players_common_container(&mut h);

    let outcome = h.run("trim_overfilled_inventories", serde_json::json!({}), false);
    assert_eq!(outcome.status, RunStatus::Ok, "{:?}", outcome.status);
    let counts = outcome.result.expect("a result")["counts"].clone();
    assert!(counts["examined"].as_i64().expect("examined") > 0);
    assert!(
        counts["resized"].as_i64().expect("resized") > 0,
        "the fixture was deliberately misresized, so this run must find and fix it"
    );

    // Converges: a second run finds nothing to resize.
    let again = h.run("trim_overfilled_inventories", serde_json::json!({}), false);
    assert_eq!(
        again.result.expect("a result")["counts"]["resized"].as_i64(),
        Some(0),
        "the command must converge in one run"
    );

    assert_round_trips(&h.session);
}

#[test]
fn trim_overfilled_inventories_refuses_rather_than_dropping_an_occupied_slot() {
    use psp_core::domain::containers;

    let mut h = Harness::new();
    shrink_a_players_expansions(&mut h);
    let common_id: Uuid = "c9b2170c-4dbf-ae4d-4593-839c432cc265".parse().expect("valid uuid literal");

    let outcome = h.run("trim_overfilled_inventories", serde_json::json!({}), false);
    assert_eq!(outcome.status, RunStatus::Ok, "{:?}", outcome.status);
    let counts = outcome.result.expect("a result")["counts"].clone();
    assert_eq!(
        counts["refused"].as_i64(),
        Some(1),
        "the deliberately shrunk player's resize must be refused, not silently skipped or applied"
    );

    let dto = containers::read_item_container(&h.session.level, &mut h.session.caches, &h.game_data, common_id, "", None)
        .expect("the refused container must still be readable");
    assert_eq!(dto.slot_num, 48, "a refusal must leave the container's own size untouched");
    let slot_47 = dto.slots.iter().find(|s| s.slot_index == 47).expect("the item at slot 47 must survive the refusal");
    assert_eq!(slot_47.static_id.as_deref(), Some("PalSphere_Ancient_2"));

    assert_round_trips(&h.session);
}

/// A healthy save has nothing to refuse: `refused` legitimately reads zero
/// here, so `.is_number()` -- confirming the key exists rather than being
/// omitted -- is the honest assertion this fixture supports.
#[test]
fn trim_overfilled_inventories_surfaces_a_refused_count_even_when_it_is_zero() {
    let mut h = Harness::new();
    let outcome = h.run("trim_overfilled_inventories", serde_json::json!({}), false);
    assert_eq!(outcome.status, RunStatus::Ok, "{:?}", outcome.status);
    let counts = outcome.result.expect("a result")["counts"].clone();
    assert!(
        counts["refused"].is_number(),
        "a resize the host refuses must be counted and surfaced, never silently ignored"
    );
}
