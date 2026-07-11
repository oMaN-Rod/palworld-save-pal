use std::collections::{BTreeMap, HashMap, HashSet};
use std::io::Cursor;
use std::path::PathBuf;

use crate::dto::summary::{GuildSummary, PlayerSummary};
use crate::error::CoreError;
use crate::progress::ProgressSink;
use crate::props;
use uuid::Uuid;

/// Where the currently loaded save came from.
#[derive(Debug)]
pub enum SaveKind {
    Steam {
        level_path: std::path::PathBuf,
    },
    GamePass {
        container_id: String,
    },
    /// Web zip upload — nothing on disk.
    InMemory,
}

/// A player's `.sav`/`_dps.sav` payload, either as on-disk paths (Steam/local
/// loads) or as in-memory bytes (GamePass container reads, web zip uploads).
/// Mirrors Python `SaveManager._player_file_refs`, whose values are
/// `{"sav": <path-or-bytes>, "dps": <path-or-bytes-or-None>}`.
#[derive(Debug, Clone)]
pub enum PlayerFileData {
    Paths {
        sav: Option<PathBuf>,
        dps: Option<PathBuf>,
    },
    Bytes {
        sav: Option<Vec<u8>>,
        dps: Option<Vec<u8>>,
    },
}

impl PlayerFileData {
    pub fn sav_bytes(&self) -> Result<Option<Vec<u8>>, CoreError> {
        match self {
            PlayerFileData::Paths {
                sav: Some(path), ..
            } => Ok(Some(std::fs::read(path)?)),
            PlayerFileData::Bytes {
                sav: Some(bytes), ..
            } => Ok(Some(bytes.clone())),
            _ => Ok(None),
        }
    }

    pub fn dps_bytes(&self) -> Result<Option<Vec<u8>>, CoreError> {
        match self {
            PlayerFileData::Paths {
                dps: Some(path), ..
            } => Ok(Some(std::fs::read(path)?)),
            PlayerFileData::Bytes {
                dps: Some(bytes), ..
            } => Ok(Some(bytes.clone())),
            _ => Ok(None),
        }
    }
}

/// One lazily-loaded player's compressed save-out: the `.sav` bytes and, when
/// the player has a `_dps.sav` companion, its bytes too. Mirrors Python
/// `player_gvas_files`'s `{"sav": bytes, "dps": Optional[bytes]}` per uid.
pub type PlayerSaveBytes = (Vec<u8>, Option<Vec<u8>>);

/// A lazily loaded player `.sav` (and, when present, its `_dps.sav`
/// companion), cached once parsed so a later edit doesn't re-read/re-parse
/// the file. Task 7 populates this on first detail load.
pub struct LoadedPlayer {
    pub uid: Uuid,
    pub sav: uesave::Save,
    pub dps: Option<uesave::Save>,
}

/// Lazily built, invalidatable lookup caches over `SaveSession::level`'s
/// world tree — port of the six `None`-able caches
/// `IndexingMixin.invalidate_performance_caches` clears
/// (`game/mixins/indexing.py:21-33`). Every field starts `None` and is only
/// ever populated by the domain code that actually needs it (see
/// `domain::world::build_*_index`); nothing in this task's own scope reads
/// or writes them except `invalidate_performance_caches` itself and this
/// crate's tests.
///
/// Cache-invalidation strategy: `WorldCaches` is deliberately a SEPARATE
/// struct from `SaveSession`'s own Phase-1 index fields
/// (`character_index`/`item_container_index`/`character_container_index`/
/// `group_index`/`guild_extra_index`), which stay eager, non-`Option`, and
/// built exactly once in `SaveSession::load` (see below) — those are kept
/// unchanged because the brief that introduced this struct says to keep
/// every existing Phase-1 field, and changing an already-`pub`,
/// already-server-consumed field's shape is the one kind of edit this task
/// is explicitly told never to make unasked. `WorldCaches` is the
/// InvalidateOnWrite layer Tasks 9/11 (pal/player/guild CRUD) actually
/// build on: every mutating operation that inserts or removes a
/// character-map/container-map entry MUST call
/// `SaveSession::invalidate_performance_caches` before returning (see the
/// invalidation matrix in this task's brief) — a mutation that forgets to
/// call it leaves a `Some(stale_index)` behind that resolves the wrong pal
/// on the next lookup, silently. There is no compiler-enforced guarantee here
/// (Rust can't tie "this Vec of MapEntry was mutated" to "clear this
/// Option"); the mitigation is procedural + tested: every mutation call
/// site is required by this contract to call
/// `invalidate_performance_caches`, and `world_index.rs`'s
/// `stale_character_index_after_removal_would_resolve_the_wrong_entry` test
/// demonstrates concretely what breaks if a future task forgets.
/// `HashMap`-vs-`BTreeMap` audit for every field below (each is a pure
/// uuid-keyed lookup table, verified against how Python's own equivalent
/// cache is consumed — `.get(uid, default)` throughout `mixins/summaries.py`/
/// `mixins/loading.py`, never iterated to build an ordered collection):
/// `character_index`/`item_container_index`/`character_container_index`/
/// `dynamic_item_index` resolve a single uuid to a single position, on
/// demand, exactly like the unchanged Phase-1 eager indexes of the same
/// shape (`SaveSession::character_index` etc.) they mirror; `pal_owner_counts`/
/// `player_guild_map` are looked up per-player one uuid at a time when
/// building a `PlayerSummary`/`GuildSummary` (`domain::summaries`), never
/// walked start-to-end for wire output. None of the six has an iteration
/// order that could reach the wire, so all six stay `HashMap` — unlike
/// `SaveSession::loaded_players` (see its own doc comment), which the
/// project's cross-phase reconciliation specifically calls out for
/// `BTreeMap`.
#[derive(Default)]
pub struct WorldCaches {
    /// InstanceId → `CharacterSaveParameterMap` position.
    pub character_index: Option<HashMap<Uuid, usize>>,
    /// key.ID → `ItemContainerSaveData` position.
    pub item_container_index: Option<HashMap<Uuid, usize>>,
    /// key.ID → `CharacterContainerSaveData` position.
    pub character_container_index: Option<HashMap<Uuid, usize>>,
    /// `RawData.id.local_id_in_created_world` → `DynamicItemSaveData` position.
    pub dynamic_item_index: Option<HashMap<Uuid, usize>>,
    /// player uid → number of pals that player owns. `i64`, matching
    /// `domain::summaries::build_pal_owner_counts`'s own return type
    /// (Phase 1) — this field is a cache of that function's output, so its
    /// type follows the producer rather than inventing a narrower one a
    /// later task would have to cast into.
    pub pal_owner_counts: Option<HashMap<Uuid, i64>>,
    /// player uid → guild id.
    pub player_guild_map: Option<HashMap<Uuid, Uuid>>,
}

