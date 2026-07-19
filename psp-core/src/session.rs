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

/// One player's compressed save-out: the `.sav` bytes and, when the player has
/// a `_dps.sav` companion, its bytes too.
pub type PlayerSaveBytes = (Vec<u8>, Option<Vec<u8>>);

/// A lazily loaded player `.sav` (and, when present, its `_dps.sav` companion),
/// cached once parsed so a later edit doesn't re-read the file.
pub struct LoadedPlayer {
    pub uid: Uuid,
    pub sav: crate::ue::Save,
    pub dps: Option<crate::ue::Save>,
}

impl LoadedPlayer {
    /// Primes both files' write schemas at the parse, so no edit path can forget to.
    pub fn new(uid: Uuid, mut sav: crate::ue::Save, dps: Option<crate::ue::Save>) -> Self {
        crate::domain::player::ensure_player_sav_schemas(&mut sav);
        let dps = dps.map(|mut dps| {
            crate::domain::pal::ensure_slot_pal_schemas(&mut dps);
            dps
        });
        Self { uid, sav, dps }
    }
}

/// Lazily built lookup caches over `SaveSession::level`'s world tree. Every
/// field starts `None` and is populated on first use by `domain::world`.
///
/// INVALIDATION CONTRACT: every mutation that inserts or removes a
/// character-map/container-map entry MUST call
/// `SaveSession::invalidate_performance_caches` before returning. Nothing
/// enforces this at compile time — a mutation that forgets leaves a stale
/// index behind that silently resolves the wrong pal on the next lookup
/// (`world_index.rs`'s
/// `stale_character_index_after_removal_would_resolve_the_wrong_entry`
/// demonstrates it).
///
/// All six are `HashMap`: each is a uuid-keyed lookup resolved one key at a
/// time, never iterated for wire output, so their iteration order cannot leak.
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
    /// player uid → number of pals that player owns.
    pub pal_owner_counts: Option<HashMap<Uuid, i64>>,
    /// player uid → guild id.
    pub player_guild_map: Option<HashMap<Uuid, Uuid>>,
}

/// A loaded world save: `Level.sav` plus the indexes and summaries needed to
/// serve on-demand detail loads without re-parsing the whole tree.
pub struct SaveSession {
    pub kind: SaveKind,
    pub world_name: String,
    pub level: crate::ue::Save,
    /// The save's on-disk path (Steam) or GamePass save id — a stable
    /// identifier on the wire.
    pub save_id: String,
    pub save_type_label: &'static str,
    /// `level_sav_bytes.len() + 33`. The 33-byte offset is part of the
    /// `loaded_save_files.size` wire contract the frontend expects.
    pub size: u64,
    pub level_meta: Option<crate::ue::Save>,
    pub world_option: Option<crate::ue::Save>,
    /// Gates every write. A user who never opens the editor must not have
    /// WorldOption.sav rewritten: re-compressing yields an identical GVAS payload but
    /// not necessarily identical bytes.
    pub world_option_dirty: bool,
    pub player_file_refs: BTreeMap<Uuid, PlayerFileData>,
    pub player_sav_cache: HashMap<Uuid, crate::ue::Save>,
    pub player_summaries: BTreeMap<Uuid, PlayerSummary>,
    pub guild_summaries: BTreeMap<Uuid, GuildSummary>,
    /// GVAS-file order of `player_summaries` / `guild_summaries`
    /// (`CharacterSaveParameterMap` order for players, `GroupSaveDataMap` order
    /// for guilds), NOT the `BTreeMap`'s sorted order. `sync_app_state` emits
    /// the summary arrays in this order, so it must be carried separately.
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
    /// GPS (Global Pal Storage) state; the methods that mutate it live in
    /// `domain::gps`.
    pub gps: crate::domain::gps::GpsState,
    /// Lazily loaded/parsed player `.sav` files, keyed like
    /// `player_file_refs`/`player_sav_cache`. `BTreeMap`, not `HashMap`: this
    /// map IS iterated (`player_sav_bytes`), and a nondeterministic order here
    /// would surface as nondeterministic save-out.
    pub loaded_players: BTreeMap<Uuid, LoadedPlayer>,
    /// Guild ids whose full `GuildDto` detail has been lazily loaded.
    pub loaded_guilds: HashSet<Uuid>,
    /// Lazily built lookup caches — see `WorldCaches` for the invalidation
    /// contract every mutation must honor.
    pub caches: WorldCaches,
}

