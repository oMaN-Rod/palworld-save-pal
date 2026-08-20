use std::collections::{BTreeMap, HashMap};

use psp_core::domain::raw_path::RawWalk;
use psp_core::dto::summary::PalSummary;
use psp_core::gamedata::GameData;
use psp_core::progress::ProgressSink;
use psp_core::session::SaveSession;

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
    pub mutation_epoch: u64,
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
    pub api_version: u32,
    pub plugin_id: String,
    pub command_id: String,
    pub now: i64,
    pub args: Vec<(String, ParamValue)>,
}

impl RunContext<'_> {
    pub fn grants(&self, capability: Capability) -> bool {
        self.granted.contains(&capability)
    }

    /// Structural change: positions moved, so handles and iterators must fail.
    pub fn note_mutation(&mut self) {
        self.mutation_epoch = self.mutation_epoch.wrapping_add(1);
        self.pals = None;
        self.container = None;
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