/// A loaded world save: `Level.sav` plus everything Task 8/9 need to derive
/// player/guild summaries and serve on-demand detail loads without
/// re-parsing the whole tree.
pub struct SaveSession {
    pub kind: SaveKind,
    pub world_name: String,
    pub level: uesave::Save,
    /// Python `SaveManager.level_sav_path` — the save's on-disk path (Steam)
    /// or GamePass save id, used as a stable identifier on the wire.
    pub save_id: String,
    pub save_type_label: &'static str,
    /// `level_sav_bytes.len() + 33`, matching CPython's `bytes.__sizeof__()`
    /// so `loaded_save_files.size` stays wire-identical to the Python build.
    pub size: u64,
    pub level_meta: Option<uesave::Save>,
    pub player_file_refs: BTreeMap<Uuid, PlayerFileData>,
    pub player_sav_cache: HashMap<Uuid, uesave::Save>,
    pub player_summaries: BTreeMap<Uuid, PlayerSummary>,
    pub guild_summaries: BTreeMap<Uuid, GuildSummary>,
    /// GVAS-file insertion order of `player_summaries` / `guild_summaries` —
    /// the order `domain::summaries::extract_summaries` inserted each entry
    /// in (`CharacterSaveParameterMap` order for players,
    /// `GroupSaveDataMap` order for guilds), NOT the `BTreeMap`'s sorted
    /// iteration order. Python's `sync_app_state_handler` emits
    /// `[str(p) for p in app_state.player_summaries.keys()]` /
    /// `.guild_summaries.keys()`, and those dicts preserve exactly this
    /// insertion order (see that function's own doc comment for why the
    /// two Python extraction paths this depends on are deterministic at the
    /// save sizes this port is verified against). Carried alongside the
    /// sorted maps the same way Task 9's `player_discovery_order` sits
    /// alongside `player_file_refs` — the sorted map is additional
    /// information layered on top, not a substitute for the wire order.
    pub player_summary_order: Vec<Uuid>,
    pub guild_summary_order: Vec<Uuid>,
    /// InstanceId → CharacterSaveParameterMap position.
    pub character_index: HashMap<Uuid, usize>,
    /// key.ID → ItemContainerSaveData position.
    pub item_container_index: HashMap<Uuid, usize>,
    /// key.ID → CharacterContainerSaveData position.
    pub character_container_index: HashMap<Uuid, usize>,
    /// key → GroupSaveDataMap position.
    pub group_index: HashMap<Uuid, usize>,
    /// key → GuildExtraSaveDataMap position.
    pub guild_extra_index: HashMap<Uuid, usize>,
    /// GPS (Global Pal Storage) session state -- Task 3D-1's `GpsState`
    /// (`domain::gps`). Kept as a single sub-struct, not flattened, so every
    /// GPS field (`file_path`/`save`/`slot_count`/`pals`/`loaded`) lives next
    /// to the methods that mutate it (`domain::gps`'s `impl SaveSession`
    /// block) rather than growing this struct's own top-level field list.
    pub gps: crate::domain::gps::GpsState,
    /// Lazily loaded/parsed player `.sav` files (Task 7). Keyed by player
    /// uid, same as `player_file_refs`/`player_sav_cache`.
    ///
    /// Deviation from the brief: the brief specifies
    /// `indexmap::IndexMap<Uuid, LoadedPlayer>`, but `indexmap` is not (and
    /// this task is told not to become) a direct dependency of `psp-core` —
    /// it only reaches this crate transitively, through `uesave`, and Rust
    /// does not let a crate name a transitive dependency's types without
    /// declaring that dependency itself. The project's own cross-phase
    /// reconciliation already resolved this exact substitution project-wide:
    /// Phase 2's `IndexMap` → `BTreeMap`, specifically to keep deterministic
    /// iteration order with zero new dependencies (`Uuid: Ord`, so
    /// `BTreeMap<Uuid, LoadedPlayer>` needs nothing beyond `std`). `HashMap`
    /// was used in an earlier revision of this field on "nothing iterates it
    /// yet" grounds — true today, but `HashMap` iteration order is
    /// nondeterministic per process, the exact bug class the parity harness
    /// caught twice in Phase 1 (`sync_app_state`'s `players`/`guilds`
    /// arrays). Fixing it now, before any Task 7+ code iterates this map, is
    /// free; fixing it after is a parity hunt. See `player_summary_order`
    /// above for this codebase's actual pattern when a map's WIRE order
    /// needs to be something other than sorted-by-key: a separate `Vec<Uuid>`
    /// alongside the map, not an ordered map type — if a later task needs
    /// `loaded_players` in original-discovery order rather than sorted-by-
    /// uuid order, that's the pattern to reach for, not reverting this to
    /// `HashMap`.
    pub loaded_players: BTreeMap<Uuid, LoadedPlayer>,
    /// Guild ids whose full `GuildDto` detail has been lazily loaded (Task 8).
    /// Not a map, so outside the `HashMap`/`BTreeMap` audit above by
    /// definition, but the same question applies to any collection: this is
    /// a pure membership set (`.contains(&uid)`), never iterated to produce
    /// ordered output, so `HashSet` stays as-is.
    pub loaded_guilds: HashSet<Uuid>,
    /// Invalidatable lookup caches — see `WorldCaches`'s own doc comment for
    /// the invalidation contract.
    pub caches: WorldCaches,
}

