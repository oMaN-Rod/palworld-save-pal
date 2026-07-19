//! The wire message-type vocabulary shared with the frontend. Every wire string
//! is the exact snake_case of its variant name; a variant that ever needs to
//! break that rule needs an explicit #[serde(rename = "...")] AND a matching
//! literal here.

macro_rules! define_message_types {
    ($($variant:ident => $wire:literal),+ $(,)?) => {
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
        #[serde(rename_all = "snake_case")]
        pub enum MessageType {
            $($variant),+
        }

        impl MessageType {
            pub const ALL: &'static [MessageType] = &[$(MessageType::$variant),+];

            /// The exact wire string (equals the serde serialization).
            pub fn as_wire(&self) -> &'static str {
                match self {
                    $(MessageType::$variant => $wire),+
                }
            }

            pub fn from_wire(wire: &str) -> Option<MessageType> {
                match wire {
                    $($wire => Some(MessageType::$variant),)+
                    _ => None,
                }
            }
        }
    };
}

define_message_types! {
    // Pal management
    AddPal => "add_pal",
    AddDpsPal => "add_dps_pal",
    AddGpsPal => "add_gps_pal",
    ClonePal => "clone_pal",
    CloneDpsPal => "clone_dps_pal",
    CloneGpsPal => "clone_gps_pal",
    DeletePals => "delete_pals",
    DeleteDpsPals => "delete_dps_pals",
    DeleteGpsPals => "delete_gps_pals",
    GetPalDetails => "get_pal_details",
    GetPals => "get_pals",
    GetGpsPals => "get_gps_pals",
    HealAllPals => "heal_all_pals",
    HealPals => "heal_pals",
    MovePal => "move_pal",
    // UPS (Universal Pal Storage)
    GetUpsPals => "get_ups_pals",
    GetUpsAllFilteredIds => "get_ups_all_filtered_ids",
    AddUpsPal => "add_ups_pal",
    UpdateUpsPal => "update_ups_pal",
    DeleteUpsPals => "delete_ups_pals",
    CloneUpsPal => "clone_ups_pal",
    CloneToUps => "clone_to_ups",
    ExportUpsPal => "export_ups_pal",
    CloneGpsPalToPlayer => "clone_gps_pal_to_player",
    ImportToUps => "import_to_ups",
    GetUpsCollections => "get_ups_collections",
    CreateUpsCollection => "create_ups_collection",
    UpdateUpsCollection => "update_ups_collection",
    DeleteUpsCollection => "delete_ups_collection",
    GetUpsTags => "get_ups_tags",
    CreateUpsTag => "create_ups_tag",
    UpdateUpsTag => "update_ups_tag",
    DeleteUpsTag => "delete_ups_tag",
    GetUpsStats => "get_ups_stats",
    NukeUpsPals => "nuke_ups_pals",
    // Player management
    DeletePlayer => "delete_player",
    SetTechnologyData => "set_technology_data",
    // Lazy loading — summaries
    GetPlayerSummaries => "get_player_summaries",
    GetGuildSummaries => "get_guild_summaries",
    // Lazy loading — on-demand details
    RequestPlayerDetails => "request_player_details",
    GetPlayerDetailsResponse => "get_player_details_response",
    RequestGuildDetails => "request_guild_details",
    GetGuildDetailsResponse => "get_guild_details_response",
    RequestGps => "request_gps",
    GetGpsResponse => "get_gps_response",
    // Game data retrieval
    GetActiveSkills => "get_active_skills",
    GetBuildings => "get_buildings",
    GetElements => "get_elements",
    GetExpData => "get_exp_data",
    GetRelicData => "get_relic_data",
    GetMapObjects => "get_map_objects",
    GetBosses => "get_bosses",
    GetRelics => "get_relics",
    GetFastTravelPoints => "get_fast_travel_points",
    GetEffigies => "get_effigies",
    GetGuilds => "get_guilds",
    GetItems => "get_items",
    GetMissions => "get_missions",
    GetPalSummaries => "get_pal_summaries",
    GetPassiveSkills => "get_passive_skills",
    GetPlayers => "get_players",
    GetTechnologies => "get_technologies",
    GetUiCommon => "get_ui_common",
    GetVersion => "get_version",
    GetWorkSuitability => "get_work_suitability",
    GetFriendshipData => "get_friendship_data",
    // Presets
    AddPreset => "add_preset",
    DeletePreset => "delete_preset",
    GetPresets => "get_presets",
    UpdatePreset => "update_preset",
    ExportPreset => "export_preset",
    ImportPreset => "import_preset",
    // Guild management
    DeleteGuild => "delete_guild",
    GetLabResearch => "get_lab_research",
    UpdateLabResearch => "update_lab_research",
    // Save file management
    DownloadSaveFile => "download_save_file",
    LoadedSaveFiles => "loaded_save_files",
    LoadZipFile => "load_zip_file",
    NoFileSelected => "no_file_selected",
    SaveModdedSave => "save_modded_save",
    SelectGamepassSave => "select_gamepass_save",
    SelectSave => "select_save",
    UpdateSaveFile => "update_save_file",
    RenameWorld => "rename_world",
    UnlockMap => "unlock_map",
    // Settings management
    GetSettings => "get_settings",
    UpdateSettings => "update_settings",
    NukePresets => "nuke_presets",
    // System messages
    Error => "error",
    ProgressMessage => "progress_message",
    SyncAppState => "sync_app_state",
    Warning => "warning",
    // Utility
    OpenInBrowser => "open_in_browser",
    // Debug
    GetGuildRawData => "get_guild_raw_data",
    GetRawData => "get_raw_data",
    // Utility (gamepass / conversion)
    OpenFolder => "open_folder",
    ConvertSavFile => "convert_sav_file",
    ConvertSaveFormat => "convert_save_format",
    ScanGamepassSaves => "scan_gamepass_saves",
    DeleteGamepassSave => "delete_gamepass_save",
    DeleteGamepassPlayer => "delete_gamepass_player",
    RenameGamepassWorld => "rename_gamepass_world",
    // Tools
    ConvertSteamId => "convert_steam_id",
    SwapPlayerUids => "swap_player_uids",
    LoadSourceSave => "load_source_save",
    GetSourcePlayers => "get_source_players",
    TransferPlayer => "transfer_player",
    UnloadSourceSave => "unload_source_save",
    // Server management
    ListServers => "list_servers",
    GetServer => "get_server",
    CreateServer => "create_server",
    UpdateServer => "update_server",
    DeleteServer => "delete_server",
    StartServer => "start_server",
    StopServer => "stop_server",
    ServerStatusUpdate => "server_status_update",
    ServerApiCall => "server_api_call",
    ServerApiResponse => "server_api_response",
    ServerPlayerCount => "server_player_count",
    ListServerMods => "list_server_mods",
    ToggleServerMod => "toggle_server_mod",
    InstallServerMod => "install_server_mod",
    DetectWorkshopDir => "detect_workshop_dir",
    LoadServerSave => "load_server_save",
    GetServerStats => "get_server_stats",
    ServerCreationProgress => "server_creation_progress",
    ImportServer => "import_server",
    // Session persistence
    ReattachSession => "reattach_session",
    EjectSession => "eject_session",
    SessionNotFound => "session_not_found",
    // World options
    GetWorldOption => "get_world_option",
    UpdateWorldOption => "update_world_option",
    ExportPresets => "export_presets",
    SaveEditedSav => "save_edited_sav",
    OpenUrl => "open_url",
}

