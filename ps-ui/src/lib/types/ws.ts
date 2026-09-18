import type { Pal, Player } from './game';

export enum MessageType {
	ADD_PAL = 'add_pal',
	ADD_DPS_PAL = 'add_dps_pal',
	CLONE_PAL = 'clone_pal',
	CLONE_DPS_PAL = 'clone_dps_pal',
	MOVE_PAL = 'move_pal',
	DELETE_PALS = 'delete_pals',
	DELETE_DPS_PALS = 'delete_dps_pals',
	HEAL_PALS = 'heal_pals',
	HEAL_ALL_PALS = 'heal_all_pals',
	DOWNLOAD_SAVE_FILE = 'download_save_file',
	ERROR = 'error',
	WARNING = 'warning',
	GET_GUILDS = 'get_guilds',
	GET_PLAYERS = 'get_players',
	LOAD_ZIP_FILE = 'load_zip_file',
	PROGRESS_MESSAGE = 'progress_message',
	SYNC_APP_STATE = 'sync_app_state',
	UPDATE_SAVE_FILE = 'update_save_file',
	GET_PRESETS = 'get_presets',
	ADD_PRESET = 'add_preset',
	UPDATE_PRESET = 'update_preset',
	DELETE_PRESET = 'delete_preset',
	GET_ACTIVE_SKILLS = 'get_active_skills',
	GET_FRIENDSHIP_DATA = 'get_friendship_data',
	GET_PASSIVE_SKILLS = 'get_passive_skills',
	GET_TECHNOLOGIES = 'get_technologies',
	GET_ELEMENTS = 'get_elements',
	GET_ITEMS = 'get_items',
	GET_PALS = 'get_pals',
	SET_TECHNOLOGY_DATA = 'set_technology_data',
	OPEN_IN_BROWSER = 'open_in_browser',
	OPEN_URL = 'open_url',
	GET_EXP_DATA = 'get_exp_data',
	GET_RELIC_DATA = 'get_relic_data',
	GET_VERSION = 'get_version',
	SELECT_SAVE = 'select_save',
	LOADED_SAVE_FILES = 'loaded_save_files',
	REATTACH_SESSION = 'reattach_session',
	EJECT_SESSION = 'eject_session',
	SESSION_NOT_FOUND = 'session_not_found',
	SAVE_MODDED_SAVE = 'save_modded_save',
	SAVE_EDITED_SAV = 'save_edited_sav',
	GET_SETTINGS = 'get_settings',
	UPDATE_SETTINGS = 'update_settings',
	GET_UI_COMMON = 'get_ui_common',
	NO_FILE_SELECTED = 'no_file_selected',
	SELECT_GAMEPASS_SAVE = 'select_gamepass_save',
	GET_WORK_SUITABILITY = 'get_work_suitability',
	GET_BUILDINGS = 'get_buildings',
	GET_MAP_OBJECT_FOOTPRINTS = 'get_map_object_footprints',
	GET_BASE_STRUCTURES = 'get_base_structures',
	GET_RAW_DATA = 'get_raw_data',
	GET_DUNGEONS = 'get_dungeons',
	GET_BOSSES = 'get_bosses',
	GET_FAST_TRAVEL_POINTS = 'get_fast_travel_points',
	GET_RELICS = 'get_relics',
	GET_MAP_LAYER = 'get_map_layer',
	DELETE_GUILD = 'delete_guild',
	DELETE_BASE = 'delete_base',
	DELETE_PLAYER = 'delete_player',
	NUKE_PRESETS = 'nuke_presets',
	EXPORT_PRESET = 'export_preset',
	IMPORT_PRESET = 'import_preset',
	EXPORT_PRESETS = 'export_presets',
	CAPTURE_BASE_BLUEPRINT = 'capture_base_blueprint',
	STORE_BLUEPRINT = 'store_blueprint',
	LIST_BLUEPRINTS = 'list_blueprints',
	LOAD_BLUEPRINT = 'load_blueprint',
	EXPORT_BLUEPRINT_FILE = 'export_blueprint_file',
	VALIDATE_BLUEPRINT_PLACEMENT = 'validate_blueprint_placement',
	PLACE_BLUEPRINT = 'place_blueprint',
	REQUEST_BLUEPRINT_GEOMETRY = 'request_blueprint_geometry',
	DELETE_BLUEPRINT = 'delete_blueprint',
	GET_LAB_RESEARCH = 'get_lab_research',
	UPDATE_LAB_RESEARCH = 'update_lab_research',
	RENAME_WORLD = 'rename_world',
	ADD_GPS_PAL = 'add_gps_pal',
	CLONE_GPS_PAL = 'clone_gps_pal',
	DELETE_GPS_PALS = 'delete_gps_pals',