/// Parses a Palworld save (`Level.sav`, `LevelMeta.sav`, a player `.sav`,
/// ...) with loud failures on the editor path: `error_to_raw(false)` (the
/// `SaveReader` default) turns any unparseable property into a hard error
/// instead of silently degrading it to raw bytes. PlZ (zlib) and CNK (chunk)
/// compressed saves are not implemented in `uesave`'s decompressor and
/// surface here as an ordinary `CoreError::Parse` — this function does not
/// attempt to handle either format itself.
pub(crate) fn parse_palworld_save(bytes: &[u8]) -> Result<uesave::Save, CoreError> {
    uesave::SaveReader::new()
        .types(uesave::games::palworld::palworld_types())
        .read(Cursor::new(bytes))
        .map_err(|parse_error| CoreError::Parse(parse_error.to_string()))
}

/// Port of `SaveManager._load_world_name`: `SaveData.WorldName` from
/// `LevelMeta.sav`, falling back to `"Unknown"` when the property is absent
/// or its string value is empty.
pub(crate) fn world_name_from_meta_properties(properties: &uesave::Properties) -> String {
    props::get(properties, &["SaveData", "WorldName"])
        .and_then(props::as_str)
        .filter(|name| !name.is_empty())
        .unwrap_or("Unknown")
        .to_string()
}

/// Resolves a `MapEntry`'s key to a `Uuid`: `nested_field` names a struct
/// field within the key (e.g. `CharacterSaveParameterMap`'s key struct has
/// both `PlayerUId` and `InstanceId`), or `None` when the key itself is a
/// bare `Guid` (e.g. `GroupSaveDataMap`, `GuildExtraSaveDataMap`).
fn map_entry_key_uuid(entry: &uesave::MapEntry, nested_field: Option<&str>) -> Option<Uuid> {
    match nested_field {
        None => props::as_uuid(&entry.key),
        Some(field) => props::get_in(&entry.key, &[field]).and_then(props::as_uuid),
    }
}

/// Builds a `Uuid -> position` index over a map's entries, matching Python's
/// `IndexingMixin` key extractors, which silently skip entries whose key
/// can't be resolved to a `Uuid` (`except (KeyError, TypeError): return
/// None`) rather than failing the whole load.
fn build_position_index(
    entries: &[uesave::MapEntry],
    nested_field: Option<&str>,
) -> HashMap<Uuid, usize> {
    entries
        .iter()
        .enumerate()
        .filter_map(|(position, entry)| {
            map_entry_key_uuid(entry, nested_field).map(|key| (key, position))
        })
        .collect()
}

impl SaveSession {
    /// Builds a `SaveSession` with only `kind` and `level` set; every other
    /// field defaults to an empty/harmless placeholder (`world_name:
    /// String::new()`, empty maps, `size: 0`, `save_type_label: "steam"`).
    ///
    /// Named for its primary external caller — Phase 2's
    /// `tests/common/mod.rs` and every test-only `SaveSession` builder in
    /// this workspace construct a session through this one function instead
    /// of re-declaring `SaveSession`'s full (and still growing) field list
    /// at each call site. `load` below is built on it too, for the same
    /// reason: one place to update when a field is added, not N
    /// independently hand-written struct literals that silently go stale
    /// (see the whole-branch Phase-1 review this constructor was raised
    /// against: four such literals already existed before Phase 2's first
    /// commit).
    pub fn new_for_tests(kind: SaveKind, level: uesave::Save) -> Self {
        SaveSession {
            kind,
            world_name: String::new(),
            level,
            save_id: String::new(),
            save_type_label: "steam",
            size: 0,
            level_meta: None,
            player_file_refs: BTreeMap::new(),
            player_sav_cache: HashMap::new(),
            player_summaries: BTreeMap::new(),
            guild_summaries: BTreeMap::new(),
            player_summary_order: Vec::new(),
            guild_summary_order: Vec::new(),
            character_index: HashMap::new(),
            item_container_index: HashMap::new(),
            character_container_index: HashMap::new(),
            group_index: HashMap::new(),
            guild_extra_index: HashMap::new(),
            gps: crate::domain::gps::GpsState::default(),
            loaded_players: BTreeMap::new(),
            loaded_guilds: HashSet::new(),
            caches: WorldCaches::default(),
        }
    }

