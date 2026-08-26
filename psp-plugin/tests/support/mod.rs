#![allow(dead_code)]

use std::collections::BTreeMap;
use std::path::PathBuf;

use psp_core::gamedata::GameData;
use psp_core::session::{PlayerFileData, SaveKind, SaveSession};
use psp_plugin::context::{LogLine, RunContext};
use psp_plugin::host;
use psp_plugin::manifest::{Capability, Manifest};
use psp_plugin::runtime::{self, RunOutcome, RunRequest, RunServices};
use psp_plugin::sandbox::{Cancel, Limits, Sandbox};
use psp_plugin::status::RunStatus;

/// Forces the run's DTO cache to flush, mid-run, from Lua, whatever the run has
/// done up to that point.
///
/// `delete_where` flushes after its predicate pass and before its apply pass,
/// unconditionally -- it has to, since the ids it is about to delete may have
/// pending writes sitting in the cache. A predicate that selects nothing still
/// reaches that flush, and then deletes nothing, bumps no epoch and invalidates
/// no handle, so this is a flush and nothing else. `save.guilds()` is the
/// cheapest of the three iterators that carry `delete_where`.
///
/// Reading a pal field is *not* a substitute, however it may read: the read
/// path's only flush lives inside `ensure_pals_snapshot`'s `ctx.pals.is_none()`
/// branch, so it flushes only when something has already dropped the snapshot.
/// A pal write drops it; a player, guild, base or slot write does not. That
/// made the older form a no-op after exactly the writes these tests care most
/// about. `dto_cache.rs::force_flush_flushes_even_when_the_pal_snapshot_is_intact`
/// is the guard: it builds the snapshot first, writes a player, and fails if
/// this constant does not flush.
///
/// Requires `save.write` as well as `save.read`, which every caller grants.
pub const FORCE_FLUSH: &str = "save.guilds():delete_where(function() return false end)\n";

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("psp-plugin has a parent directory")
        .to_path_buf()
}

fn load_corpus() -> SaveSession {
    let dir = repo_root().join("tests/fixtures/saves/v1_relics");
    let level = std::fs::read(dir.join("Level.sav")).expect("the corpus fixture is checked in");
    let meta = std::fs::read(dir.join("LevelMeta.sav")).ok();

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
            let Ok(uid) = uuid::Uuid::parse_str(stem) else { continue };
            let slot = player_file_refs
                .entry(uid)
                .or_insert(PlayerFileData::Paths { sav: None, dps: None });
            if let PlayerFileData::Paths { sav, dps } = slot {
                if is_dps { *dps = Some(path) } else { *sav = Some(path) }
            }
        }
    }

    SaveSession::load(
        SaveKind::Steam { level_path: dir.join("Level.sav") },
        dir.to_string_lossy().into_owned(),
        "steam",
        &level,
        meta.as_deref(),
        None,
        player_file_refs,
        None,
        false,
        &psp_core::progress::null_progress(),
    )
    .expect("the corpus fixture must load; a failure here is a repo bug")
}

pub fn load_game_data() -> GameData {
    GameData::load(&repo_root().join("data/json")).expect("game data is checked in")
}

type ConfirmFn = Box<dyn Fn(&str) -> bool>;

pub struct Harness {
    session: SaveSession,
    game_data: GameData,
    granted: Vec<Capability>,
    dry_run: bool,
    limits: Limits,
    storage: BTreeMap<String, String>,
    log: Vec<LogLine>,
    counts: BTreeMap<String, i64>,
    storage_writes: Vec<(String, String)>,
    confirm: Option<ConfirmFn>,
    progress: Option<psp_core::progress::ProgressSink>,
    dto_flush_count: u64,
    dto_index_build_count: u64,
}

fn build(granted: &[Capability], dry_run: bool, limits: Limits) -> Harness {
    Harness {
        session: load_corpus(),
        game_data: load_game_data(),
        granted: granted.to_vec(),
        dry_run,
        limits,
        storage: BTreeMap::new(),
        log: Vec::new(),
        counts: BTreeMap::new(),
        storage_writes: Vec::new(),
        confirm: None,
        progress: None,
        dto_flush_count: 0,
        dto_index_build_count: 0,
    }
}

