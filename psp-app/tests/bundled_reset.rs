use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use psp_core::error::CoreError;
use psp_core::gamedata::GameData;
use psp_core::progress::null_progress;
use psp_core::props;
use psp_core::session::{PlayerFileData, SaveKind, SaveSession};

use psp_app::plugin_registry::BUNDLED;
use psp_plugin::manifest::{Manifest, Origin};
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
            .find(|p| p.id == "pst.reset")
            .expect("pst.reset is a bundled plugin");
        let manifest = Manifest::parse(plugin.manifest, Origin::Bundled)
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

fn empty_map_property() -> psp_core::ue::Property {
    psp_core::ue::Property::Map(Vec::new())
}

fn top_level_key_present(session: &SaveSession, key: &str) -> bool {
    session.world_properties().ok().and_then(|world| props::get(world, &[key])).is_some()
}

fn seed_top_level_key(session: &mut SaveSession, key: &str) {
    let props = psp_core::domain::world::world_props_mut(&mut session.level)
        .expect("the fixture level has a worldSaveData");
    props.insert(key, empty_map_property());
}

fn assert_round_trips(session: &SaveSession) {
    let bytes = session.level_sav_bytes().expect("the level serializes");
    reparse(&bytes).unwrap_or_else(|e| panic!("the written save did not reparse: {e}"));
}

/// The player-file counterpart of [`assert_round_trips`], for commands (like
/// `fix_missions`) that write player `.sav` files and never touch `Level.sav`
/// at all -- round-tripping the level there would prove nothing about them.
fn assert_player_files_round_trip(session: &SaveSession) {
    let player_files = session.player_sav_bytes().expect("the player files serialize");
    assert!(!player_files.is_empty(), "the fixture must have loaded players");
    for (uid, (sav_bytes, dps_bytes)) in &player_files {
        psp_core::savio::read_sav_bytes(sav_bytes)
            .unwrap_or_else(|e| panic!("player {uid}'s written .sav did not reparse: {e}"));
        if let Some(dps_bytes) = dps_bytes {
            psp_core::savio::read_sav_bytes(dps_bytes)
                .unwrap_or_else(|e| panic!("player {uid}'s written _dps.sav did not reparse: {e}"));
        }
    }
}

fn assert_single_key_reset(command_id: &str, key: &str, count_key: &str) {
    let mut h = Harness::new();
    let carried = top_level_key_present(&h.session, key);
    if !carried {
        seed_top_level_key(&mut h.session, key);
    }
    eprintln!("{command_id}: fixture carried {key} = {carried}");
    assert!(
        top_level_key_present(&h.session, key),
        "{command_id}: {key} must be readable before the run, or this test proves nothing"
    );

    let dry = h.run(command_id, serde_json::json!({}), true);
    assert_eq!(dry.status, RunStatus::Ok, "{command_id} dry: {:?}", dry.status);
    assert_eq!(dry.counts.get(count_key).copied(), Some(1), "{command_id} dry");
    assert!(
        top_level_key_present(&h.session, key),
        "{command_id}: a dry run must not remove {key}"
    );

    let real = h.run(command_id, serde_json::json!({}), false);
    assert_eq!(real.status, RunStatus::Ok, "{command_id}: {:?}", real.status);
    assert_eq!(real.counts.get(count_key).copied(), Some(1), "{command_id}");
    assert!(
        !top_level_key_present(&h.session, key),
        "{command_id}: {key} must actually be gone, not merely reported gone"
    );
    assert_round_trips(&h.session);

    let again = h.run(command_id, serde_json::json!({}), false);
    assert_eq!(again.counts.get(count_key).copied(), Some(0), "{command_id} is not idempotent");
}

#[test]
fn reset_supply_drops_removes_the_key_and_is_idempotent() {
    assert_single_key_reset("reset_supply_drops", "SupplySaveData", "supply_save_data");
}