    /// Port of `IndexingMixin.invalidate_performance_caches`
    /// (`game/mixins/indexing.py:21-33`): resets every lazily built lookup
    /// cache to `None` so the next accessor rebuilds it from the current
    /// world tree. Every character-map/container-map mutation (pal/player/
    /// guild CRUD, Tasks 9/11) MUST call this before returning — see
    /// `WorldCaches`'s doc comment for the invalidation matrix and why this
    /// is a procedural contract rather than a compiler-enforced one.
    pub fn invalidate_performance_caches(&mut self) {
        self.caches = WorldCaches::default();
    }

    /// Parses `Level.sav` (and `LevelMeta.sav`, when present) and builds the
    /// typed indexes Task 8/9 rely on. Mirrors the combination of Python's
    /// `AppState.process_save_files` (the `"Loading Level.sav..."` progress
    /// message) and `SaveManager.load_sav_files` (everything else), which the
    /// Rust port folds into a single entry point since Task 9's WS handler is
    /// a thin wrapper around this call.
    #[allow(clippy::too_many_arguments)]
    pub fn load(
        kind: SaveKind,
        save_id: String,
        save_type_label: &'static str,
        level_sav_bytes: &[u8],
        level_meta_bytes: Option<&[u8]>,
        player_file_refs: BTreeMap<Uuid, PlayerFileData>,
        gps_file_path: Option<PathBuf>,
        progress: &ProgressSink,
    ) -> Result<Self, CoreError> {
        progress("Loading Level.sav...");
        let level = parse_palworld_save(level_sav_bytes)?;

        let (world_name, level_meta) = match level_meta_bytes {
            Some(meta_bytes) => {
                progress("Loading level meta...");
                let meta = parse_palworld_save(meta_bytes)?;
                let name = world_name_from_meta_properties(&meta.root.properties);
                (name, Some(meta))
            }
            None => {
                progress("No LevelMeta.sav found, skipped.");
                ("No LevelMeta.sav found".to_string(), None)
            }
        };

        // CPython's bytes.__sizeof__() is len(data) + 33 (verified against
        // .venv Python 3.13); Python's _get_file_size falls back to that when
        // handed a plain bytes object (no seek/tell), which is always the
        // case for level_sav here.
        let size = level_sav_bytes.len() as u64 + 33;

        let mut session = SaveSession::new_for_tests(kind, level);
        session.world_name = world_name;
        session.save_id = save_id;
        session.save_type_label = save_type_label;
        session.size = size;
        session.level_meta = level_meta;
        session.player_file_refs = player_file_refs;
        session.gps.file_path = gps_file_path;

        session.character_index =
            build_position_index(session.character_map()?, Some("InstanceId"));
        session.item_container_index =
            build_position_index(session.item_container_map()?, Some("ID"));
        session.character_container_index =
            build_position_index(session.character_container_map()?, Some("ID"));
        session.group_index = build_position_index(session.group_map()?, None);
        session.guild_extra_index =
            build_position_index(session.guild_extra_map().unwrap_or(&[]), None);

        crate::domain::summaries::extract_summaries(&mut session, progress)?;

        Ok(session)
    }

    /// Port of `PlayerSwapMixin.rebuild_player_caches`
    /// (`game/mixins/player_swap.py:282-291`): invalidate the lazy performance
    /// caches, drop the loaded-guild set, and re-extract both summary maps
    /// from the (now-mutated) world tree. The eager Phase-1 position indexes
    /// are rebuilt too -- unlike Python (whose indexes are all in the lazy
    /// cache layer), this port keeps them eager and built once in `load`, so a
    /// character/container/group-map mutation leaves them stale until they are
    /// rebuilt here.
    ///
    /// `loaded_players` is deliberately NOT cleared: it is this port's
    /// `_player_gvas_files` (the parsed player GVAS the save-out iterates),
    /// which Python's `rebuild_player_caches` also keeps -- Python only clears
    /// `_players`/`_loaded_players` (the domain objects and the id set), whose
    /// only observable effect here is the summaries' `loaded` flag resetting to
    /// `false`, which re-extraction already does. `extract_summaries` is called
    /// with a null progress sink so this reproduces Python's rebuild exactly
    /// (its `_extract_*_summaries` take no `ws_callback` and emit nothing).
    pub fn rebuild_player_caches(&mut self) -> Result<(), CoreError> {
        self.invalidate_performance_caches();
        self.loaded_guilds.clear();
        self.character_index = build_position_index(self.character_map()?, Some("InstanceId"));
        self.item_container_index = build_position_index(self.item_container_map()?, Some("ID"));
        self.character_container_index =
            build_position_index(self.character_container_map()?, Some("ID"));
        self.group_index = build_position_index(self.group_map()?, None);
        self.guild_extra_index = build_position_index(self.guild_extra_map().unwrap_or(&[]), None);
        crate::domain::summaries::extract_summaries(self, &crate::progress::null_progress())
    }

