import type { Pal, Player } from './game';

export enum MessageType {
	ADD_PAL = 'add_pal',
	CLONE_PAL = 'clone_pal',
	MOVE_PAL = 'move_pal',
	DELETE_PALS = 'delete_pals',
	HEAL_PALS = 'heal_pals',
	HEAL_ALL_PALS = 'heal_all_pals',
	DOWNLOAD_SAVE_FILE = 'download_save_file',
	ERROR = 'error',
	WARNING = 'warning',
	GET_GUILDS = 'get_guilds',
	GET_PLAYERS = 'get_players',
	GET_PAL_DETAILS = 'get_pal_details',
	LOAD_ZIP_FILE = 'load_zip_file',
	PROGRESS_MESSAGE = 'progress_message',
	SYNC_APP_STATE = 'sync_app_state',
	UPDATE_SAVE_FILE = 'update_save_file',
	GET_PRESETS = 'get_presets',
	ADD_PRESET = 'add_preset',
	UPDATE_PRESET = 'update_preset',
	DELETE_PRESET = 'delete_preset',
	GET_ACTIVE_SKILLS = 'get_active_skills',
	GET_PASSIVE_SKILLS = 'get_passive_skills',
	GET_TECHNOLOGIES = 'get_technologies',
	GET_ELEMENTS = 'get_elements',
	GET_ITEMS = 'get_items',
	GET_PALS = 'get_pals',
	SET_TECHNOLOGY_DATA = 'set_technology_data',
	OPEN_IN_BROWSER = 'open_in_browser',
	GET_EXP_DATA = 'get_exp_data',
	GET_VERSION = 'get_version',
	SELECT_SAVE = 'select_save',
	LOADED_SAVE_FILES = 'loaded_save_files',
	SAVE_MODDED_SAVE = 'save_modded_save',
	GET_SETTINGS = 'get_settings',
	UPDATE_SETTINGS = 'update_settings',
	GET_UI_COMMON = 'get_ui_common',
	NO_FILE_SELECTED = 'no_file_selected',
	SELECT_GAMEPASS_SAVE = 'select_gamepass_save',
	GET_SAVE_TYPE = 'get_save_type',
	GET_WORK_SUITABILITY = 'get_work_suitability',
	GET_BUILDINGS = 'get_buildings',
	GET_RAW_DATA = 'get_raw_data',
	DELETE_GUILD = 'delete_guild',
}

interface UpdateSaveFileData {
	modifiedPals: Record<string, Pal>;
	modifiedPlayers: Record<string, Player>;
}

export interface Message {
	type: MessageType;
	data?: any | UpdateSaveFileData;
}