	GET_UPS_PALS = 'get_ups_pals',
	GET_UPS_ALL_FILTERED_IDS = 'get_ups_all_filtered_ids',
	ADD_UPS_PAL = 'add_ups_pal',
	UPDATE_UPS_PAL = 'update_ups_pal',
	DELETE_UPS_PALS = 'delete_ups_pals',
	CLONE_UPS_PAL = 'clone_ups_pal',
	CLONE_TO_UPS = 'clone_to_ups',
	EXPORT_UPS_PAL = 'export_ups_pal',
	CLONE_GPS_PAL_TO_PLAYER = 'clone_gps_pal_to_player',
	IMPORT_TO_UPS = 'import_to_ups',
	GET_UPS_COLLECTIONS = 'get_ups_collections',
	CREATE_UPS_COLLECTION = 'create_ups_collection',
	UPDATE_UPS_COLLECTION = 'update_ups_collection',
	DELETE_UPS_COLLECTION = 'delete_ups_collection',
	GET_UPS_TAGS = 'get_ups_tags',
	CREATE_UPS_TAG = 'create_ups_tag',
	UPDATE_UPS_TAG = 'update_ups_tag',
	DELETE_UPS_TAG = 'delete_ups_tag',
	GET_UPS_STATS = 'get_ups_stats',
	NUKE_UPS_PALS = 'nuke_ups_pals',

	UNLOCK_MAP = 'unlock_map',
	GET_PLAYER_SUMMARIES = 'get_player_summaries',
	GET_GUILD_SUMMARIES = 'get_guild_summaries',
	GET_PAL_SUMMARIES = 'get_pal_summaries',
	GET_OVERVIEW_STATS = 'get_overview_stats',
	EXPORT_OVERVIEW_STATS = 'export_overview_stats',
	REQUEST_PLAYER_DETAILS = 'request_player_details',
	GET_PLAYER_DETAILS_RESPONSE = 'get_player_details_response',
	REQUEST_GUILD_DETAILS = 'request_guild_details',
	GET_GUILD_DETAILS_RESPONSE = 'get_guild_details_response',
	REQUEST_GPS = 'request_gps',
	GET_GPS_RESPONSE = 'get_gps_response',
	GET_MISSIONS = 'get_missions',

	OPEN_FOLDER = 'open_folder',
	CONVERT_SAV_FILE = 'convert_sav_file',
	CONVERT_SAVE_FORMAT = 'convert_save_format',
	SCAN_GAMEPASS_SAVES = 'scan_gamepass_saves',
	DELETE_GAMEPASS_SAVE = 'delete_gamepass_save',
	DELETE_GAMEPASS_PLAYER = 'delete_gamepass_player',
	RENAME_GAMEPASS_WORLD = 'rename_gamepass_world',

	CONVERT_STEAM_ID = 'convert_steam_id',
	SWAP_PLAYER_UIDS = 'swap_player_uids',
	LOAD_SOURCE_SAVE = 'load_source_save',
	GET_SOURCE_PLAYERS = 'get_source_players',
	TRANSFER_PLAYER = 'transfer_player',
	UNLOAD_SOURCE_SAVE = 'unload_source_save',

	LIST_SERVERS = 'list_servers',
	GET_SERVER = 'get_server',
	CREATE_SERVER = 'create_server',
	UPDATE_SERVER = 'update_server',
	DELETE_SERVER = 'delete_server',
	START_SERVER = 'start_server',
	STOP_SERVER = 'stop_server',
	SERVER_STATUS_UPDATE = 'server_status_update',
	SERVER_API_CALL = 'server_api_call',
	SERVER_API_RESPONSE = 'server_api_response',
	DETECT_WORKSHOP_DIR = 'detect_workshop_dir',
	LOAD_SERVER_SAVE = 'load_server_save',
	GET_SERVER_STATS = 'get_server_stats',
	SERVER_CREATION_PROGRESS = 'server_creation_progress',
	IMPORT_SERVER = 'import_server',

	GET_WORLD_OPTION = 'get_world_option',
	UPDATE_WORLD_OPTION = 'update_world_option',

	GET_BREEDING_PALS = 'get_breeding_pals',
	BREEDING_DIRECT_CHILD = 'breeding_direct_child',
	BREEDING_DIRECT_PARTNERS = 'breeding_direct_partners',
	BREEDING_DIRECT_PARENTS = 'breeding_direct_parents',
	BREEDING_CHAIN = 'breeding_chain',