pub fn harness(granted: &[Capability]) -> Harness {
    build(granted, false, Limits::default())
}

pub fn harness_dry(granted: &[Capability]) -> Harness {
    build(granted, true, Limits::default())
}

pub fn harness_with_timeout(granted: &[Capability], wall_clock_ms: i64) -> Harness {
    build(granted, false, Limits { wall_clock_ms, ..Limits::default() })
}

pub fn harness_with_memory(granted: &[Capability], memory_bytes: usize) -> Harness {
    build(granted, false, Limits { memory_bytes, ..Limits::default() })
}

impl Harness {
    pub fn with_confirm(mut self, confirm: impl Fn(&str) -> bool + 'static) -> Self {
        self.confirm = Some(Box::new(confirm));
        self
    }

    /// Simulates a missing or malformed `pals.json`: `GameData::load` tolerates
    /// both by leaving its catalog empty rather than erroring, so this is
    /// `GameData::from_entries` over nothing, not a corrupted-file fixture.
    pub fn with_empty_game_data(mut self) -> Self {
        self.game_data = GameData::from_entries(std::iter::empty()).expect("an empty entry set always parses");
        self
    }

    /// Replaces the loaded catalogs with exactly the entries given, so a test
    /// can pin how a lookup treats a spelling the real `data/json` tree does
    /// not happen to contain.
    pub fn with_game_data_entries(mut self, entries: &[(&str, &str)]) -> Self {
        let owned = entries.iter().map(|(key, json)| (key.to_string(), json.to_string()));
        self.game_data = GameData::from_entries(owned).expect("the test entries must parse");
        self
    }

    pub fn with_progress(mut self, sink: psp_core::progress::ProgressSink) -> Self {
        self.progress = Some(sink);
        self
    }