    pub fn world_properties(&self) -> Result<&uesave::Properties, CoreError> {
        props::get(&self.level.root.properties, &["worldSaveData"])
            .and_then(props::struct_properties)
            .ok_or_else(|| CoreError::Parse("worldSaveData missing from Level.sav".to_string()))
    }

    fn required_map(&self, name: &str) -> Result<&[uesave::MapEntry], CoreError> {
        props::get(self.world_properties()?, &[name])
            .and_then(props::map_entries)
            .map(Vec::as_slice)
            .ok_or_else(|| CoreError::Parse(format!("{name} missing from worldSaveData")))
    }

    fn optional_map(&self, name: &str) -> Option<&[uesave::MapEntry]> {
        props::get(self.world_properties().ok()?, &[name])
            .and_then(props::map_entries)
            .map(Vec::as_slice)
    }

    /// `CharacterSaveParameterMap` — every player and pal. Required: absent
    /// only in a malformed save (Python `_set_data` raises `KeyError`).
    pub fn character_map(&self) -> Result<&[uesave::MapEntry], CoreError> {
        self.required_map("CharacterSaveParameterMap")
    }

    pub fn item_container_map(&self) -> Result<&[uesave::MapEntry], CoreError> {
        self.required_map("ItemContainerSaveData")
    }

    pub fn character_container_map(&self) -> Result<&[uesave::MapEntry], CoreError> {
        self.required_map("CharacterContainerSaveData")
    }

    pub fn group_map(&self) -> Result<&[uesave::MapEntry], CoreError> {
        self.required_map("GroupSaveDataMap")
    }

    /// `BaseCampSaveData` — absent in saves that have never had a base built
    /// (Python guards with `"BaseCampSaveData" in world_save_data`).
    pub fn base_camp_map(&self) -> Option<&[uesave::MapEntry]> {
        self.optional_map("BaseCampSaveData")
    }

    /// `GuildExtraSaveDataMap` — same optionality as `base_camp_map`.
    pub fn guild_extra_map(&self) -> Option<&[uesave::MapEntry]> {
        self.optional_map("GuildExtraSaveDataMap")
    }

    /// Compresses the current (possibly edited) `Level.sav` tree back to its
    /// PlM/Oodle `.sav` bytes. Port of `SerializationMixin.sav`
    /// (`serialization.py:106-111`), which does `compress_gvas_to_sav(self.
    /// _gvas_file.write(CUSTOM_PROPERTIES), 0x31)` — the save-type `0x31` and
    /// `PlM` magic are emitted by `savio::write_sav_bytes`.
    pub fn level_sav_bytes(&self) -> Result<Vec<u8>, CoreError> {
        crate::savio::write_sav_bytes(&self.level)
    }

    /// Compresses the loaded `LevelMeta.sav` tree back to its `.sav` bytes, or
    /// `None` when no LevelMeta was loaded. Port of
    /// `SerializationMixin.level_meta_sav` (`serialization.py:172-178`), which
    /// returns `None` when `self._level_meta_gvas_file` is falsy.
    pub fn level_meta_sav_bytes(&self) -> Result<Option<Vec<u8>>, CoreError> {
        match &self.level_meta {
            Some(meta) => Ok(Some(crate::savio::write_sav_bytes(meta)?)),
            None => Ok(None),
        }
    }

    /// Compresses every lazily-loaded player's `.sav` (and its `_dps.sav`
    /// companion, when present) back to `.sav` bytes. Port of
    /// `SerializationMixin.player_gvas_files` (`serialization.py:124-145`),
    /// which iterates `self._player_gvas_files` — ONLY the players lazily
    /// loaded so far (Task 7 populates `loaded_players` on first detail
    /// load), NOT every player the save records.
    ///
    /// Deviation from the brief: the brief's signature returns
    /// `indexmap::IndexMap<Uuid, ..>`. `indexmap` is deliberately not a
    /// dependency of this port (`session.rs`'s `loaded_players` doc comment
    /// records the project-wide `IndexMap → BTreeMap` reconciliation). The
    /// return is a `BTreeMap<Uuid, (Vec<u8>, Option<Vec<u8>>)>` instead. This
    /// is faithful, not a compromise: Python's dict maps each uid to its OWN
    /// independent pair of files, so the map's iteration order affects no
    /// file's contents and reaches no wire array — only the per-uid `.sav`/
    /// `_dps.sav` bytes matter, and those are `BTreeMap`-order-independent.
    pub fn player_sav_bytes(&self) -> Result<BTreeMap<Uuid, PlayerSaveBytes>, CoreError> {
        let mut player_files = BTreeMap::new();
        for (player_id, loaded) in &self.loaded_players {
            let sav_bytes = crate::savio::write_sav_bytes(&loaded.sav)?;
            let dps_bytes = match &loaded.dps {
                Some(dps_save) => Some(crate::savio::write_sav_bytes(dps_save)?),
                None => None,
            };
            player_files.insert(*player_id, (sav_bytes, dps_bytes));
        }
        Ok(player_files)
    }