	// Plugins
	LIST_PLUGINS = 'list_plugins',
	GET_PLUGIN = 'get_plugin',
	INSTALL_PLUGIN = 'install_plugin',
	EXPORT_PLUGIN = 'export_plugin',
	CLONE_PLUGIN = 'clone_plugin',
	UNINSTALL_PLUGIN = 'uninstall_plugin',
	SET_PLUGIN_ENABLED = 'set_plugin_enabled',
	RUN_PLUGIN_COMMAND = 'run_plugin_command',
	LIST_PLUGIN_ENTITIES = 'list_plugin_entities',
	CANCEL_PLUGIN_RUN = 'cancel_plugin_run',
	PLUGIN_RUN_RESULT = 'plugin_run_result',
	CHECK_PLUGIN_SYNTAX = 'check_plugin_syntax',
	CHECK_PLUGIN_MANIFEST = 'check_plugin_manifest',
	GET_API_DEFINITION = 'get_api_definition',
	CREATE_PLUGIN = 'create_plugin',
	SAVE_PLUGIN_SOURCE = 'save_plugin_source',
	DELETE_PLUGIN_SOURCE = 'delete_plugin_source',
	RUN_PLUGIN_DRAFT = 'run_plugin_draft',
	GET_EDITOR_TIER = 'get_editor_tier',
	LSP_REQUEST = 'lsp_request',
	LSP_NOTIFICATION = 'lsp_notification',
	OPEN_LSP_SESSION = 'open_lsp_session',
	SUBSCRIBE_LIVE = 'subscribe_live',
	LIVE_FRAME = 'live_frame',
	ENSURE_GAMEDATA_LAUNCH_ARG = 'ensure_gamedata_launch_arg',
	SIGNAL_SET_SOURCE = 'signal_set_source',
	SIGNAL_STATUS = 'signal_status',
	SIGNAL_START_PAIRING = 'signal_start_pairing',
	SIGNAL_STOP_PAIRING = 'signal_stop_pairing',
	SIGNAL_SET_ARMED = 'signal_set_armed',
	SIGNAL_LIST_DEVICES = 'signal_list_devices',
	SIGNAL_RENAME_DEVICE = 'signal_rename_device',
	SIGNAL_REVOKE_DEVICE = 'signal_revoke_device',
	SIGNAL_RESET_REMOTE_ACCESS = 'signal_reset_remote_access',
	LIST_LOCAL_SAVES = 'list_local_saves',
	BROWSE_DIRECTORY = 'browse_directory',
	GAME_STATUS = 'game_status',
	GAME_PLAYERS = 'game_players',
	GAME_PALS = 'game_pals',
	GAME_PAL_DETAIL = 'game_pal_detail',
	GAME_INVENTORY = 'game_inventory',
	GAME_GUILD = 'game_guild',
	GAME_GUILDS = 'game_guilds',
	GAME_BASE_PALS = 'game_base_pals',
	GAME_GUILD_CONTAINERS = 'game_guild_containers',
	GAME_EDIT_GUILD = 'game_edit_guild',
	GAME_SET_GUILD_ROLE = 'game_set_guild_role',
	GAME_CAPABILITIES = 'game_capabilities',
	GAME_HEAL_PALS = 'game_heal_pals',
	GAME_SET_ITEM_SLOT = 'game_set_item_slot',
	GAME_REMOVE_PAL = 'game_remove_pal',
	GAME_MOVE_PAL = 'game_move_pal',
	GAME_ADD_PAL = 'game_add_pal',
	GAME_EDIT_PAL = 'game_edit_pal',
	GAME_EDIT_PLAYER = 'game_edit_player',
	GAME_INSTANCES = 'game_instances',
	GAME_ADD_INSTANCE = 'game_add_instance',
	GAME_UPDATE_INSTANCE = 'game_update_instance',
	GAME_DELETE_INSTANCE = 'game_delete_instance',
	GAME_SELECT_INSTANCE = 'game_select_instance',
	GAME_TEST_INSTANCE = 'game_test_instance',
	MOD_TARGET_LIST = 'mod_target_list',
	MOD_TARGET_DETECT = 'mod_target_detect',
	MOD_TARGET_ADD = 'mod_target_add',
	MOD_TARGET_REMOVE = 'mod_target_remove',
	MOD_TARGET_SCAN = 'mod_target_scan',
	MOD_ADOPT = 'mod_adopt',
	MOD_ANALYZE = 'mod_analyze',
	MOD_INSTALL = 'mod_install',
	MOD_LIST = 'mod_list',
	MOD_REMOVE = 'mod_remove',
	MOD_VERSION_SET_CURRENT = 'mod_version_set_current',
	MOD_VERSION_DELETE = 'mod_version_delete',
	MOD_BACKUP_LIST = 'mod_backup_list',
	MOD_BACKUP_RESTORE = 'mod_backup_restore',
	MOD_BACKUP_DELETE = 'mod_backup_delete',
	PROFILE_LIST = 'profile_list',
	PROFILE_SET_MOD = 'profile_set_mod',
	PROFILE_PLAN = 'profile_plan',
	PROFILE_APPLY = 'profile_apply',
	MOD_PROGRESS = 'mod_progress',
	PROFILE_CREATE = 'profile_create',
	PROFILE_RENAME = 'profile_rename',
	PROFILE_DELETE = 'profile_delete',
	PROFILE_ACTIVATE = 'profile_activate',
	PROFILE_REORDER = 'profile_reorder',
	PROFILE_SET_OPTIONS = 'profile_set_options',
	WORLD_PROFILE_SET = 'world_profile_set',
	GAME_LAUNCH = 'game_launch',
	MOD_UPLOAD_BEGIN = 'mod_upload_begin',
	MOD_UPLOAD_CHUNK = 'mod_upload_chunk',
	MOD_UPLOAD_END = 'mod_upload_end',
	PROFILE_EXPORT = 'profile_export',
	PROFILE_IMPORT = 'profile_import',
	FRAMEWORK_STATUS = 'framework_status',
	FRAMEWORK_INSTALL = 'framework_install',
	FRAMEWORK_REMOVE = 'framework_remove',
	FRAMEWORK_HAZARD_REMOVE = 'framework_hazard_remove',
	GAME_INSTANCE_SET_TARGET = 'game_instance_set_target',
	MOD_VERIFICATION_GET = 'mod_verification_get',
	MOD_VERIFICATION_SUBSCRIBE = 'mod_verification_subscribe',
	MOD_VERIFICATION = 'mod_verification',
	MOD_CONFLICTS = 'mod_conflicts',
	MOD_IOSTORE_CONVERT = 'mod_iostore_convert',
	PROFILE_REMOVE_MOD = 'profile_remove_mod',
	MOD_RELEASE_PROFILES = 'mod_release_profiles',
	NEXUS_ACCOUNT_GET = 'nexus_account_get',
	NEXUS_KEY_SET = 'nexus_key_set',
	NEXUS_KEY_CLEAR = 'nexus_key_clear',
	NEXUS_CATEGORIES = 'nexus_categories',
	NEXUS_SEARCH = 'nexus_search',
	NEXUS_MOD_FILES = 'nexus_mod_files',
	NEXUS_DOWNLOAD = 'nexus_download',
	NEXUS_LINK_SUBSCRIBE = 'nexus_link_subscribe',
	NEXUS_LINK = 'nexus_link',
	NEXUS_HANDLER_STATUS = 'nexus_handler_status',
	NEXUS_HANDLER_REGISTER = 'nexus_handler_register',
	MOD_UPDATE_CHECK = 'mod_update_check',
	MOD_UPDATE_IGNORE = 'mod_update_ignore'
}