#[test]
fn reset_anti_air_turrets_removes_the_key_and_is_idempotent() {
    assert_single_key_reset("reset_anti_air_turrets", "FixedWeaponDestroySaveData", "fixed_weapon_destroy_save_data");
}

#[test]
fn reset_oilrig_removes_the_key_and_is_idempotent() {
    assert_single_key_reset("reset_oilrig", "OilrigSaveData", "oilrig_save_data");
}

#[test]
fn reset_invader_removes_the_key_and_is_idempotent() {
    assert_single_key_reset("reset_invader", "InvaderSaveData", "invader_save_data");
}

#[test]
fn reset_lock_gimmick_removes_the_key_and_is_idempotent() {
    assert_single_key_reset("reset_lock_gimmick", "LockGimmickSaveData", "lock_gimmick_save_data");
}

/// The only Reset command that removes more than one key. Both must go, and
/// the count must be the number actually removed — a command that deleted one
/// of two and reported 2 would satisfy any single-key assertion.
#[test]
fn reset_dungeons_removes_both_keys_and_is_idempotent() {
    let mut h = Harness::new();
    for key in ["DungeonPointMarkerSaveData", "DungeonSaveData"] {
        if !top_level_key_present(&h.session, key) {
            seed_top_level_key(&mut h.session, key);
        }
        assert!(top_level_key_present(&h.session, key), "{key} must be readable before the run");
    }

    let dry = h.run("reset_dungeons", serde_json::json!({}), true);
    assert_eq!(dry.status, RunStatus::Ok, "{:?}", dry.status);
    assert_eq!(dry.counts.get("dungeon_save_data").copied(), Some(2));
    assert!(top_level_key_present(&h.session, "DungeonSaveData"), "a dry run must remove nothing");

    let real = h.run("reset_dungeons", serde_json::json!({}), false);
    assert_eq!(real.status, RunStatus::Ok, "{:?}", real.status);
    assert_eq!(real.counts.get("dungeon_save_data").copied(), Some(2));
    assert!(!top_level_key_present(&h.session, "DungeonPointMarkerSaveData"));
    assert!(!top_level_key_present(&h.session, "DungeonSaveData"));
    assert_round_trips(&h.session);

    let again = h.run("reset_dungeons", serde_json::json!({}), false);
    assert_eq!(again.counts.get("dungeon_save_data").copied(), Some(0));
}

/// Stronger than the per-command dry assertions: a dry run must leave the
/// serialized level byte-for-byte identical, which catches a write that
/// changed something other than the key under test.
#[test]
fn a_dry_reset_leaves_the_level_byte_identical() {
    for command_id in [
        "reset_supply_drops",
        "reset_anti_air_turrets",
        "reset_oilrig",
        "reset_invader",
        "reset_dungeons",
        "reset_lock_gimmick",
    ] {
        let mut h = Harness::new();
        let before = h.session.level_gvas_bytes().expect("serializes");
        let outcome = h.run(command_id, serde_json::json!({}), true);
        assert_eq!(outcome.status, RunStatus::Ok, "{command_id}: {:?}", outcome.status);
        let after = h.session.level_gvas_bytes().expect("serializes");
        assert_eq!(before, after, "{command_id}: a dry run must not change a single byte");
    }
}

fn player_has_quest_array(session: &mut SaveSession, uid: Uuid) -> bool {
    if session.ensure_player_loaded(uid).is_err() {
        return false;
    }
    session
        .loaded_players
        .get(&uid)
        .and_then(|loaded| {
            psp_core::props::get(
                &loaded.sav.root.properties,
                &["SaveData", "CompletedQuestArray_FullRelease"],
            )
        })
        .is_some()
}