/// Parses a Palworld save (`Level.sav`, `LevelMeta.sav`, a player `.sav`, ...).
/// `SaveReader`'s default `error_to_raw(false)` is deliberate: on an editor
/// path an unparseable property must be a hard error, not silently degraded to
/// raw bytes that would be written back as-is.
pub(crate) fn parse_palworld_save(bytes: &[u8]) -> Result<crate::ue::Save, CoreError> {
    crate::ue::SaveReader::new()
        .game::<crate::ue::Palworld>()
        .types(crate::ue::games::palworld::palworld_types())
        .read(Cursor::new(bytes))
        .map_err(|parse_error| CoreError::Parse(parse_error.to_string()))
}

/// `SaveData.WorldName` from a LevelMeta (or Level) property tree, or `None`
/// when the property is absent or empty. Callers supply their own placeholder:
/// `world_name_from_meta_properties` uses `"Unknown"`,
/// `gamepass::scan::world_name_from_level_meta` uses `"Unknown World"`.
pub(crate) fn world_name_property(properties: &crate::ue::Properties) -> Option<String> {
    props::get(properties, &["SaveData", "WorldName"])
        .and_then(props::as_str)
        .filter(|name| !name.is_empty())
        .map(|name| name.to_string())
}

pub(crate) fn world_name_from_meta_properties(properties: &crate::ue::Properties) -> String {
    world_name_property(properties).unwrap_or_else(|| "Unknown".to_string())
}

/// Sets `SaveData.WorldName` on a LevelMeta (or Level) save in place. Shared by
/// `SaveSession::set_world_name` and `gamepass::scan::set_world_name_in_level_meta`.
///
/// Takes the whole `Save`, not its properties: a save whose world was never named
/// recorded no `WorldName` schema, and this write creates the property.
pub(crate) fn set_world_name_property(
    save: &mut crate::ue::Save,
    new_name: &str,
) -> Result<(), CoreError> {
    let save_data = props::get_mut(&mut save.root.properties, &["SaveData"])
        .and_then(props::struct_props_mut)
        .ok_or_else(|| CoreError::Parse("LevelMeta SaveData missing".into()))?;
    save_data.insert("WorldName", props::str_property(new_name));
    props::ensure_schema(
        save,
        "SaveData.WorldName".to_string(),
        crate::ue::PropertyTagPartial {
            id: None,
            data: crate::ue::PropertyTagDataPartial::Other(crate::ue::PropertyType::StrProperty),
        },
    );
    Ok(())
}

/// Resolves a `MapEntry`'s key to a `Uuid`: `nested_field` names a struct
/// field within the key (e.g. `CharacterSaveParameterMap`'s key struct has
/// both `PlayerUId` and `InstanceId`), or `None` when the key itself is a
/// bare `Guid` (e.g. `GroupSaveDataMap`, `GuildExtraSaveDataMap`).
fn map_entry_key_uuid(entry: &crate::ue::MapEntry, nested_field: Option<&str>) -> Option<Uuid> {
    match nested_field {
        None => props::as_uuid(&entry.key),
        Some(field) => props::get_in(&entry.key, &[field]).and_then(props::as_uuid),
    }
}

