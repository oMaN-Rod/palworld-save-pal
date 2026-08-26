use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::time::Instant;

use psp_core::domain::containers;
use psp_core::domain::guild::{base_camp_location, guild_chest_id};
use psp_core::domain::map_object;
use psp_core::domain::player;
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
            .find(|p| p.id == "pst.tools")
            .expect("pst.tools is a bundled plugin");
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

fn assert_round_trips(session: &SaveSession) {
    let bytes = session.level_sav_bytes().expect("the level serializes");
    reparse(&bytes).unwrap_or_else(|e| panic!("the written save did not reparse: {e}"));
}

const TICKS_PER_DAY: i64 = 864_000_000_000;

fn game_date_time_ticks(session: &SaveSession) -> Option<i64> {
    psp_core::props::get(
        session.world_properties().ok()?,
        &["GameTimeSaveData", "GameDateTimeTicks"],
    )
    .and_then(psp_core::props::as_i64)
}

/// The command sets an absolute day count, so the test must prove the value
/// LANDED, not merely that the command reported success — and must start from
/// a different value than it sets, or an unchanged save would also pass.
#[test]
fn edit_game_days_writes_the_requested_day_count() {
    let mut h = Harness::new();
    let before = game_date_time_ticks(&h.session).expect("the fixture has GameTimeSaveData");
    let target_days = (before / TICKS_PER_DAY) + 7;

    let dry = h.run("edit_game_days", serde_json::json!({ "days": target_days }), true);
    assert_eq!(dry.status, RunStatus::Ok, "{:?}", dry.status);
    assert_eq!(
        game_date_time_ticks(&h.session),
        Some(before),
        "a dry run must not change the clock"
    );

    let real = h.run("edit_game_days", serde_json::json!({ "days": target_days }), false);
    assert_eq!(real.status, RunStatus::Ok, "{:?}", real.status);
    assert_eq!(
        game_date_time_ticks(&h.session),
        Some(target_days * TICKS_PER_DAY),
        "the clock must hold exactly the requested day count in ticks"
    );
    assert_round_trips(&h.session);
}

/// The command's whole output is its command lines, so the test asserts on
/// them — a count alone would pass against a command that emitted the right
/// number of malformed strings.
#[test]
fn paldefender_commands_emits_one_line_per_base() {
    let mut h = Harness::new();
    let base_count = h.session.base_camp_map().map(|entries| entries.len()).unwrap_or(0);
    assert!(base_count > 0, "the fixture must have base camps");

    let outcome = h.run("paldefender_commands", serde_json::json!({}), false);
    assert_eq!(outcome.status, RunStatus::Ok, "{:?}", outcome.status);
    assert_eq!(outcome.counts.get("bases").copied(), Some(base_count as i64));

    let lines = outcome
        .result
        .as_ref()
        .and_then(|v| v.get("lines"))
        .and_then(|v| v.as_array())
        .expect("the result carries a lines array");
    assert_eq!(lines.len(), base_count);

    let mut emitted: Vec<(i64, i64, i64)> = Vec::with_capacity(lines.len());
    for line in lines {
        let line = line.as_str().expect("each line is a string");
        assert!(line.starts_with("/killnearestbase "), "got {line:?}");
        let parts: Vec<&str> = line.split_whitespace().collect();
        assert_eq!(parts.len(), 4, "expected the command plus three coordinates, got {line:?}");
        let x: i64 = parts[1].parse().unwrap_or_else(|e| panic!("x in {line:?} did not parse: {e}"));
        let y: i64 = parts[2].parse().unwrap_or_else(|e| panic!("y in {line:?} did not parse: {e}"));
        let z: i64 = parts[3].parse().unwrap_or_else(|e| panic!("z in {line:?} did not parse: {e}"));
        emitted.push((x, y, z));
    }
    emitted.sort_unstable();

    let mut expected: Vec<(i64, i64, i64)> = h
        .session
        .base_camp_map()
        .expect("the fixture must have base camps")
        .iter()
        .filter_map(base_camp_location)
        .map(|(x, y, z)| (x.round() as i64, y.round() as i64, z.round() as i64))
        .collect();
    expected.sort_unstable();

    assert_eq!(
        emitted, expected,
        "emitted coordinates must match the fixture's base camp locations, order-independently"
    );
}