/// Duplicates a fixture player's summary and file reference under fresh UIDs
/// until the session carries at least `target` players. Reuses the same
/// on-disk file reference for every clone: `ensure_player_gvas_loaded` reads
/// and parses independently per UID, so this still exercises the real
/// per-player load path the command runs against, without needing every
/// clone to be a distinct file on disk.
fn clone_players_until(session: &mut SaveSession, target: usize) {
    let template_uid = *session
        .player_summary_order
        .first()
        .expect("the fixture must have at least one player to clone");
    let template_summary = session
        .player_summaries
        .get(&template_uid)
        .cloned()
        .expect("the template player has a summary");
    let template_file_ref = session
        .player_file_refs
        .get(&template_uid)
        .cloned()
        .expect("the template player has a file reference");

    while session.player_summary_order.len() < target {
        let uid = Uuid::new_v4();
        let mut summary = template_summary.clone();
        summary.uid = uid;
        session.player_summary_order.push(uid);
        session.player_summaries.insert(uid, summary);
        session.player_file_refs.insert(uid, template_file_ref.clone());
    }
}

/// The seed has to be readable through the same path the command uses before
/// the assertion means anything: if no fixture player carries a
/// completed-quest array, a command that did nothing at all would report
/// zero and look correct. A sample of one is also not enough: with only one
/// player carrying the array, "clears every matching player" and "clears
/// only the first matching player and stops" both pass.
#[test]
fn fix_missions_clears_completed_quests_for_every_player() {
    let mut h = Harness::new();
    let uids: Vec<uuid::Uuid> = h.session.player_summary_order.clone();
    assert!(!uids.is_empty(), "the fixture must have players");

    let with_quests: Vec<Uuid> = uids
        .iter()
        .copied()
        .filter(|uid| player_has_quest_array(&mut h.session, *uid))
        .collect();

    assert!(
        with_quests.len() >= 2,
        "the fixture must have at least two players carrying a completed-quest array, so this \
         test can tell clearing every player apart from clearing only the first; got {}",
        with_quests.len()
    );

    let dry = h.run("fix_missions", serde_json::json!({}), true);
    assert_eq!(dry.status, RunStatus::Ok, "{:?}", dry.status);
    assert_eq!(dry.counts.get("players").copied(), Some(with_quests.len() as i64));

    let real = h.run("fix_missions", serde_json::json!({}), false);
    assert_eq!(real.status, RunStatus::Ok, "{:?}", real.status);
    assert_eq!(real.counts.get("players").copied(), Some(with_quests.len() as i64));
    for uid in &uids {
        assert!(
            !player_has_quest_array(&mut h.session, *uid),
            "player {uid} still carries a completed-quest array"
        );
    }
    assert_player_files_round_trip(&h.session);
}

/// Synthesises the one dimension that actually grows -- player count -- and
/// asserts the command still finishes well inside its real budget. It
/// asserts elapsed time, not a count: the count is already covered above,
/// and what super-linear growth breaks is the clock. The threshold is loose
/// on purpose: it exists to catch super-linear growth, which shows up as a
/// multiple of the budget, not a percentage of it, so machine speed and test
/// parallelism (which have nothing to do with the defect this guards
/// against) should not be able to flip it.
#[test]
fn fix_missions_stays_linear_in_player_count() {
    let mut h = Harness::new();
    let baseline = h.session.player_summary_order.len();
    assert!(baseline > 0, "the fixture must have players");

    clone_players_until(&mut h.session, 2_000);
    let scaled = h.session.player_summary_order.len();
    assert!(scaled >= 2_000, "expected at least 2000 players, got {scaled}");

    let started = std::time::Instant::now();
    let outcome = h.run("fix_missions", serde_json::json!({}), false);
    let elapsed = started.elapsed();

    assert_eq!(outcome.status, RunStatus::Ok, "{:?}", outcome.status);
    assert!(
        elapsed < std::time::Duration::from_secs(90),
        "fix_missions took {elapsed:?} for {scaled} players against a 90s bound (real budget \
         120s, ~30s headroom kept below it). This bound exists to catch super-linear growth, \
         which shows up as a multiple of the budget, not a near miss -- so this margin should \
         only ever be crossed by an actual algorithmic regression, not machine speed. \
         Investigate before raising anything."
    );
    eprintln!("fix_missions_stays_linear_in_player_count: {scaled} players in {elapsed:?}");
}