/// Builds a `Uuid -> position` index over a map's entries. Entries whose key
/// can't be resolved to a `Uuid` are skipped, so one malformed entry never
/// fails the whole load.
fn build_position_index(
    entries: &[crate::ue::MapEntry],
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
    /// field gets an empty placeholder. `load` builds on this too, so a new
    /// field only needs a default in one place.
    pub fn new_for_tests(kind: SaveKind, level: crate::ue::Save) -> Self {
        SaveSession {
            kind,
            world_name: String::new(),
            level,
            save_id: String::new(),
            save_type_label: "steam",
            size: 0,
            level_meta: None,
            world_option: None,
            world_option_dirty: false,
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

    /// Resets every lazily built cache to `None` so the next accessor rebuilds
    /// it from the current world tree. Mandatory after any character-map or
    /// container-map mutation; see `WorldCaches`.
    pub fn invalidate_performance_caches(&mut self) {
        self.caches = WorldCaches::default();
    }

    /// Parses `Level.sav` (and `LevelMeta.sav`, when present) and builds the
    /// position indexes and summaries.
    ///
    /// `emit_top_level_progress` gates ONLY the leading `"Loading Level.sav..."`
    /// frame. `select_save`/`load_zip` pass `true`; the transfer path emits its
    /// own `"Loading {label} Level.sav..."` first and passes `false` to avoid a
    /// duplicate. Every later frame is emitted regardless.
    #[allow(clippy::too_many_arguments)]
    pub fn load(
        kind: SaveKind,
        save_id: String,
        save_type_label: &'static str,
        level_sav_bytes: &[u8],
        level_meta_bytes: Option<&[u8]>,
        world_option_bytes: Option<&[u8]>,
        player_file_refs: BTreeMap<Uuid, PlayerFileData>,
        gps_file_path: Option<PathBuf>,
        emit_top_level_progress: bool,
        progress: &ProgressSink,
    ) -> Result<Self, CoreError> {
        if emit_top_level_progress {
            progress("Loading Level.sav...");
        }
        let mut level = parse_palworld_save(level_sav_bytes)?;
        // Priming at the parse, not at each mutation site, is what keeps a later edit
        // from depending on which properties this particular file happened to carry.
        crate::domain::pal::ensure_pal_property_schemas(&mut level);
        crate::domain::containers::ensure_container_schemas(&mut level);

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

        // A broken WorldOption must not cost the user their world: Level.sav is the
        // world, WorldOption is config. Degrade to absent + warn instead of failing
        // the load (this is why it does NOT use `error_to_raw(false)` semantics).
        let world_option = world_option_bytes.and_then(|bytes| {
            match parse_palworld_save(bytes) {
                Ok(mut save) => {
                    crate::domain::world_option::ensure_world_option_schemas(&mut save);
                    Some(save)
                }
                Err(error) => {
                    tracing::warn!("WorldOption.sav failed to parse, ignoring: {error}");
                    None
                }
            }
        });

        let size = level_sav_bytes.len() as u64 + 33;

        let mut session = SaveSession::new_for_tests(kind, level);
        session.world_name = world_name;
        session.save_id = save_id;
        session.save_type_label = save_type_label;
        session.size = size;
        session.level_meta = level_meta;
        session.world_option = world_option;
        session.world_option_dirty = false;
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

    /// Invalidates the lazy caches, drops the loaded-guild set, rebuilds the
    /// eager position indexes, and re-extracts both summary maps from the
    /// now-mutated world tree.
    ///
    /// `loaded_players` is deliberately NOT cleared: it holds the parsed player
    /// GVAS that the save-out path iterates, and dropping it would discard
    /// pending edits.
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

    /// Force-loads `player_uid`'s GVAS into `loaded_players`. Needed by
    /// handlers that resolve a player purely as a transfer DESTINATION, since
    /// `domain::player::build_player_dto` returns `Ok(None)` for any player the
    /// frontend has not opened yet.
    ///
    /// Callers must first confirm the uid is real (via
    /// `player_summaries`/`player_file_refs`): this is a silent no-op, not an
    /// error, when the player is already loaded or has no file reference.
    pub fn ensure_player_loaded(&mut self, player_uid: Uuid) -> Result<(), CoreError> {
        crate::transfer::ensure_player_gvas_loaded(self, player_uid)
    }

    pub fn world_properties(&self) -> Result<&crate::ue::Properties, CoreError> {
        props::get(&self.level.root.properties, &["worldSaveData"])
            .and_then(props::struct_properties)
            .ok_or_else(|| CoreError::Parse("worldSaveData missing from Level.sav".to_string()))
    }

    fn required_map(&self, name: &str) -> Result<&[crate::ue::MapEntry], CoreError> {
        props::get(self.world_properties()?, &[name])
            .and_then(props::map_entries)
            .map(Vec::as_slice)
            .ok_or_else(|| CoreError::Parse(format!("{name} missing from worldSaveData")))
    }

    fn optional_map(&self, name: &str) -> Option<&[crate::ue::MapEntry]> {
        props::get(self.world_properties().ok()?, &[name])
            .and_then(props::map_entries)
            .map(Vec::as_slice)
    }

    /// `CharacterSaveParameterMap` — every player and pal. Absent only in a
    /// malformed save.
    pub fn character_map(&self) -> Result<&[crate::ue::MapEntry], CoreError> {
        self.required_map("CharacterSaveParameterMap")
    }

    pub fn item_container_map(&self) -> Result<&[crate::ue::MapEntry], CoreError> {
        self.required_map("ItemContainerSaveData")
    }

    pub fn character_container_map(&self) -> Result<&[crate::ue::MapEntry], CoreError> {
        self.required_map("CharacterContainerSaveData")
    }

    pub fn group_map(&self) -> Result<&[crate::ue::MapEntry], CoreError> {
        self.required_map("GroupSaveDataMap")
    }

    /// `BaseCampSaveData` — absent in saves that never had a base built.
    pub fn base_camp_map(&self) -> Option<&[crate::ue::MapEntry]> {
        self.optional_map("BaseCampSaveData")
    }

    /// `GuildExtraSaveDataMap` — same optionality as `base_camp_map`.
    pub fn guild_extra_map(&self) -> Option<&[crate::ue::MapEntry]> {
        self.optional_map("GuildExtraSaveDataMap")
    }

    /// Compresses the current (possibly edited) `Level.sav` tree back to `.sav`
    /// bytes.
    pub fn level_sav_bytes(&self) -> Result<Vec<u8>, CoreError> {
        crate::savio::write_sav_bytes(&self.level)
    }

    /// `None` when no `LevelMeta.sav` was loaded.
    pub fn level_meta_sav_bytes(&self) -> Result<Option<Vec<u8>>, CoreError> {
        match &self.level_meta {
            Some(meta) => Ok(Some(crate::savio::write_sav_bytes(meta)?)),
            None => Ok(None),
        }
    }

    /// Compresses every LOADED player's `.sav` (and `_dps.sav` companion) back
    /// to bytes — only the players opened so far, not every player in the save.
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

    /// Updates the loaded `LevelMeta.sav`'s `SaveData.WorldName` AND the
    /// session's own `world_name`. Neither is touched unless a LevelMeta is
    /// loaded and its `SaveData` lookup succeeds; the error string reaches the
    /// UI verbatim.
    pub fn set_world_name(&mut self, new_name: &str) -> Result<(), CoreError> {
        let Some(meta) = self.level_meta.as_mut() else {
            return Err(CoreError::Other(
                "No LevelMeta GvasFile has been loaded.".to_string(),
            ));
        };
        set_world_name_property(meta, new_name)?;
        self.world_name = new_name.to_string();
        Ok(())
    }

    pub fn world_option_dto(&self) -> crate::dto::world_option::WorldOptionDto {
        use crate::domain::world_option;
        match &self.world_option {
            Some(save) => crate::dto::world_option::WorldOptionDto {
                present: true,
                version: world_option::read_version(save),
                settings: world_option::read_settings(save)
                    .into_iter()
                    .map(|entry| crate::dto::world_option::WorldOptionEntryDto {
                        key: entry.key,
                        kind: entry.kind.wire_tag().to_string(),
                        value: entry.value,
                    })
                    .collect(),
            },
            None => crate::dto::world_option::WorldOptionDto {
                present: false,
                version: 0,
                settings: Vec::new(),
            },
        }
    }

    pub fn apply_world_option_patch(
        &mut self,
        patch: &[crate::domain::world_option::WorldOptionPatch],
    ) -> Result<(), CoreError> {
        let save = self
            .world_option
            .as_mut()
            .ok_or_else(|| CoreError::Other("No WorldOption.sav loaded".to_string()))?;
        if crate::domain::world_option::apply_patch(save, patch)? {
            self.world_option_dirty = true;
        }
        Ok(())
    }

    /// `&mut` access to `Level.sav`'s ROOT properties — one level above
    /// `worldSaveData`, which is only one top-level entry among siblings.
    /// `props::swap_uuid_values_deep` must walk the whole root.
    pub fn level_properties_mut(&mut self) -> &mut crate::ue::Properties {
        &mut self.level.root.properties
    }

    /// Writes `second` into `first`'s loaded GVAS at both `SaveData.PlayerUId`
    /// and `SaveData.IndividualId.PlayerUId`, and `first` into `second`'s. A
    /// no-op for whichever uid has no loaded GVAS.
    pub fn swap_player_gvas_uids(&mut self, first: uuid::Uuid, second: uuid::Uuid) {
        fn set_player_uid(sav: &mut crate::ue::Save, new_uid: uuid::Uuid) {
            let Ok(save_data) = crate::domain::player::save_data_props_mut(sav) else {
                return;
            };
            save_data.insert("PlayerUId", props::guid_property(new_uid));
            if let Some(individual_id) =
                props::get_mut(save_data, &["IndividualId"]).and_then(props::struct_props_mut)
            {
                individual_id.insert("PlayerUId", props::guid_property(new_uid));
            }
        }
        if let Some(loaded) = self.loaded_players.get_mut(&first) {
            set_player_uid(&mut loaded.sav, second);
        }
        if let Some(loaded) = self.loaded_players.get_mut(&second) {
            set_player_uid(&mut loaded.sav, first);
        }
    }

    /// Runs only when BOTH uids already have a loaded GVAS and a file
    /// reference, so this is a plain swap, never a partial one.
    ///
    /// Note the asymmetry: the two `.sav` trees trade places between the uids
    /// (each carrying its `swap_player_gvas_uids`-updated `PlayerUId`), but
    /// each uid KEEPS its own original `_dps.sav` companion rather than the one
    /// that traveled with the swapped `.sav`.
    pub fn swap_player_file_refs(&mut self, first: uuid::Uuid, second: uuid::Uuid) {
        if let (Some(first_loaded), Some(second_loaded)) = (
            self.loaded_players.remove(&first),
            self.loaded_players.remove(&second),
        ) {
            self.loaded_players.insert(
                first,
                LoadedPlayer {
                    uid: first,
                    sav: second_loaded.sav,
                    dps: first_loaded.dps,
                },
            );
            self.loaded_players.insert(
                second,
                LoadedPlayer {
                    uid: second,
                    sav: first_loaded.sav,
                    dps: second_loaded.dps,
                },
            );
        }

        if let (Some(first_ref), Some(second_ref)) = (
            self.player_file_refs.remove(&first),
            self.player_file_refs.remove(&second),
        ) {
            self.player_file_refs.insert(first, second_ref);
            self.player_file_refs.insert(second, first_ref);
        }
    }
}

/// The on-disk locations a standalone `TransferTarget` was loaded from, so the
/// auto-save step can write the edited session straight back without
/// re-deriving paths. `level_meta` is `None` when the load found no
/// `LevelMeta.sav`; `save_dir` is `level_sav`'s parent, the directory backed up
/// before writing.
pub struct TransferSaveInfo {
    pub level_sav: std::path::PathBuf,
    pub level_meta: Option<std::path::PathBuf>,
    pub players_dir: std::path::PathBuf,
    pub save_dir: std::path::PathBuf,
}

/// A standalone Steam save loaded as the transfer TARGET (`role: "target"` on
/// `load_source_save`), as opposed to the main `Session::save`, which can also
/// serve as a target.
pub struct TransferTarget {
    pub session: SaveSession,
    pub save_info: TransferSaveInfo,
}

/// Per-WS-connection state; sessions are per-connection so multiple tabs cannot
/// clobber each other.
#[derive(Default)]
pub struct Session {
    pub save: Option<SaveSession>,
    pub source: Option<SaveSession>,
    /// Standalone transfer-target save. When absent, `transfer_player` falls
    /// back to `save` as the target.
    pub transfer_target: Option<TransferTarget>,
    /// Saves discovered by `select_gamepass_directory`, keyed by save id;
    /// consumed by `select_gamepass_save`.
    pub gamepass_saves: HashMap<String, crate::dto::gamepass::GamepassSaveData>,
    /// Read by the gamepass branch of `save_modded_save` to locate the original
    /// containers to copy from.
    pub selected_gamepass_save: Option<crate::dto::gamepass::GamepassSaveData>,
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

    /// Full integration test against the committed `v1_relics` Steam save
    /// fixture (Level.sav, LevelMeta.sav and Players/). Never skips.
    #[test]
    fn test_load_real_steam_save() {
        let save_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../tests/fixtures/saves/v1_relics");
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
            None,
            player_file_refs,
            None,
            true,
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
            None,
            std::collections::BTreeMap::new(),
            None,
            true,
            &null_progress(),
        );
        assert!(result.is_err());
    }

    #[test]
    fn test_invalidate_performance_caches_clears_every_field() {
        let level = minimal_uesave_save(crate::ue::Properties::default());
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

    /// `new_for_tests` is the sole construction path for every hand-built
    /// `SaveSession` (including `load`), so pin every field it defaults.
    #[test]
    fn test_new_for_tests_sets_kind_and_level_and_defaults_everything_else() {
        let level = minimal_uesave_save(crate::ue::Properties::default());
        let session = SaveSession::new_for_tests(SaveKind::InMemory, level);

        assert!(matches!(session.kind, SaveKind::InMemory));
        assert_eq!("", session.world_name);
        assert_eq!("", session.save_id);
        assert_eq!("steam", session.save_type_label);
        assert_eq!(0, session.size);
        assert!(session.level_meta.is_none());
        assert!(session.world_option.is_none());
        assert!(!session.world_option_dirty);
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

    #[test]
    fn test_world_name_falls_back_to_unknown_when_save_data_absent() {
        assert_eq!(
            "Unknown",
            world_name_from_meta_properties(&crate::ue::Properties::default())
        );
    }

    #[test]
    fn test_world_name_falls_back_to_unknown_when_world_name_absent() {
        let mut properties = crate::ue::Properties::default();
        properties.insert("SaveData", struct_property(crate::ue::Properties::default()));

        assert_eq!("Unknown", world_name_from_meta_properties(&properties));
    }

    #[test]
    fn test_world_name_falls_back_to_unknown_when_world_name_empty() {
        let mut save_data = crate::ue::Properties::default();
        save_data.insert("WorldName", crate::ue::Property::Str(String::new()));
        let mut properties = crate::ue::Properties::default();
        properties.insert("SaveData", struct_property(save_data));

        assert_eq!("Unknown", world_name_from_meta_properties(&properties));
    }

    #[test]
    fn test_world_name_uses_present_non_empty_value() {
        let mut save_data = crate::ue::Properties::default();
        save_data.insert("WorldName", crate::ue::Property::Str("My World".to_string()));
        let mut properties = crate::ue::Properties::default();
        properties.insert("SaveData", struct_property(save_data));

        assert_eq!("My World", world_name_from_meta_properties(&properties));
    }

    /// A `crate::ue::Save` carrying only root `properties`; the header/schema
    /// fields only matter to the (de)serializer, which these tests never touch.
    fn minimal_uesave_save(properties: crate::ue::Properties) -> crate::ue::Save {
        crate::ue::Save {
            header: crate::ue::Header {
                magic: 0,
                save_game_version: 0,
                package_version: crate::ue::PackageVersion { ue4: 0, ue5: None },
                engine_version_major: 0,
                engine_version_minor: 0,
                engine_version_patch: 0,
                engine_version_build: 0,
                engine_version: String::new(),
                custom_version: None,
            },
            schemas: crate::ue::PropertySchemas::default(),
            root: crate::ue::Root {
                save_game_type: String::new(),
                properties,
            },
            extra: Vec::new(),
        }
    }

    fn session_with_level_properties(root_properties: crate::ue::Properties) -> SaveSession {
        let mut session =
            SaveSession::new_for_tests(SaveKind::InMemory, minimal_uesave_save(root_properties));
        session.world_name = "Test".to_string();
        session.save_id = "test".to_string();
        session
    }

    fn struct_property(properties: crate::ue::Properties) -> crate::ue::Property {
        crate::ue::Property::Struct(crate::ue::StructValue::Struct(properties))
    }

    fn empty_map_property() -> crate::ue::Property {
        crate::ue::Property::Map(Vec::new())
    }

    #[test]
    fn test_world_properties_missing_from_level_returns_parse_error() {
        let session = session_with_level_properties(crate::ue::Properties::default());

        match session.world_properties().unwrap_err() {
            CoreError::Parse(message) => {
                assert_eq!("worldSaveData missing from Level.sav", message)
            }
            other => panic!("expected CoreError::Parse, got {other:?}"),
        }
    }

    /// Each of the four required maps must fail with a message naming the
    /// missing map, never panic.
    #[test]
    fn test_required_maps_missing_from_world_save_data_return_named_parse_errors() {
        let mut root_properties = crate::ue::Properties::default();
        root_properties.insert(
            "worldSaveData",
            struct_property(crate::ue::Properties::default()),
        );
        let session = session_with_level_properties(root_properties);

        let cases: [(Result<&[crate::ue::MapEntry], CoreError>, &str); 4] = [
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

    /// `BaseCampSaveData` and `GuildExtraSaveDataMap` are optional: absence
    /// must come back `None`, never an `Err`.
    #[test]
    fn test_optional_maps_absent_from_world_save_data_return_none_not_error() {
        let mut root_properties = crate::ue::Properties::default();
        root_properties.insert(
            "worldSaveData",
            struct_property(crate::ue::Properties::default()),
        );
        let session = session_with_level_properties(root_properties);

        assert!(session.base_camp_map().is_none());
        assert!(session.guild_extra_map().is_none());
    }

    #[test]
    fn test_optional_maps_present_in_world_save_data_return_their_entries() {
        let mut world_save_data = crate::ue::Properties::default();
        world_save_data.insert("BaseCampSaveData", empty_map_property());
        world_save_data.insert("GuildExtraSaveDataMap", empty_map_property());
        let mut root_properties = crate::ue::Properties::default();
        root_properties.insert("worldSaveData", struct_property(world_save_data));
        let session = session_with_level_properties(root_properties);

        assert!(session
            .base_camp_map()
            .is_some_and(|entries| entries.is_empty()));
        assert!(session
            .guild_extra_map()
            .is_some_and(|entries| entries.is_empty()));
    }

    /// Level.sav is untrusted input on the load path, so a malformed or
    /// truncated compression header must produce a clean error, never a panic.
    #[test]
    fn test_parse_palworld_save_rejects_unsupported_and_truncated_formats_cleanly() {
        // Non-CNK compression header: 4-byte uncompressed_len, 4-byte
        // compressed_len, 3-byte magic, 1-byte save_type, then payload.
        fn compression_header(magic: &[u8; 3]) -> Vec<u8> {
            let mut header = vec![0u8; 12];
            header[8..11].copy_from_slice(magic);
            header
        }

        // PlZ (zlib) and CNK (chunked zlib) are supported magics, so these fail
        // on their payloads instead: a bad zlib stream for PlZ, and a 12-byte
        // buffer where CNK needs 24 for its nested header.
        let plz_error = parse_palworld_save(&compression_header(b"PlZ")).unwrap_err();
        assert!(matches!(plz_error, CoreError::Parse(_)), "{plz_error}");

        let cnk_error = parse_palworld_save(&compression_header(b"CNK")).unwrap_err();
        assert!(matches!(cnk_error, CoreError::Parse(_)), "{cnk_error}");

        // A genuinely unknown magic is still rejected cleanly.
        let unknown_error = parse_palworld_save(&compression_header(b"XYZ")).unwrap_err();
        assert!(
            matches!(unknown_error, CoreError::Parse(_)),
            "{unknown_error}"
        );

        // Fewer than the 12 bytes CompressionHeader::read needs before it can
        // even inspect a magic value.
        let truncated_error = parse_palworld_save(&[0u8; 4]).unwrap_err();
        assert!(matches!(truncated_error, CoreError::Parse(_)));
        assert!(truncated_error.to_string().contains("io error"));
    }

    /// Load the lexicographically first fixture from tests/fixtures/world_option/.
    fn load_world_option_fixture() -> Result<crate::ue::Save, CoreError> {
        let fixture_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../tests/fixtures/world_option/19804164.sav");
        let bytes = std::fs::read(&fixture_path)
            .map_err(|e| CoreError::Other(format!("Failed to read fixture: {e}")))?;
        let mut save = crate::savio::read_sav_bytes(&bytes)?;
        crate::domain::world_option::ensure_world_option_schemas(&mut save);
        Ok(save)
    }

    #[test]
    fn apply_world_option_patch_sets_dirty_only_on_real_change() {
        let world_option = load_world_option_fixture().unwrap();
        let mut session = SaveSession::new_for_tests(SaveKind::InMemory, minimal_uesave_save(crate::ue::Properties::default()));
        session.world_option = Some(world_option);
        session.world_option_dirty = false;

        // Assert dirty is false initially
        assert!(!session.world_option_dirty);

        // Read a current setting to know what value exists
        let current_settings = crate::domain::world_option::read_settings(
            session.world_option.as_ref().unwrap(),
        );
        assert!(!current_settings.is_empty(), "fixture must have at least one setting");
        let first_setting = &current_settings[0];
        let original_value = first_setting.value.clone();

        // Create a genuinely different value based on the setting's type
        let different_value = match first_setting.kind {
            crate::domain::world_option::WoKind::Bool => serde_json::json!(!original_value.as_bool().unwrap()),
            crate::domain::world_option::WoKind::Int => {
                let current = original_value.as_i64().unwrap_or(0);
                serde_json::json!(current + 1)
            }
            crate::domain::world_option::WoKind::Float => {
                let current = original_value.as_f64().unwrap_or(0.0);
                serde_json::json!(current + 1.0)
            }
            crate::domain::world_option::WoKind::Str => serde_json::json!("different_value"),
            crate::domain::world_option::WoKind::Name => serde_json::json!("different_name"),
            crate::domain::world_option::WoKind::Enum(name) => {
                // Pick a different variant; if it's "Custom", use "Easy", otherwise use "Custom"
                let current_str = original_value.as_str().unwrap_or("");
                let new_variant = if current_str.contains("Custom") { "Easy" } else { "Custom" };
                serde_json::json!(format!("{}::{}", name, new_variant))
            }
            crate::domain::world_option::WoKind::EnumArray => serde_json::json!(["SomeValue"]),
            crate::domain::world_option::WoKind::NameArray => serde_json::json!(["SomeName"]),
        };

        // Apply a patch with a genuinely different value
        let patch = crate::domain::world_option::WorldOptionPatch {
            key: first_setting.key.clone(),
            value: different_value,
        };
        let result = session.apply_world_option_patch(&[patch]);
        assert!(result.is_ok(), "patch should succeed: {result:?}");
        assert!(session.world_option_dirty, "dirty should be true after real change");

        // Now test the no-op case with a fresh session
        let world_option = load_world_option_fixture().unwrap();
        let mut session = SaveSession::new_for_tests(SaveKind::InMemory, minimal_uesave_save(crate::ue::Properties::default()));
        session.world_option = Some(world_option);
        session.world_option_dirty = false;

        // Apply a patch with the EXISTING value (no-op)
        let patch = crate::domain::world_option::WorldOptionPatch {
            key: first_setting.key.clone(),
            value: original_value,
        };
        let result = session.apply_world_option_patch(&[patch]);
        assert!(result.is_ok(), "no-op patch should succeed");
        assert!(!session.world_option_dirty, "dirty should remain false on no-op change");
    }

    #[test]
    fn apply_world_option_patch_errors_when_no_world_option_loaded() {
        let mut session = SaveSession::new_for_tests(SaveKind::InMemory, minimal_uesave_save(crate::ue::Properties::default()));
        session.world_option = None;

        let patch = crate::domain::world_option::WorldOptionPatch {
            key: "ExpRate".to_string(),
            value: serde_json::json!(1.0),
        };
        let result = session.apply_world_option_patch(&[patch]);
        assert!(result.is_err(), "should error when world_option is None");
        match result.unwrap_err() {
            CoreError::Other(msg) => {
                assert!(msg.contains("No WorldOption.sav loaded"));
            }
            other => panic!("expected CoreError::Other, got {other:?}"),
        }
    }

    #[test]
    fn load_degrades_gracefully_on_corrupt_world_option() {
        let level_fixture_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../tests/fixtures/saves/v1_relics/Level.sav");
        let valid_level_bytes = std::fs::read(&level_fixture_path)
            .expect("failed to read Level.sav fixture");

        let corrupt_world_option_bytes = b"not a real sav";

        let session = SaveSession::load(
            SaveKind::InMemory,
            "test_corrupt".to_string(),
            "steam",
            &valid_level_bytes,
            None,
            Some(corrupt_world_option_bytes),
            std::collections::BTreeMap::new(),
            None,
            false,
            &null_progress(),
        );

        // Load must succeed (corrupt WorldOption does not fail the whole load)
        assert!(session.is_ok(), "load should succeed despite corrupt WorldOption");

        let session = session.unwrap();
        // The session's world_option should be None (degraded to absent)
        assert!(session.world_option.is_none(), "corrupted WorldOption should be degraded to None");
    }
}