/// The container id, its current slot count, and the highest occupied
/// `slot_index` of the first item container in the fixture that holds at
/// least one item -- everything `modify_one_container_slots`'s tests need to
/// exercise a growing resize and a destructive-shrink refusal against a real
/// container.
fn find_occupied_container(session: &mut SaveSession, game_data: &GameData) -> (Uuid, i32, i32) {
    let ids: Vec<Uuid> = session
        .item_container_map()
        .expect("the fixture has item containers")
        .iter()
        .filter_map(|entry| {
            psp_core::props::get(psp_core::props::struct_props(&entry.key)?, &["ID"])
                .and_then(psp_core::props::as_uuid)
        })
        .collect();
    for id in ids {
        let Some(dto) =
            containers::read_item_container(&session.level, &mut session.caches, game_data, id, "", None)
        else {
            continue;
        };
        let highest_occupied = dto
            .slots
            .iter()
            .filter(|slot| matches!(slot.static_id.as_deref(), Some(v) if !v.is_empty() && v != "None"))
            .map(|slot| slot.slot_index)
            .max();
        if let Some(highest_occupied) = highest_occupied {
            return (id, dto.slot_num, highest_occupied);
        }
    }
    panic!("the fixture must have at least one container holding an item");
}

fn container_slot_num(session: &mut SaveSession, game_data: &GameData, id: Uuid) -> i32 {
    containers::read_item_container(&session.level, &mut session.caches, game_data, id, "", None)
        .expect("the container still exists")
        .slot_num
}

/// Covers all three outcomes `modify_one_container_slots` can report --
/// resized, refused, and an unknown container id -- asserting the
/// container's actual capacity afterward rather than only the reported
/// counts, since a command that reports "resized" without the resize
/// landing (or "refused" while quietly shrinking anyway) would otherwise
/// pass.
#[test]
fn modify_one_container_slots_covers_resize_refusal_and_unknown_id() {
    let mut h = Harness::new();
    let (container_id, original_slots, highest_occupied) =
        find_occupied_container(&mut h.session, &h.game_data);

    let grow_target = original_slots + 5;
    let grown = h.run(
        "modify_one_container_slots",
        serde_json::json!({ "container_id": container_id.to_string(), "slots": grow_target }),
        false,
    );
    assert_eq!(grown.status, RunStatus::Ok, "{:?}", grown.status);
    let grown_counts = grown
        .result
        .as_ref()
        .and_then(|v| v.get("counts"))
        .expect("the command's own result carries a counts table");
    assert_eq!(grown_counts.get("containers").and_then(|v| v.as_i64()), Some(1));
    assert_eq!(grown_counts.get("refused").and_then(|v| v.as_i64()), Some(0));
    assert_eq!(
        container_slot_num(&mut h.session, &h.game_data, container_id),
        grow_target,
        "the container must actually hold the requested slot count after a resize"
    );
    assert_round_trips(&h.session);

    let refused = h.run(
        "modify_one_container_slots",
        serde_json::json!({ "container_id": container_id.to_string(), "slots": highest_occupied }),
        false,
    );
    assert_eq!(refused.status, RunStatus::Ok, "{:?}", refused.status);
    let refused_counts = refused
        .result
        .as_ref()
        .and_then(|v| v.get("counts"))
        .expect("the command's own result carries a counts table");
    assert_eq!(refused_counts.get("containers").and_then(|v| v.as_i64()), Some(0));
    assert_eq!(refused_counts.get("refused").and_then(|v| v.as_i64()), Some(1));
    assert_eq!(
        container_slot_num(&mut h.session, &h.game_data, container_id),
        grow_target,
        "a refused shrink must leave the container's capacity exactly as the resize left it"
    );
    assert_round_trips(&h.session);

    let unknown_id = Uuid::new_v4();
    let missing = h.run(
        "modify_one_container_slots",
        serde_json::json!({ "container_id": unknown_id.to_string(), "slots": 10 }),
        false,
    );
    assert_eq!(missing.status, RunStatus::Ok, "{:?}", missing.status);
    let missing_counts = missing
        .result
        .as_ref()
        .and_then(|v| v.get("counts"))
        .expect("the command's own result carries a counts table");
    assert_eq!(missing_counts.get("containers").and_then(|v| v.as_i64()), Some(0));
    assert_eq!(missing_counts.get("refused").and_then(|v| v.as_i64()), Some(0));
    let missing_summary = missing.result.as_ref().and_then(|v| v.get("summary")).and_then(|v| v.as_str());
    assert_eq!(missing_summary, Some(format!("No container with id {unknown_id}").as_str()));
}

