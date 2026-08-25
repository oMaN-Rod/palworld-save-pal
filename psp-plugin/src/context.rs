use std::collections::{BTreeMap, HashMap};

use psp_core::domain::raw_path::RawWalk;
use psp_core::dto::summary::PalSummary;
use psp_core::gamedata::GameData;
use psp_core::progress::ProgressSink;
use psp_core::session::SaveSession;

use crate::host::dto_cache::DtoCache;
use crate::manifest::{Capability, ParamValue};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LogLevel {
    Info,
    Warn,
    Error,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LogLine {
    pub level: LogLevel,
    pub message: String,
}

#[derive(Debug, Clone, Copy)]
pub struct PalIndexEntry {
    pub position: usize,
    pub is_boss: bool,
    pub is_lucky: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeleteWhereKind {
    Player,
    Guild,
    Pal,
}

/// Parked here, not in `delete_where`'s own frame: its predicate `lua_pcall`
/// can `longjmp` past that frame, skipping the destructors of any `Vec` it owned.
pub struct DeleteWhereState {
    pub kind: DeleteWhereKind,
    pub ids: Vec<uuid::Uuid>,
    pub kill: Vec<uuid::Uuid>,
}

/// Parked in `RunContext` for the same reason as [`DeleteWhereState`].
pub struct ClearSlotsState {
    pub containers: Vec<uuid::Uuid>,
    pub kill: Vec<(uuid::Uuid, i32)>,
}

pub struct RunContext<'a> {
    pub session: &'a mut SaveSession,
    pub game_data: &'a GameData,
    pub granted: Vec<Capability>,
    pub dry_run: bool,
    /// Private: `note_mutation` must be the only path that can change this, or a
    /// site that increments it directly skips the `pals`/`container`/
    /// `pal_entry_index` clears that have to happen in lock step with it. Read
    /// it via `mutation_epoch()`; it can only ever start at `0`, via `new`.
    mutation_epoch: u64,
    pub log: Vec<LogLine>,
    pub counts: BTreeMap<String, i64>,
    pub storage: BTreeMap<String, String>,
    pub storage_writes: Vec<(String, String)>,
    pub progress: Option<&'a ProgressSink>,
    pub confirm: Option<&'a dyn Fn(&str) -> bool>,
    /// One `Option` rather than two so the snapshot and its index cannot diverge.
    pub pals: Option<(Vec<PalSummary>, HashMap<uuid::Uuid, PalIndexEntry>)>,
    /// Parked here so it survives a `longjmp` past a host frame; `Some` only
    /// while a `raw.visit` runs, which is how a nested visit is refused. The
    /// runtime must clear it once the command's own `lua_pcall` returns.
    pub raw_walk: Option<RawWalk>,
    pub delete_where: Option<DeleteWhereState>,
    pub clear_slots: Option<ClearSlotsState>,
    /// Without this memo every `slot.*` read rebuilds the whole container DTO,
    /// making one walk quadratic. One entry suffices: the walk is sequential.
    pub container: Option<(uuid::Uuid, psp_core::dto::container::ItemContainerDto)>,
    /// Write-behind cache for `PalDto`: a setter marks an entry dirty here
    /// instead of round-tripping the whole DTO through the save on every call.
    pub dto_cache: DtoCache,
    /// `CharacterSaveParameterMap` entry id -> position, built once per run on
    /// first use. A structural write can reorder or remove entries, so this
    /// must be cleared alongside `pals`/`container` in `note_mutation`.
    pub pal_entry_index: Option<BTreeMap<uuid::Uuid, usize>>,
    /// How many pals `dto_cache::flush` has actually written back this run.
    /// Deliberately not in `counts`: that map is user-facing plugin-run
    /// output, and this is host-internal observability for tests.
    pub dto_flush_count: u64,
    /// How many times `dto_cache::pal_entry_index` has actually rebuilt the
    /// index this run. Deliberately not in `counts` for the same reason as
    /// `dto_flush_count`: host-internal observability, not plugin-run output.
    pub dto_index_build_count: u64,
    pub api_version: u32,
    pub plugin_id: String,
    pub command_id: String,
    pub now: i64,
    pub args: Vec<(String, ParamValue)>,
}

impl<'a> RunContext<'a> {
    /// The only public way to build a `RunContext`: `mutation_epoch` and the
    /// other run-scoped caches always start empty, so they aren't parameters.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        session: &'a mut SaveSession,
        game_data: &'a GameData,
        granted: Vec<Capability>,
        dry_run: bool,
        log: Vec<LogLine>,
        counts: BTreeMap<String, i64>,
        storage: BTreeMap<String, String>,
        storage_writes: Vec<(String, String)>,
        progress: Option<&'a ProgressSink>,
        confirm: Option<&'a dyn Fn(&str) -> bool>,
        api_version: u32,
        plugin_id: String,
        command_id: String,
        now: i64,
        args: Vec<(String, ParamValue)>,
    ) -> Self {
        RunContext {
            session,
            game_data,
            granted,
            dry_run,
            mutation_epoch: 0,
            log,
            counts,
            storage,
            storage_writes,
            progress,
            confirm,
            pals: None,
            raw_walk: None,
            delete_where: None,
            clear_slots: None,
            container: None,
            dto_cache: DtoCache::default(),
            pal_entry_index: None,
            dto_flush_count: 0,
            dto_index_build_count: 0,
            api_version,
            plugin_id,
            command_id,
            now,
            args,
        }
    }

    pub fn grants(&self, capability: Capability) -> bool {
        self.granted.contains(&capability)
    }

    /// The epoch a handle was stamped with is compared against this to detect
    /// a structural change since the handle was taken.
    pub(crate) fn mutation_epoch(&self) -> u64 {
        self.mutation_epoch
    }

    /// Structural change: positions moved, so handles and iterators must fail.
    pub fn note_mutation(&mut self) {
        self.mutation_epoch = self.mutation_epoch.wrapping_add(1);
        self.pals = None;
        self.container = None;
        self.pal_entry_index = None;
    }

    /// Non-structural: a value overwritten in place, so handles stay valid.
    pub fn note_write(&mut self) {
        self.container = None;
    }

    /// A write to a value the `pals` snapshot itself caches: drops the snapshot
    /// without invalidating handles, so the next read rebuilds and sees the write.
    pub fn note_pal_field_write(&mut self) {
        self.pals = None;
        self.container = None;
    }

    pub fn bump(&mut self, key: &str, by: i64) {
        let entry = self.counts.entry(key.to_string()).or_insert(0);
        *entry += by;
    }
}