    /// Renames the world: updates the loaded `LevelMeta.sav`'s
    /// `SaveData.WorldName` property AND the session's own `world_name`. Port
    /// of `SaveManager.set_world_name` (`save_manager.py:191-204`), which
    /// raises `ValueError("No LevelMeta GvasFile has been loaded.")` when no
    /// LevelMeta is loaded — reproduced here as `CoreError::Other` carrying
    /// that exact string (it reaches the UI as an error frame). Python sets
    /// `self.world_name` before writing the property; here the `SaveData`
    /// borrow is live while we write `WorldName`, so `self.world_name` is set
    /// immediately after. Both run only once the LevelMeta guard and the
    /// `SaveData` lookup have succeeded, so the observable result is identical:
    /// neither is touched unless a LevelMeta is loaded.
    pub fn set_world_name(&mut self, new_name: &str) -> Result<(), CoreError> {
        let Some(meta) = self.level_meta.as_mut() else {
            return Err(CoreError::Other(
                "No LevelMeta GvasFile has been loaded.".to_string(),
            ));
        };
        let save_data = props::get_mut(&mut meta.root.properties, &["SaveData"])
            .and_then(props::struct_props_mut)
            .ok_or_else(|| CoreError::Parse("LevelMeta SaveData missing".into()))?;
        save_data.insert("WorldName", props::str_property(new_name));
        self.world_name = new_name.to_string();
        Ok(())
    }
}

/// Per-WS-connection state (spec §3: per-connection sessions fix multi-tab clobbering).
#[derive(Default)]
pub struct Session {
    pub save: Option<SaveSession>,
    /// Transfer-source save (Phase 3).
    pub source: Option<SaveSession>,
}

impl Session {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn save_mut(&mut self) -> Result<&mut SaveSession, CoreError> {
        self.save.as_mut().ok_or(CoreError::SaveNotLoaded)
    }
}

#[cfg(test)]
mod tests {
    use super::Session;
    use crate::error::CoreError;

    #[test]
    fn save_mut_without_loaded_save_is_save_not_loaded() {
        let mut session = Session::new();
        assert!(matches!(session.save_mut(), Err(CoreError::SaveNotLoaded)));
    }
}

#[cfg(test)]
mod load_tests {
    use super::*;
    use crate::progress::null_progress;

    /// Full integration test against a real Steam save directory.
    /// Set PSP_TEST_SAVE_DIR to a directory containing Level.sav,
    /// LevelMeta.sav and Players/. Skipped when unset.
    #[test]
    fn test_load_real_steam_save() {
        let Some(save_dir) = std::env::var_os("PSP_TEST_SAVE_DIR") else {
            eprintln!("PSP_TEST_SAVE_DIR not set, skipping");
            return;
        };
        let save_dir = std::path::PathBuf::from(save_dir);
        let level_sav_bytes = std::fs::read(save_dir.join("Level.sav")).unwrap();
        let level_meta_bytes = std::fs::read(save_dir.join("LevelMeta.sav")).ok();

        let mut player_file_refs = std::collections::BTreeMap::new();
        for entry in std::fs::read_dir(save_dir.join("Players")).unwrap() {
            let path = entry.unwrap().path();
            if path.extension().is_none_or(|ext| ext != "sav") {
                continue;
            }
            let stem = path.file_stem().unwrap().to_string_lossy().to_string();
            let is_dps = stem.contains("_dps");
            let uid: uuid::Uuid = stem.replace("_dps", "").parse().unwrap();
            let file_ref = player_file_refs
                .entry(uid)
                .or_insert(PlayerFileData::Paths {
                    sav: None,
                    dps: None,
                });
            if let PlayerFileData::Paths { sav, dps } = file_ref {
                if is_dps {
                    *dps = Some(path);
                } else {
                    *sav = Some(path);
                }
            }
        }

        let progress_log = std::sync::Arc::new(std::sync::Mutex::new(Vec::<String>::new()));
        let log_clone = progress_log.clone();
        let progress: crate::progress::ProgressSink = std::sync::Arc::new(move |message: &str| {
            log_clone.lock().unwrap().push(message.to_string());
        });

        let session = SaveSession::load(
            SaveKind::Steam {
                level_path: save_dir.join("Level.sav"),
            },
            save_dir.to_string_lossy().into_owned(),
            "steam",
            &level_sav_bytes,
            level_meta_bytes.as_deref(),
            player_file_refs,
            None,
            &progress,
        )
        .unwrap();

        assert_eq!(level_sav_bytes.len() as u64 + 33, session.size);
        assert!(!session.world_name.is_empty());
        assert!(!session.player_summaries.is_empty());
        assert!(!session.character_index.is_empty());
        assert!(session.character_map().unwrap().len() >= session.character_index.len());

        let logged = progress_log.lock().unwrap();
        assert_eq!("Loading Level.sav...", logged[0]);
        assert!(
            logged[1] == "Loading level meta..." || logged[1] == "No LevelMeta.sav found, skipped."
        );
        assert!(logged.contains(&"Extracting player summaries...".to_string()));
        assert!(logged.contains(&"Extracting guild summaries...".to_string()));
    }

    #[test]
    fn test_load_rejects_garbage_bytes() {
        let result = SaveSession::load(
            SaveKind::InMemory,
            "bad".to_string(),
            "steam",
            b"this is not a save file at all",
            None,
            std::collections::BTreeMap::new(),
            None,
            &null_progress(),
        );
        assert!(result.is_err());
    }