/// Every container's current `slot_num`, keyed by id -- the baseline a bulk
/// resize test diffs against to prove it hit exactly its own target set and
/// left every other container alone.
fn all_container_slot_counts(h: &mut Harness) -> BTreeMap<Uuid, i32> {
    let ids: Vec<Uuid> = h
        .session
        .item_container_map()
        .expect("the fixture has item containers")
        .iter()
        .filter_map(|entry| {
            psp_core::props::get(psp_core::props::struct_props(&entry.key)?, &["ID"])
                .and_then(psp_core::props::as_uuid)
        })
        .collect();
    ids.into_iter()
        .filter_map(|id| {
            containers::read_item_container(&h.session.level, &mut h.session.caches, &h.game_data, id, "", None)
                .map(|dto| (id, dto.slot_num))
        })
        .collect()
}

/// Every player's main inventory (`common_container`) id -- the target set
/// `modify_all_player_slots` must hit.
fn player_common_container_ids(session: &mut SaveSession, game_data: &GameData) -> BTreeSet<Uuid> {
    let uids: Vec<Uuid> = session.player_summaries.keys().copied().collect();
    uids.into_iter()
        .filter_map(|uid| {
            player::get_player_details(session, game_data, uid, &null_progress())
                .ok()
                .flatten()
                .and_then(|dto| dto.common_container)
                .map(|container| container.id)
        })
        .collect()
}

/// `(container_id, current slot_num, highest occupied slot_index)` for every
/// player common container that holds at least one item -- what
/// `modify_all_player_slots_continues_past_a_refused_container` needs to pick
/// a slot count that refuses exactly one container while the rest succeed.
fn player_common_container_occupancy(session: &mut SaveSession, game_data: &GameData) -> Vec<(Uuid, i32, i32)> {
    let uids: Vec<Uuid> = session.player_summaries.keys().copied().collect();
    uids.into_iter()
        .filter_map(|uid| {
            player::get_player_details(session, game_data, uid, &null_progress())
                .ok()
                .flatten()
                .and_then(|dto| dto.common_container)
        })
        .filter_map(|container| {
            let highest_occupied = container
                .slots
                .iter()
                .filter(|slot| matches!(slot.static_id.as_deref(), Some(v) if !v.is_empty() && v != "None"))
                .map(|slot| slot.slot_index)
                .max()?;
            Some((container.id, container.slot_num, highest_occupied))
        })
        .collect()
}

/// Every guild's chest container id -- the target set
/// `modify_all_guild_chest_slots` must hit.
fn guild_chest_container_ids(session: &SaveSession) -> BTreeSet<Uuid> {
    session
        .guild_summaries
        .keys()
        .copied()
        .filter_map(|id| guild_chest_id(session, id))
        .collect()
}

/// A bulk resize must hit exactly its own set. Asserting only that the target
/// containers changed would pass for a command that resized every container in
/// the world -- which is why the untouched set is asserted too. `60` is
/// deliberately not the manifest default (`42`), so a command that ignored its
/// argument and resized to the default would also fail.
#[test]
fn modify_all_player_slots_resizes_only_player_common_containers() {
    let mut h = Harness::new();
    let before: BTreeMap<Uuid, i32> = all_container_slot_counts(&mut h);
    let targets: BTreeSet<Uuid> = player_common_container_ids(&mut h.session, &h.game_data);
    assert!(
        !targets.is_empty(),
        "the fixture must have players with a main inventory for this test to mean anything"
    );

    let dry = h.run("modify_all_player_slots", serde_json::json!({ "slots": 60 }), true);
    assert_eq!(dry.status, RunStatus::Ok, "{:?}", dry.status);
    assert_eq!(all_container_slot_counts(&mut h), before, "a dry run must resize nothing");

    let start = Instant::now();
    let real = h.run("modify_all_player_slots", serde_json::json!({ "slots": 60 }), false);
    eprintln!("modify_all_player_slots on the fixture took {:?}", start.elapsed());
    assert_eq!(real.status, RunStatus::Ok, "{:?}", real.status);

    let after = all_container_slot_counts(&mut h);
    for (id, slot_num) in &after {
        if targets.contains(id) {
            assert_eq!(*slot_num, 60, "player common container {id} was not resized");
        } else {
            assert_eq!(
                Some(slot_num),
                before.get(id),
                "container {id} is not a player common container and must not have been touched"
            );
        }
    }
    assert_round_trips(&h.session);
}