#[cfg(test)]
mod tests {
    use super::MessageType;

    /// An independent duplicate of the enum table: a typo on either side fails
    /// the tests below rather than silently changing the wire vocabulary.
    const EXPECTED_WIRE_NAMES: &[&str] = &[
        "add_pal",
        "add_dps_pal",
        "add_gps_pal",
        "clone_pal",
        "clone_dps_pal",
        "clone_gps_pal",
        "delete_pals",
        "delete_dps_pals",
        "delete_gps_pals",
        "get_pal_details",
        "get_pals",
        "get_gps_pals",
        "heal_all_pals",
        "heal_pals",
        "move_pal",
        "get_ups_pals",
        "get_ups_all_filtered_ids",
        "add_ups_pal",
        "update_ups_pal",
        "delete_ups_pals",
        "clone_ups_pal",
        "clone_to_ups",
        "export_ups_pal",
        "clone_gps_pal_to_player",
        "import_to_ups",
        "get_ups_collections",
        "create_ups_collection",
        "update_ups_collection",
        "delete_ups_collection",
        "get_ups_tags",
        "create_ups_tag",
        "update_ups_tag",
        "delete_ups_tag",
        "get_ups_stats",
        "nuke_ups_pals",
        "delete_player",
        "set_technology_data",
        "get_player_summaries",
        "get_guild_summaries",
        "request_player_details",
        "get_player_details_response",
        "request_guild_details",
        "get_guild_details_response",
        "request_gps",
        "get_gps_response",
        "get_active_skills",
        "get_buildings",
        "get_elements",
        "get_exp_data",
        "get_relic_data",
        "get_map_objects",
        "get_bosses",
        "get_relics",
        "get_fast_travel_points",
        "get_effigies",
        "get_guilds",
        "get_items",
        "get_missions",
        "get_pal_summaries",
        "get_passive_skills",
        "get_players",
        "get_technologies",
        "get_ui_common",
        "get_version",
        "get_work_suitability",
        "get_friendship_data",
        "add_preset",
        "delete_preset",
        "get_presets",
        "update_preset",
        "export_preset",
        "import_preset",
        "delete_guild",
        "get_lab_research",
        "update_lab_research",
        "download_save_file",
        "loaded_save_files",
        "load_zip_file",
        "no_file_selected",
        "save_modded_save",
        "select_gamepass_save",
        "select_save",
        "update_save_file",
        "rename_world",
        "unlock_map",
        "get_settings",
        "update_settings",
        "nuke_presets",
        "error",
        "progress_message",
        "sync_app_state",
        "warning",
        "open_in_browser",
        "get_guild_raw_data",
        "get_raw_data",
        "open_folder",
        "convert_sav_file",
        "convert_save_format",
        "scan_gamepass_saves",
        "delete_gamepass_save",
        "delete_gamepass_player",
        "rename_gamepass_world",
        "convert_steam_id",
        "swap_player_uids",
        "load_source_save",
        "get_source_players",
        "transfer_player",
        "unload_source_save",
        "list_servers",
        "get_server",
        "create_server",
        "update_server",
        "delete_server",
        "start_server",
        "stop_server",
        "server_status_update",
        "server_api_call",
        "server_api_response",
        "server_player_count",
        "list_server_mods",
        "toggle_server_mod",
        "install_server_mod",
        "detect_workshop_dir",
        "load_server_save",
        "get_server_stats",
        "server_creation_progress",
        "import_server",
    ];