    /// Synthetic (no corpus save required) proof that
    /// `invalidate_performance_caches` actually clears every field --
    /// `world_index.rs`'s corpus-gated tests cover the same contract against
    /// real save data, but skip silently when `PSP_TEST_SAVE_DIR` is unset,
    /// so this unit test is the one that always runs.
    #[test]
    fn test_invalidate_performance_caches_clears_every_field() {
        let level = minimal_uesave_save(uesave::Properties::default());
        let mut session = SaveSession::new_for_tests(SaveKind::InMemory, level);
        session.caches = WorldCaches {
            character_index: Some(std::collections::HashMap::from([(Uuid::nil(), 0)])),
            item_container_index: Some(std::collections::HashMap::from([(Uuid::nil(), 0)])),
            character_container_index: Some(std::collections::HashMap::from([(Uuid::nil(), 0)])),
            dynamic_item_index: Some(std::collections::HashMap::from([(Uuid::nil(), 0)])),
            pal_owner_counts: Some(std::collections::HashMap::from([(Uuid::nil(), 1)])),
            player_guild_map: Some(std::collections::HashMap::from([(
                Uuid::nil(),
                Uuid::nil(),
            )])),
        };

        session.invalidate_performance_caches();

        assert!(session.caches.character_index.is_none());
        assert!(session.caches.item_container_index.is_none());
        assert!(session.caches.character_container_index.is_none());
        assert!(session.caches.dynamic_item_index.is_none());
        assert!(session.caches.pal_owner_counts.is_none());
        assert!(session.caches.player_guild_map.is_none());
    }

    /// `new_for_tests` is now the sole construction path for every
    /// hand-built `SaveSession` in this workspace (including `load` itself),
    /// so its defaults matter beyond just this test file. Pins every field
    /// it's responsible for defaulting, not just the two it takes as
    /// arguments.
    #[test]
    fn test_new_for_tests_sets_kind_and_level_and_defaults_everything_else() {
        let level = minimal_uesave_save(uesave::Properties::default());
        let session = SaveSession::new_for_tests(SaveKind::InMemory, level);

        assert!(matches!(session.kind, SaveKind::InMemory));
        assert_eq!("", session.world_name);
        assert_eq!("", session.save_id);
        assert_eq!("steam", session.save_type_label);
        assert_eq!(0, session.size);
        assert!(session.level_meta.is_none());
        assert!(session.player_file_refs.is_empty());
        assert!(session.player_sav_cache.is_empty());
        assert!(session.player_summaries.is_empty());
        assert!(session.guild_summaries.is_empty());
        assert!(session.player_summary_order.is_empty());
        assert!(session.guild_summary_order.is_empty());
        assert!(session.character_index.is_empty());
        assert!(session.item_container_index.is_empty());
        assert!(session.character_container_index.is_empty());
        assert!(session.group_index.is_empty());
        assert!(session.guild_extra_index.is_empty());
        assert!(session.gps.file_path.is_none());
        assert!(!session.gps.loaded);
        assert!(session.loaded_players.is_empty());
        assert!(session.loaded_guilds.is_empty());
        assert!(session.caches.character_index.is_none());
        assert!(session.caches.item_container_index.is_none());
        assert!(session.caches.character_container_index.is_none());
        assert!(session.caches.dynamic_item_index.is_none());
        assert!(session.caches.pal_owner_counts.is_none());
        assert!(session.caches.player_guild_map.is_none());
    }

    /// Python's `_load_world_name`: `world_name if world_name else "Unknown"`
    /// (`palworld_save_pal/game/save_manager.py`). Four distinct shapes of
    /// `LevelMeta.sav`'s property tree, each asserted separately so a
    /// regression in any one branch fails on its own name.
    #[test]
    fn test_world_name_falls_back_to_unknown_when_save_data_absent() {
        assert_eq!(
            "Unknown",
            world_name_from_meta_properties(&uesave::Properties::default())
        );
    }

    #[test]
    fn test_world_name_falls_back_to_unknown_when_world_name_absent() {
        let mut properties = uesave::Properties::default();
        properties.insert("SaveData", struct_property(uesave::Properties::default()));

        assert_eq!("Unknown", world_name_from_meta_properties(&properties));
    }

    #[test]
    fn test_world_name_falls_back_to_unknown_when_world_name_empty() {
        let mut save_data = uesave::Properties::default();
        save_data.insert("WorldName", uesave::Property::Str(String::new()));
        let mut properties = uesave::Properties::default();
        properties.insert("SaveData", struct_property(save_data));

        assert_eq!("Unknown", world_name_from_meta_properties(&properties));
    }

    #[test]
    fn test_world_name_uses_present_non_empty_value() {
        let mut save_data = uesave::Properties::default();
        save_data.insert("WorldName", uesave::Property::Str("My World".to_string()));
        let mut properties = uesave::Properties::default();
        properties.insert("SaveData", struct_property(save_data));

        assert_eq!("My World", world_name_from_meta_properties(&properties));
    }

    /// Builds a `uesave::Save` whose only content that matters to
    /// `world_properties`/`required_map`/`optional_map` is `properties` at
    /// the property-tree root; the header/schema fields only matter to the
    /// (de)serializer, which these tests never touch.
    fn minimal_uesave_save(properties: uesave::Properties) -> uesave::Save {
        uesave::Save {
            header: uesave::Header {
                magic: 0,
                save_game_version: 0,
                package_version: uesave::PackageVersion { ue4: 0, ue5: None },
                engine_version_major: 0,
                engine_version_minor: 0,
                engine_version_patch: 0,
                engine_version_build: 0,
                engine_version: String::new(),
                custom_version: None,
            },
            schemas: uesave::PropertySchemas::default(),
            root: uesave::Root {
                save_game_type: String::new(),
                properties,
            },
            extra: Vec::new(),
        }
    }