/// The command's loop restarts `save.containers()` after a successful resize
/// (which invalidates the iterator) but must NOT restart after a refusal
/// (which does not) -- otherwise it would either abort mid-walk or spin
/// needlessly. A run with exactly one refusal among several successes is the
/// only way to prove the walk actually survives a refusal and keeps going,
/// rather than merely reporting the right counts by accident.
///
/// The slot count is chosen dynamically, just above the second-highest
/// occupied slot index among the fixture's player common containers and at or
/// under the highest one -- so the top container refuses (it still holds an
/// item past that index) while every other target container, none of which
/// holds anything that far out, succeeds.
#[test]
fn modify_all_player_slots_continues_past_a_refused_container() {
    let mut h = Harness::new();
    let mut occupancy = player_common_container_occupancy(&mut h.session, &h.game_data);
    assert!(
        occupancy.len() >= 2,
        "the fixture must have at least two players with an occupied common container \
         for this test to force one refusal alongside at least one success"
    );
    occupancy.sort_by(|a, b| b.2.cmp(&a.2));
    let (refused_id, refused_slot_num, top_occupied) = occupancy[0];
    let second_occupied = occupancy[1].2;
    assert!(
        top_occupied > second_occupied,
        "need a clear gap between the two highest occupied indices to force exactly one refusal"
    );
    let slots = (second_occupied + 2).max(42);
    assert!(
        slots <= top_occupied,
        "the chosen slot count must still sit at or under the top container's highest \
         occupied index, or nothing would be refused"
    );

    let targets: BTreeSet<Uuid> = player_common_container_ids(&mut h.session, &h.game_data);

    let real = h.run("modify_all_player_slots", serde_json::json!({ "slots": slots }), false);
    assert_eq!(real.status, RunStatus::Ok, "{:?}", real.status);

    let result_counts = real
        .result
        .as_ref()
        .and_then(|v| v.get("counts"))
        .expect("the command's own result carries a counts table");
    let resized_count = result_counts.get("containers").and_then(|v| v.as_i64()).unwrap_or(-1);
    let refused_count = result_counts.get("refused").and_then(|v| v.as_i64()).unwrap_or(-1);
    assert_eq!(refused_count, 1, "exactly the top container must be refused");
    assert_eq!(
        resized_count,
        (targets.len() - 1) as i64,
        "every other target container must have been resized despite the earlier refusal"
    );
    assert!(resized_count > 0 && refused_count > 0, "both counters must be non-zero in the same run");

    let after = all_container_slot_counts(&mut h);
    assert_eq!(
        after.get(&refused_id).copied(),
        Some(refused_slot_num),
        "the refused container's capacity must be exactly unchanged"
    );
    for id in &targets {
        if *id != refused_id {
            assert_eq!(
                after.get(id).copied(),
                Some(slots),
                "container {id} should have been resized despite another container's earlier refusal"
            );
        }
    }
    assert_round_trips(&h.session);
}

