use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use psp_core::domain::{containers, player};
use psp_core::dto::container::{ItemContainerDto, ItemContainerSlotDto};
use psp_core::error::CoreError;
use psp_core::gamedata::GameData;
use psp_core::progress::null_progress;
use psp_core::props;
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

/// Re-parses `level_bytes` through the exact path `SaveSession::load` uses,
/// against the same fixture directory's `LevelMeta.sav`/`Players/*.sav` --
/// the shape test 11 needs to prove a command's output save is not corrupt.
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

/// One `pst.cleanup` command run against a fresh copy of the `v1_relics`
/// fixture. Each test builds its own `Harness` -- state never carries over
/// between tests, matching the pattern `psp-plugin`'s own test harness uses.
struct Harness {
    session: SaveSession,
    game_data: GameData,
    manifest: Manifest,
    sources: BTreeMap<String, String>,
}

impl Harness {
    fn new() -> Self {
        let plugin = &BUNDLED[0];
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

const ALL_COMMANDS: &[&str] = &[
    "delete_empty_guilds",
    "delete_inactive_players",
    "fix_all_negative_timestamps",
    "remove_invalid_items_from_save",
];

// --- guild membership helpers (test 2 and the dry-run gate both need these) ---

/// A guild's members that are BOTH non-admin AND resolvable through the write
/// API (i.e. present in `player_summaries`, meaning the fixture ships a
/// `.sav` for them). See the task report: several of this fixture's guild
/// members have no shipped `.sav` at all and can never be resolved this way,
/// which is why `manufacture_empty_guild` has to force the last bit directly.
fn resolvable_non_admin_members(session: &SaveSession, guild_id: Uuid) -> Vec<Uuid> {
    let admin = session.guild_summaries.get(&guild_id).and_then(|g| g.admin_player_uid);
    session
        .player_summaries
        .iter()
        .filter(|(uid, p)| p.guild_id == Some(guild_id) && Some(**uid) != admin)
        .map(|(uid, _)| *uid)
        .collect()
}

/// Deletes a guild's resolvable non-admin members for real, then forces the
/// guild's cached `player_count` the rest of the way to zero. Returns the
/// guild's id.
///
/// No guild in the fixture can reach `player_count == 0` through the write
/// API alone: `delete_player` refuses a guild's admin while the guild is
/// loaded, AND several guilds carry members with no shipped `.sav` file at
/// all, which `player::delete_player` can never resolve (it requires
/// `player_file_refs`). Both floors apply to every guild in this fixture --
/// see the task report. This deletes what the API genuinely can, then forces
/// the remainder, standing in for however a real save could reach a
/// zero-member guild (a stale row surviving a crash or a manual edit)
/// without needing a matching ghost `.sav` in the test corpus.
fn manufacture_empty_guild(h: &mut Harness) -> Uuid {
    let target_guild = h
        .session
        .guild_summary_order
        .iter()
        .copied()
        .max_by_key(|id| resolvable_non_admin_members(&h.session, *id).len())
        .expect("the fixture has guilds");
    let victims = resolvable_non_admin_members(&h.session, target_guild);
    assert!(
        !victims.is_empty(),
        "the fixture must have a guild with at least one resolvable non-admin member"
    );

    let progress = null_progress();
    for victim in victims {
        // `delete_player` requires the player already in `loaded_players`,
        // same as the plugin's own `player.delete()` (`save_write.rs`) does
        // by calling `get_player_details` first.
        player::get_player_details(&mut h.session, &h.game_data, victim, &progress)
            .expect("loading a guild's non-admin member must succeed")
            .expect("non-admin member must resolve");
        let deleted = player::delete_player(&mut h.session, &h.game_data, victim, &progress)
            .expect("deleting a guild's non-admin member must succeed");
        assert!(deleted, "non-admin member {victim} must not be refused");
    }

    h.session
        .guild_summaries
        .get_mut(&target_guild)
        .expect("the target guild has a summary")
        .player_count = 0;

    target_guild
}

// --- timestamp helpers (fix_all_negative_timestamps and the temporal delete_inactive_players tests) ---

fn world_now_tick(session: &SaveSession) -> i64 {
    let world_props = psp_core::domain::world::world_props(&session.level)
        .expect("worldSaveData must be present");
    props::get(world_props, &["GameTimeSaveData", "RealDateTimeTicks"])
        .and_then(props::as_i64)
        .expect("GameTimeSaveData.RealDateTimeTicks must be a scalar i64")
}

/// Writes `LastOnlineRealTime` directly onto a player's `CharacterSaveParameterMap`
/// entry -- the same field `psp-core`'s own `transfer::sync_timestamps` writes.
/// `raw.set` cannot do this itself: it only writes an ALREADY-PRESENT scalar,
/// and this key is absent from every entry in the unmodified fixture (see the
/// task report), so seeding it is test setup, done through the domain layer
/// directly rather than through the plugin API under test.
///
/// A brand-new property also needs a schema entry before `level_sav_bytes()`
/// can serialize it -- `uesave` only knows the shape of properties it either
/// read or was told about (see `psp-core/tests/schema_priming.rs`), and
/// `LastOnlineRealTime` is absent from `psp-core`'s own curated
/// `save_parameter_schemas()` list precisely because nothing in `psp-core`
/// ever creates it fresh; `transfer::sync_timestamps` only ever updates a
/// copy that already exists. `raw.set` in the ACTUAL command never hits this:
/// it only writes where `raw.exists` already holds, so this priming step is
/// test-only, standing in for a real save where a prior gameplay event (a
/// guild transfer, for one) already introduced the property.
fn seed_last_online_real_time(session: &mut SaveSession, uid: Uuid, value: i64) {
    props::ensure_schema(
        &mut session.level,
        format!("{}.LastOnlineRealTime", psp_core::domain::pal::LEVEL_SAVE_PARAMETER_PREFIX),
        psp_core::ue::PropertyTagPartial {
            id: None,
            data: psp_core::ue::PropertyTagDataPartial::Other(psp_core::ue::PropertyType::Int64Property),
        },
    );
    let entries =
        psp_core::domain::world::character_map_mut(&mut session.level).expect("character map");
    let entry = entries
        .iter_mut()
        .find(|entry| {
            psp_core::domain::world::entry_is_player(entry)
                && psp_core::domain::world::entry_player_uid(entry) == Some(uid)
        })
        .expect("the player has a character entry");
    let save_parameter = psp_core::domain::world::entry_save_parameter_mut(entry)
        .expect("the player entry has a SaveParameter bag");
    save_parameter.insert("LastOnlineRealTime", props::int64_property(value));
}

fn read_last_online_real_time(session: &SaveSession, uid: Uuid) -> Option<i64> {
    let entries = session.character_map().ok()?;
    let entry = entries.iter().find(|entry| {
        psp_core::domain::world::entry_is_player(entry)
            && psp_core::domain::world::entry_player_uid(entry) == Some(uid)
    })?;
    let save_parameter = psp_core::domain::world::entry_save_parameter(entry)?;
    props::get(save_parameter, &["LastOnlineRealTime"]).and_then(props::as_i64)
}

fn elapsed_days_since_last_online(session: &SaveSession) -> i64 {
    let now = chrono::Utc::now();
    session
        .player_summaries
        .values()
        .filter_map(|p| p.last_online_time.map(|t| (now - t.0.and_utc()).num_days()))
        .max()
        .expect("the fixture must have players with a readable last_online timestamp (see test 6a)")
}

// --- invalid-item helpers (remove_invalid_items_from_save) ---

const BOGUS_ITEM_ID: &str = "PSP_Test_Definitely_Bogus_Item";

/// Seeds a bogus item into the first fixture player's common inventory
/// container's slot 0, returning that container's id. The fixture ships no
/// bogus item of its own, so `remove_invalid_items_from_save`'s tests must
/// seed one -- see the task report.
fn seed_bogus_item(h: &mut Harness) -> (Uuid, i32) {
    let progress = null_progress();
    let target_uid = h.session.player_summary_order[0];
    let details = player::get_player_details(&mut h.session, &h.game_data, target_uid, &progress)
        .expect("player details load")
        .expect("the first fixture player exists");
    let container_id = details
        .common_container
        .expect("the player has a common inventory container")
        .id;

    // Two separate ways a seeded stack ends up invisible to the very command
    // being tested, both of which this picks its way around:
    //
    // 1. A NEW slot index is not safe. This fixture's slot 0 already exists in
    //    the raw array carrying a dangling dynamic-item reference, and
    //    `read_item_container` drops such a slot on the way out — so seeding
    //    there wrote a stack no read could ever return.
    // 2. An EXISTING slot that owns a dynamic item is not safe either. The
    //    incoming DTO has `dynamic_item: None`, so `apply_item_container_dto`'s
    //    cleanup pass deletes that dynamic entry while the raw slot keeps
    //    pointing at it — turning the slot into case 1.
    //
    // What is left is an existing slot holding a plain stackable (nil dynamic
    // id), which survives the overwrite and stays readable through the same
    // path the command uses.
    let slot_index = containers::read_item_container(
        &h.session.level,
        &mut h.session.caches,
        &h.game_data,
        container_id,
        "",
        None,
    )
    .expect("the container resolves")
    .slots
    .iter()
    .find(|slot| {
        slot.dynamic_item.is_none()
            && matches!(slot.local_id, None | Some(psp_core::props::EMPTY_UUID))
    })
    .expect("the first fixture player's inventory holds at least one plain stackable")
    .slot_index;

    let seed = ItemContainerDto {
        id: container_id,
        r#type: String::new(),
        slots: vec![ItemContainerSlotDto {
            dynamic_item: None,
            slot_index,
            count: 1,
            static_id: Some(BOGUS_ITEM_ID.to_string()),
            local_id: None,
        }],
        key: None,
        slot_num: 0,
    };
    containers::apply_item_container_dto(&mut h.session, container_id, &seed, None)
        .expect("seeding the bogus item must succeed");

    let seeded = containers::read_item_container(
        &h.session.level,
        &mut h.session.caches,
        &h.game_data,
        container_id,
        "",
        None,
    )
    .expect("the container still resolves")
    .slots
    .iter()
    .any(|slot| slot.slot_index == slot_index && slot.static_id.as_deref() == Some(BOGUS_ITEM_ID));
    assert!(
        seeded,
        "the bogus stack must be readable after seeding, or the tests built on it prove nothing"
    );

    (container_id, slot_index)
}

// --- per-command setup shared by test 11's round trip and test 12's dry-run gate ---

fn setup_for(command_id: &str, h: &mut Harness) -> serde_json::Value {
    match command_id {
        "delete_empty_guilds" => {
            manufacture_empty_guild(h);
            serde_json::json!({})
        }
        "delete_inactive_players" => {
            let max_elapsed = elapsed_days_since_last_online(&h.session);
            serde_json::json!({ "days": 1.max(max_elapsed - 20) })
        }
        "fix_all_negative_timestamps" => {
            let world_tick = world_now_tick(&h.session);
            let uid = h.session.player_summary_order[0];
            seed_last_online_real_time(&mut h.session, uid, world_tick + 1_000_000);
            serde_json::json!({})
        }
        "remove_invalid_items_from_save" => {
            seed_bogus_item(h);
            serde_json::json!({})
        }
        other => panic!("no setup defined for {other}"),
    }
}

// --- 1 ---

#[test]
fn the_bundled_manifest_parses_and_declares_four_commands() {
    let plugin = &BUNDLED[0];
    assert_eq!(plugin.id, "pst.cleanup");
    let manifest =
        Manifest::parse(plugin.manifest).expect("the bundled manifest must parse");
    assert_eq!(manifest.commands.len(), 4);
    let mut ids: Vec<&str> = manifest.commands.iter().map(|c| c.id.as_str()).collect();
    ids.sort_unstable();
    let mut expected: Vec<&str> = ALL_COMMANDS.to_vec();
    expected.sort_unstable();
    assert_eq!(ids, expected);
}

// --- 2 ---

#[test]
fn delete_empty_guilds_removes_exactly_the_guilds_with_no_players() {
    let mut h = Harness::new();
    let target_guild = manufacture_empty_guild(&mut h);
    let untouched_before: BTreeMap<Uuid, i64> = h
        .session
        .guild_summaries
        .iter()
        .filter(|(id, _)| **id != target_guild)
        .map(|(id, g)| (*id, g.player_count))
        .collect();

    let outcome = h.run("delete_empty_guilds", serde_json::json!({}), false);
    assert_eq!(outcome.status, RunStatus::Ok, "{:?}", outcome.status);
    assert_eq!(outcome.counts.get("guilds").copied(), Some(1));
    assert_eq!(
        outcome.counts.get("unresolved").copied(),
        Some(0),
        "the manufactured guild resolves cleanly; a nonzero here would mean a real host-level \
         skip went unreported"
    );
    assert!(
        !h.session.guild_summaries.contains_key(&target_guild),
        "the manufactured empty guild must be removed"
    );

    for (id, before) in untouched_before {
        let after = h.session.guild_summaries.get(&id).map(|g| g.player_count);
        assert_eq!(after, Some(before), "guild {id} must be spared");
    }
}

// --- 3 ---

#[test]
fn delete_empty_guilds_on_a_save_with_none_reports_zero() {
    let mut h = Harness::new();
    assert!(
        h.session.guild_summaries.values().all(|g| g.player_count != 0),
        "the unmodified fixture must have no zero-player guild (see the task report)"
    );
    let guild_count_before = h.session.guild_summaries.len();

    let outcome = h.run("delete_empty_guilds", serde_json::json!({}), false);
    assert_eq!(outcome.status, RunStatus::Ok, "{:?}", outcome.status);
    assert_eq!(outcome.counts.get("guilds").copied(), Some(0));
    assert_eq!(outcome.counts.get("unresolved").copied(), Some(0));
    assert_eq!(h.session.guild_summaries.len(), guild_count_before);
}

// --- 4 ---

#[test]
fn delete_inactive_players_respects_its_days_parameter() {
    let max_elapsed = elapsed_days_since_last_online(&Harness::new().session);
    let short_days = 1.max(max_elapsed - 20);
    let long_days = max_elapsed + 20;

    let mut short_h = Harness::new();
    let short_outcome =
        short_h.run("delete_inactive_players", serde_json::json!({ "days": short_days }), false);
    assert_eq!(short_outcome.status, RunStatus::Ok, "{:?}", short_outcome.status);
    let short_removed = short_outcome.counts.get("players").copied().unwrap_or(0);

    let mut long_h = Harness::new();
    let long_outcome =
        long_h.run("delete_inactive_players", serde_json::json!({ "days": long_days }), false);
    assert_eq!(long_outcome.status, RunStatus::Ok, "{:?}", long_outcome.status);
    let long_removed = long_outcome.counts.get("players").copied().unwrap_or(0);

    assert!(
        short_removed > long_removed,
        "a shorter inactivity window must remove more players than a longer one \
         (short={short_days}d -> {short_removed}, long={long_days}d -> {long_removed})"
    );
    assert_eq!(long_removed, 0, "a threshold older than every login must remove nobody");
}

// --- 5 ---

#[test]
fn delete_inactive_players_never_removes_a_guild_admin() {
    let mut h = Harness::new();
    let loaded_admins: Vec<Uuid> = h
        .session
        .guild_summaries
        .values()
        .filter_map(|g| g.admin_player_uid)
        .filter(|uid| h.session.player_summaries.contains_key(uid))
        .collect();
    assert!(!loaded_admins.is_empty(), "the fixture must have at least one loaded guild admin");

    let aggressive_days = 1.max(elapsed_days_since_last_online(&h.session) - 20);
    let outcome =
        h.run("delete_inactive_players", serde_json::json!({ "days": aggressive_days }), false);
    assert_eq!(outcome.status, RunStatus::Ok, "{:?}", outcome.status);
    assert!(
        outcome.counts.get("admins_skipped").copied().unwrap_or(0) > 0,
        "this aggressive a threshold must have encountered at least one guild admin to skip"
    );
    assert_eq!(
        outcome.counts.get("unresolved").copied(),
        Some(0),
        "every fixture player resolves cleanly; a nonzero here would mean a real host-level \
         skip went unreported"
    );

    for admin in loaded_admins {
        assert!(
            h.session.player_summaries.contains_key(&admin),
            "guild admin {admin} must never be removed"
        );
    }
}

// --- 6 ---

#[test]
fn delete_inactive_players_with_a_huge_threshold_removes_nobody() {
    let mut h = Harness::new();
    let outcome = h.run("delete_inactive_players", serde_json::json!({ "days": 3650 }), false);
    assert_eq!(outcome.status, RunStatus::Ok, "{:?}", outcome.status);
    assert_eq!(outcome.counts.get("players").copied(), Some(0));
    assert_eq!(outcome.counts.get("unresolved").copied(), Some(0));
}

// --- 6a ---

#[test]
fn the_fixture_has_players_with_readable_last_online_timestamps() {
    let h = Harness::new();
    let with_timestamp =
        h.session.player_summaries.values().filter(|p| p.last_online_time.is_some()).count();
    assert!(
        with_timestamp > 0,
        "PlayerSummary.last_online_time is None for every fixture player; \
         tests 4 and 6 are unexercised, not merely passing vacuously -- see the task report"
    );
}

// --- 7 ---

#[test]
fn fix_all_negative_timestamps_clamps_a_seeded_future_timestamp() {
    let mut h = Harness::new();
    let world_tick = world_now_tick(&h.session);
    let target_uid = h.session.player_summary_order[0];
    seed_last_online_real_time(&mut h.session, target_uid, world_tick + 1_000_000);

    let outcome = h.run("fix_all_negative_timestamps", serde_json::json!({}), false);
    assert_eq!(outcome.status, RunStatus::Ok, "{:?}", outcome.status);
    assert_eq!(outcome.counts.get("timestamps").copied(), Some(1));

    let after = read_last_online_real_time(&h.session, target_uid)
        .expect("the seeded value must still be present");
    assert_eq!(after, world_tick);
}

// --- 8 ---

#[test]
fn fix_all_negative_timestamps_leaves_a_valid_timestamp_alone() {
    let mut h = Harness::new();
    let world_tick = world_now_tick(&h.session);
    let target_uid = h.session.player_summary_order[1];
    let valid_value = world_tick - 1_000_000;
    seed_last_online_real_time(&mut h.session, target_uid, valid_value);

    let outcome = h.run("fix_all_negative_timestamps", serde_json::json!({}), false);
    assert_eq!(outcome.status, RunStatus::Ok, "{:?}", outcome.status);
    assert_eq!(outcome.counts.get("timestamps").copied(), Some(0));

    let after = read_last_online_real_time(&h.session, target_uid)
        .expect("the seeded value must still be present");
    assert_eq!(after, valid_value, "a timestamp that is not in the future must be left alone");
}

// --- 10 ---

#[test]
fn remove_invalid_items_clears_a_seeded_bogus_item_and_keeps_valid_ones() {
    let mut h = Harness::new();
    let (container_id, slot_index) = seed_bogus_item(&mut h);

    let outcome = h.run("remove_invalid_items_from_save", serde_json::json!({}), false);
    assert_eq!(outcome.status, RunStatus::Ok, "{:?}", outcome.status);
    let cleared = outcome.counts.get("slots").copied().unwrap_or(0);
    let checked = outcome.counts.get("checked").copied().unwrap_or(0);
    assert!(
        cleared >= 1,
        "the seeded bogus stack must be cleared; summary was {:?}, counts {:?}",
        outcome.summary,
        outcome.counts
    );
    assert!(checked > cleared, "other valid item stacks must still be present to check");

    let after = containers::read_item_container(
        &h.session.level,
        &mut h.session.caches,
        &h.game_data,
        container_id,
        "",
        None,
    )
    .expect("the container still resolves after the run");
    let still_bogus = after
        .slots
        .iter()
        .find(|slot| slot.slot_index == slot_index)
        .and_then(|slot| slot.static_id.as_deref())
        == Some(BOGUS_ITEM_ID);
    assert!(!still_bogus, "the bogus item must be gone after the command runs");
}

// --- 11 ---

#[test]
fn every_command_round_trips_the_save_through_write_and_reparse() {
    for command_id in ALL_COMMANDS {
        let mut h = Harness::new();
        let args = setup_for(command_id, &mut h);

        let outcome = h.run(command_id, args, false);
        assert_eq!(outcome.status, RunStatus::Ok, "{command_id}: {:?}", outcome.status);

        let bytes = h
            .session
            .level_sav_bytes()
            .unwrap_or_else(|e| panic!("{command_id}: level_sav_bytes failed: {e}"));
        reparse(&bytes).unwrap_or_else(|e| panic!("{command_id}: the written save did not reparse: {e}"));
    }
}

// --- 12 ---

#[test]
fn every_destructive_command_has_matching_dry_run_and_real_counts() {
    for command_id in ALL_COMMANDS {
        let mut h = Harness::new();
        let args = setup_for(command_id, &mut h);

        let before = h.session.level_sav_bytes().expect("level_sav_bytes before the dry run");
        let dry = h.run(command_id, args.clone(), true);
        assert_eq!(dry.status, RunStatus::Ok, "{command_id} dry: {:?}", dry.status);
        let after_dry = h.session.level_sav_bytes().expect("level_sav_bytes after the dry run");
        assert_eq!(before, after_dry, "{command_id}: a dry run must not change level_sav_bytes()");

        let real = h.run(command_id, args, false);
        assert_eq!(real.status, RunStatus::Ok, "{command_id} real: {:?}", real.status);
        assert!(!real.counts.is_empty(), "{command_id}: expected at least one count key");

        // `RunOutcome::counts` carries extra internal preview keys under a
        // dry run (see its own doc comment) that a real run never populates,
        // so the two maps are not required to be equal wholesale -- every
        // key the SCRIPT itself reports (`summary`'s paired `counts` table,
        // which is what the manifest promises the user) must agree between
        // the two, which is what this checks.
        for (key, value) in &real.counts {
            assert_eq!(
                dry.counts.get(key),
                Some(value),
                "{command_id}: dry/real count mismatch for {key:?} (dry={:?} real={:?})",
                dry.counts,
                real.counts
            );
        }
    }
}

// --- 13 ---

/// Two bundled plugins must both seed and both resolve. A registry that
/// silently served only the first would still pass every single-plugin test
/// in this file.
#[test]
fn both_bundled_plugins_are_registered_with_distinct_ids() {
    let ids: Vec<&str> = psp_app::plugin_registry::BUNDLED.iter().map(|p| p.id).collect();
    assert!(ids.contains(&"pst.cleanup"), "got {ids:?}");
    assert!(ids.contains(&"pst.reset"), "got {ids:?}");
    assert_eq!(
        ids.len(),
        ids.iter().collect::<std::collections::BTreeSet<_>>().len(),
        "bundled plugin ids must be unique: {ids:?}"
    );
}