    /// A `SaveSession` whose `level` is built from `root_properties` and
    /// every other field is a harmless placeholder — enough to exercise
    /// `world_properties`/`required_map`/`optional_map`, the only methods
    /// these tests call.
    fn session_with_level_properties(root_properties: uesave::Properties) -> SaveSession {
        let mut session =
            SaveSession::new_for_tests(SaveKind::InMemory, minimal_uesave_save(root_properties));
        session.world_name = "Test".to_string();
        session.save_id = "test".to_string();
        session
    }

    fn struct_property(properties: uesave::Properties) -> uesave::Property {
        uesave::Property::Struct(uesave::StructValue::Struct(properties))
    }

    fn empty_map_property() -> uesave::Property {
        uesave::Property::Map(Vec::new())
    }

    #[test]
    fn test_world_properties_missing_from_level_returns_parse_error() {
        let session = session_with_level_properties(uesave::Properties::default());

        match session.world_properties().unwrap_err() {
            CoreError::Parse(message) => {
                assert_eq!("worldSaveData missing from Level.sav", message)
            }
            other => panic!("expected CoreError::Parse, got {other:?}"),
        }
    }

    /// Python raises on absence for these four maps (`_set_data`'s
    /// unconditional `["..."]` indexing); each must fail with a message
    /// naming the missing map, not panic.
    #[test]
    fn test_required_maps_missing_from_world_save_data_return_named_parse_errors() {
        let mut root_properties = uesave::Properties::default();
        root_properties.insert(
            "worldSaveData",
            struct_property(uesave::Properties::default()),
        );
        let session = session_with_level_properties(root_properties);

        let cases: [(Result<&[uesave::MapEntry], CoreError>, &str); 4] = [
            (session.character_map(), "CharacterSaveParameterMap"),
            (session.item_container_map(), "ItemContainerSaveData"),
            (
                session.character_container_map(),
                "CharacterContainerSaveData",
            ),
            (session.group_map(), "GroupSaveDataMap"),
        ];

        for (result, name) in cases {
            match result.unwrap_err() {
                CoreError::Parse(message) => {
                    assert_eq!(format!("{name} missing from worldSaveData"), message)
                }
                other => panic!("expected CoreError::Parse for {name}, got {other:?}"),
            }
        }
    }

    /// `BaseCampSaveData` and `GuildExtraSaveDataMap` are optional (Python
    /// guards both with `"... in world_save_data"`): their absence must
    /// come back `None`, never an `Err`.
    #[test]
    fn test_optional_maps_absent_from_world_save_data_return_none_not_error() {
        let mut root_properties = uesave::Properties::default();
        root_properties.insert(
            "worldSaveData",
            struct_property(uesave::Properties::default()),
        );
        let session = session_with_level_properties(root_properties);

        assert!(session.base_camp_map().is_none());
        assert!(session.guild_extra_map().is_none());
    }

    #[test]
    fn test_optional_maps_present_in_world_save_data_return_their_entries() {
        let mut world_save_data = uesave::Properties::default();
        world_save_data.insert("BaseCampSaveData", empty_map_property());
        world_save_data.insert("GuildExtraSaveDataMap", empty_map_property());
        let mut root_properties = uesave::Properties::default();
        root_properties.insert("worldSaveData", struct_property(world_save_data));
        let session = session_with_level_properties(root_properties);

        assert!(session
            .base_camp_map()
            .is_some_and(|entries| entries.is_empty()));
        assert!(session
            .guild_extra_map()
            .is_some_and(|entries| entries.is_empty()));
    }

    /// `test_load_rejects_garbage_bytes` only proves *some* error comes back;
    /// this pins the exact, non-panicking messages for the three unsupported/
    /// malformed shapes called out by the phase plan: PlZ (zlib) and CNK
    /// (chunk) formats that `uesave`'s decompressor deliberately doesn't
    /// implement yet, and a header truncated before the compression header
    /// can even be read. None of these should ever panic, since Level.sav is
    /// untrusted, attacker-controlled input on the editor's load path.
    #[test]
    fn test_parse_palworld_save_rejects_unsupported_and_truncated_formats_cleanly() {
        // Mirrors uesave::compression::CompressionHeader::read's non-CNK
        // layout: 4-byte uncompressed_len, 4-byte compressed_len, 3-byte
        // magic, 1-byte save_type, then payload.
        fn compression_header(magic: &[u8; 3]) -> Vec<u8> {
            let mut header = vec![0u8; 12];
            header[8..11].copy_from_slice(magic);
            header
        }

        let plz_error = parse_palworld_save(&compression_header(b"PlZ")).unwrap_err();
        assert_eq!(
            "parse error: at offset 0: Zlib compression not yet supported",
            plz_error.to_string()
        );

        let cnk_error = parse_palworld_save(&compression_header(b"CNK")).unwrap_err();
        assert_eq!(
            "parse error: at offset 0: Chunk format not yet supported",
            cnk_error.to_string()
        );

        // Fewer than the 12 bytes CompressionHeader::read needs before it can
        // even inspect a magic value.
        let truncated_error = parse_palworld_save(&[0u8; 4]).unwrap_err();
        assert!(matches!(truncated_error, CoreError::Parse(_)));
        assert!(truncated_error.to_string().contains("io error"));
    }
}