/// The lock count is read back through `psp_core::domain::map_object`
/// directly, not from the command's own report -- a count that only agrees
/// with itself would pass even if the command wrote nothing.
#[test]
fn unlock_all_private_chests_clears_every_lock_and_predicts_the_dry_run() {
    let mut h = Harness::new();
    let locked_before = map_object::count_private_chest_locks(&h.session).expect("counts");
    assert!(locked_before > 0, "the fixture must have at least one locked chest");

    let dry = h.run("unlock_all_private_chests", serde_json::json!({}), true);
    assert_eq!(dry.status, RunStatus::Ok, "{:?}", dry.status);
    assert_eq!(dry.counts.get("locks").copied(), Some(locked_before as i64));
    assert_eq!(
        map_object::count_private_chest_locks(&h.session).expect("counts"),
        locked_before,
        "a dry run must not change any lock"
    );

    let real = h.run("unlock_all_private_chests", serde_json::json!({}), false);
    assert_eq!(real.status, RunStatus::Ok, "{:?}", real.status);
    assert_eq!(real.counts.get("locks").copied(), Some(locked_before as i64));
    assert_eq!(
        map_object::count_private_chest_locks(&h.session).expect("counts"),
        0,
        "every lock must be cleared"
    );

    assert_round_trips(&h.session);
}

/// A bulk resize must hit exactly its own set. Asserting only that the target
/// containers changed would pass for a command that resized every container in
/// the world -- which is why the untouched set is asserted too. `77` is
/// deliberately not the manifest default (`50`), so a command that ignored its
/// argument and resized to the default would also fail.
#[test]
fn modify_all_guild_chest_slots_resizes_only_guild_chest_containers() {
    let mut h = Harness::new();
    let before: BTreeMap<Uuid, i32> = all_container_slot_counts(&mut h);
    let targets: BTreeSet<Uuid> = guild_chest_container_ids(&h.session);
    assert!(
        !targets.is_empty(),
        "the fixture must have guilds with a chest container for this test to mean anything"
    );

    let dry = h.run("modify_all_guild_chest_slots", serde_json::json!({ "slots": 77 }), true);
    assert_eq!(dry.status, RunStatus::Ok, "{:?}", dry.status);
    assert_eq!(all_container_slot_counts(&mut h), before, "a dry run must resize nothing");

    let real = h.run("modify_all_guild_chest_slots", serde_json::json!({ "slots": 77 }), false);
    assert_eq!(real.status, RunStatus::Ok, "{:?}", real.status);

    let after = all_container_slot_counts(&mut h);
    for (id, slot_num) in &after {
        if targets.contains(id) {
            assert_eq!(*slot_num, 77, "guild chest container {id} was not resized");
        } else {
            assert_eq!(
                Some(slot_num),
                before.get(id),
                "container {id} is not a guild chest container and must not have been touched"
            );
        }
    }
    assert_round_trips(&h.session);
}

#[test]
fn max_all_pals_raises_every_world_pal_to_the_legal_maximum() {
    let mut h = Harness::new();
    let outcome = h.run("max_all_pals", serde_json::json!({ "cheat_mode": false }), false);
    assert_eq!(outcome.status, RunStatus::Ok, "{:?}", outcome.status);
    let result = outcome.result.expect("a result");
    let counts = result["counts"].clone();
    let maxed = counts["pals"].as_i64().expect("pals");
    assert!(maxed > 0, "the fixture must contain pals to max");
    assert!(counts["dps_pals"].is_number(), "dps_pals must be reported");

    let below: Vec<String> = psp_core::domain::pal::pal_summaries(&h.session, &h.game_data)
        .expect("pal summaries")
        .iter()
        .filter(|p| p.level < 80)
        .map(|p| p.instance_id.to_string())
        .collect();
    assert!(below.is_empty(), "these pals were left below level 80: {below:?}");

    assert_round_trips(&h.session);
}

#[test]
fn max_all_pals_under_a_dry_run_writes_nothing() {
    let mut h = Harness::new();
    let before = psp_core::domain::pal::pal_summaries(&h.session, &h.game_data)
        .expect("pal summaries")
        .iter()
        .map(|p| (p.instance_id, p.level))
        .collect::<std::collections::BTreeMap<_, _>>();

    let dry = h.run("max_all_pals", serde_json::json!({ "cheat_mode": false }), true);
    assert_eq!(dry.status, RunStatus::Ok, "{:?}", dry.status);

    let after = psp_core::domain::pal::pal_summaries(&h.session, &h.game_data)
        .expect("pal summaries")
        .iter()
        .map(|p| (p.instance_id, p.level))
        .collect::<std::collections::BTreeMap<_, _>>();
    assert_eq!(before, after, "a dry run must not move a single level");
}
