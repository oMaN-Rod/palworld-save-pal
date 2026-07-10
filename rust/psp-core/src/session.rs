use std::collections::{BTreeMap, HashMap};
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
    pub gps_file_path: Option<PathBuf>,
    pub gps_loaded: bool,
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

        let mut session = SaveSession {
            kind,
            world_name,
            level,
            save_id,
            save_type_label,
            size,
            level_meta,
            player_file_refs,
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
            gps_file_path,
            gps_loaded: false,
        };

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

    pub fn world_properties(&self) -> Result<&uesave::Properties, CoreError> {
        props::get(&self.level.root.properties, &["worldSaveData"])
            .and_then(props::struct_properties)
            .ok_or_else(|| CoreError::Parse("worldSaveData missing from Level.sav".to_string()))
    }

    fn required_map(&self, name: &str) -> Result<&[uesave::MapEntry], CoreError> {
        props::get(self.world_properties()?, &[name])
            .and_then(props::map_entries)
            .ok_or_else(|| CoreError::Parse(format!("{name} missing from worldSaveData")))
    }

    fn optional_map(&self, name: &str) -> Option<&[uesave::MapEntry]> {
        props::get(self.world_properties().ok()?, &[name]).and_then(props::map_entries)
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

    pub fn has_gps_available(&self) -> bool {
        self.gps_loaded || self.gps_file_path.is_some()
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
        SaveSession {
            kind: SaveKind::InMemory,
            world_name: "Test".to_string(),
            level: minimal_uesave_save(root_properties),
            save_id: "test".to_string(),
            save_type_label: "steam",
            size: 0,
            level_meta: None,
            player_file_refs: std::collections::BTreeMap::new(),
            player_sav_cache: std::collections::HashMap::new(),
            player_summaries: std::collections::BTreeMap::new(),
            guild_summaries: std::collections::BTreeMap::new(),
            player_summary_order: Vec::new(),
            guild_summary_order: Vec::new(),
            character_index: std::collections::HashMap::new(),
            item_container_index: std::collections::HashMap::new(),
            character_container_index: std::collections::HashMap::new(),
            group_index: std::collections::HashMap::new(),
            guild_extra_index: std::collections::HashMap::new(),
            gps_file_path: None,
            gps_loaded: false,
        }
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