    pub fn run(&mut self, source: &str) -> (RunStatus, Option<String>) {
        let mut sandbox = Sandbox::new(self.limits, Cancel::new()).expect("a sandbox must open");
        let mut ctx = RunContext::new(
            &mut self.session,
            &self.game_data,
            self.granted.clone(),
            self.dry_run,
            std::mem::take(&mut self.log),
            std::mem::take(&mut self.counts),
            std::mem::take(&mut self.storage),
            std::mem::take(&mut self.storage_writes),
            self.progress.as_ref(),
            self.confirm.as_deref(),
            psp_plugin::manifest::SUPPORTED_API_VERSION,
            "test.harness".to_string(),
            "harness".to_string(),
            0,
            Vec::new(),
        );

        let status = unsafe {
            host::set_context(sandbox.as_ptr(), (&mut ctx) as *mut RunContext<'_> as *mut _);
            let status = match host::install_globals(sandbox.as_ptr()) {
                Ok(()) => sandbox.eval("=harness", source),
                Err(err) => RunStatus::Error(err.into_message()),
            };
            let flush = host::flush_dto_cache(&mut ctx);
            let status = host::fold_flush_error(&mut ctx, status, flush);
            host::clear_context(sandbox.as_ptr());
            status
        };

        self.log = std::mem::take(&mut ctx.log);
        self.counts = std::mem::take(&mut ctx.counts);
        self.storage = std::mem::take(&mut ctx.storage);
        self.storage_writes = std::mem::take(&mut ctx.storage_writes);
        self.dto_flush_count = ctx.dto_flush_count;
        self.dto_index_build_count = ctx.dto_index_build_count;
        drop(ctx);

        (status, sandbox.take_return_string())
    }

    pub fn a_player_uid(&self) -> uuid::Uuid {
        *self
            .session
            .player_summary_order
            .first()
            .expect("the corpus fixture has players")
    }

    pub fn session(&self) -> &SaveSession { &self.session }
    pub fn session_mut(&mut self) -> &mut SaveSession { &mut self.session }
    pub fn counts(&self) -> &BTreeMap<String, i64> { &self.counts }
    pub fn dto_flush_count(&self) -> u64 { self.dto_flush_count }
    pub fn dto_index_build_count(&self) -> u64 { self.dto_index_build_count }
    pub fn log(&self) -> &[LogLine] { &self.log }
    pub fn storage_writes(&self) -> &[(String, String)] { &self.storage_writes }
    pub fn seed_storage(&mut self, key: &str, value: &str) {
        self.storage.insert(key.to_string(), value.to_string());
    }
}

fn run_with_limits(
    manifest_json: &str,
    source: &str,
    command_id: &str,
    args: serde_json::Value,
    dry_run: bool,
    limits: Limits,
) -> RunOutcome {
    let manifest = Manifest::parse(manifest_json).expect("the fixture manifest must parse");
    let mut session = load_corpus();
    let game_data = load_game_data();
    let mut sources = BTreeMap::new();
    sources.insert(manifest.entry.clone(), source.to_string());
    let storage = BTreeMap::new();
    let granted = manifest.capabilities.clone();

    let request = RunRequest {
        manifest: &manifest,
        sources: &sources,
        command_id,
        args: &args,
        dry_run,
        granted: &granted,
    };
    let services = RunServices {
        session: &mut session,
        game_data: &game_data,
        progress: None,
        storage: &storage,
        confirm: None,
        limits,
        cancel: Cancel::new(),
    };

    runtime::run_command(request, services)
}

pub fn run_multi(
    manifest_json: &str,
    sources: BTreeMap<String, String>,
    command_id: &str,
    args: serde_json::Value,
    dry_run: bool,
) -> RunOutcome {
    let manifest =
        Manifest::parse(manifest_json).expect("the fixture manifest must parse");
    let mut session = load_corpus();
    let game_data = load_game_data();
    let storage = BTreeMap::new();
    let granted = manifest.capabilities.clone();

    let request = RunRequest {
        manifest: &manifest,
        sources: &sources,
        command_id,
        args: &args,
        dry_run,
        granted: &granted,
    };
    let services = RunServices {
        session: &mut session,
        game_data: &game_data,
        progress: None,
        storage: &storage,
        confirm: None,
        limits: Limits::default(),
        cancel: Cancel::new(),
    };

    runtime::run_command(request, services)
}

pub fn run(
    manifest_json: &str,
    source: &str,
    command_id: &str,
    args: serde_json::Value,
    dry_run: bool,
) -> RunOutcome {
    run_with_limits(manifest_json, source, command_id, args, dry_run, Limits::default())
}

pub fn run_with_timeout(
    manifest_json: &str,
    source: &str,
    command_id: &str,
    args: serde_json::Value,
    wall_clock_ms: i64,
) -> RunOutcome {
    run_with_limits(
        manifest_json,
        source,
        command_id,
        args,
        false,
        Limits { wall_clock_ms, ..Limits::default() },
    )
}

pub fn run_with_memory(
    manifest_json: &str,
    source: &str,
    command_id: &str,
    args: serde_json::Value,
    memory_bytes: usize,
) -> RunOutcome {
    run_with_limits(
        manifest_json,
        source,
        command_id,
        args,
        false,
        Limits { memory_bytes, ..Limits::default() },
    )
}

/// For proving `run_command` intersects `granted` with the manifest's own capabilities rather than trusting the caller's grant alone.
pub fn run_with_granted(
    manifest_json: &str,
    source: &str,
    command_id: &str,
    args: serde_json::Value,
    granted: &[Capability],
) -> RunOutcome {
    let manifest = Manifest::parse(manifest_json).expect("the fixture manifest must parse");
    let mut session = load_corpus();
    let game_data = load_game_data();
    let mut sources = BTreeMap::new();
    sources.insert(manifest.entry.clone(), source.to_string());
    let storage = BTreeMap::new();

    let request = RunRequest {
        manifest: &manifest,
        sources: &sources,
        command_id,
        args: &args,
        dry_run: false,
        granted,
    };
    let services = RunServices {
        session: &mut session,
        game_data: &game_data,
        progress: None,
        storage: &storage,
        confirm: None,
        limits: Limits::default(),
        cancel: Cancel::new(),
    };

    runtime::run_command(request, services)
}