    /// Session-persistence and world-option types, which sit after the other
    /// 127 in declaration order.
    const FEATURE_ADDITION_WIRE_NAMES: &[&str] = &[
        "reattach_session",
        "eject_session",
        "session_not_found",
        "get_world_option",
        "update_world_option",
        "export_presets",
        "save_edited_sav",
        "open_url",
    ];

    #[test]
    fn exactly_127_message_types() {
        assert_eq!(EXPECTED_WIRE_NAMES.len(), 127);
        assert_eq!(
            MessageType::ALL.len(),
            EXPECTED_WIRE_NAMES.len() + FEATURE_ADDITION_WIRE_NAMES.len()
        );
    }

    #[test]
    fn wire_names_match_python_enum_exactly() {
        let actual: Vec<&str> = MessageType::ALL.iter().map(|t| t.as_wire()).collect();
        let expected: Vec<&str> = EXPECTED_WIRE_NAMES
            .iter()
            .chain(FEATURE_ADDITION_WIRE_NAMES)
            .copied()
            .collect();
        assert_eq!(actual, expected);
    }

    #[test]
    fn world_option_message_types_round_trip_wire_names() {
        assert_eq!(MessageType::GetWorldOption.as_wire(), "get_world_option");
        assert_eq!(
            MessageType::UpdateWorldOption.as_wire(),
            "update_world_option"
        );
        assert_eq!(
            MessageType::from_wire("get_world_option"),
            Some(MessageType::GetWorldOption)
        );
        assert_eq!(
            MessageType::from_wire("update_world_option"),
            Some(MessageType::UpdateWorldOption)
        );
    }

    #[test]
    fn open_url_message_round_trips_wire_name() {
        assert_eq!(MessageType::OpenUrl.as_wire(), "open_url");
        assert_eq!(MessageType::from_wire("open_url"), Some(MessageType::OpenUrl));
    }

    #[test]
    fn serde_agrees_with_as_wire_for_every_variant() {
        for message_type in MessageType::ALL {
            let serialized = serde_json::to_value(message_type).unwrap();
            assert_eq!(
                serialized,
                serde_json::Value::String(message_type.as_wire().into())
            );
            let deserialized: MessageType = serde_json::from_value(serialized).unwrap();
            assert_eq!(&deserialized, message_type);
            assert_eq!(
                MessageType::from_wire(message_type.as_wire()),
                Some(*message_type)
            );
        }
        assert_eq!(MessageType::from_wire("not_a_real_type"), None);
    }
}