interface UpdateSaveFileData {
	modifiedPals: Record<string, Pal>;
	modifiedPlayers: Record<string, Player>;
}

export interface Message {
	type: MessageType;
	data?: any | UpdateSaveFileData;
}

export type PluginParamType =
	| 'int'
	| 'float'
	| 'string'
	| 'bool'
	| 'enum'
	| 'entity'
	| 'multiselect';

export interface PluginParam {
	id: string;
	type: PluginParamType;
	label: string;
	description: string | null;
	default: unknown;
	min: number | null;
	max: number | null;
	options: string[];
	entity: string | null;
}

export interface PluginCommand {
	id: string;
	title: string;
	description: string | null;
	destructive: boolean;
	params: PluginParam[];
}

export interface PluginUiWidget {
	type: string;
	id: string | null;
	label: string | null;
	entity: string | null;
	from: string | null;
	path: string | null;
	command: string | null;
	columns: string[];
	selectable: boolean;
	span: string | null;
	args: Record<string, string>;
	text: string | null;
}

export interface PluginUiSection {
	title: string | null;
	columns: number;
	widgets: PluginUiWidget[];
}

export interface PluginEntityOption {
	id: string;
	label: string;
}

export interface PluginEntityOptions {
	options: PluginEntityOption[];
	total: number;
}

export interface PluginSummary {
	id: string;
	name: string;
	version: string;
	author: string | null;
	enabled: boolean;
	bundled: boolean;
	commands: PluginCommand[];
	ui: PluginUiSection[];
	error?: string;
}

export type PluginRunStatus = 'ok' | 'timeout' | 'cancelled' | 'memory_exceeded' | 'error';

export type PluginLogLevel = 'info' | 'warn' | 'error';

export interface PluginLogLine {
	level: PluginLogLevel;
	message: string;
}

export interface PluginRunResult {
	run_id: string;
	status: PluginRunStatus;
	message: string | null;
	summary: string | null;
	counts: Record<string, number>;
	result: unknown;
	log: PluginLogLine[];
}
