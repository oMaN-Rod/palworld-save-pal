import type { Pal, Player } from './game';

export enum MessageType {
	ADD_PAL = 'add_pal',
	CLONE_PAL = 'clone_pal',
	MOVE_PAL = 'move_pal',
	DELETE_PALS = 'delete_pals',
	HEAL_PALS = 'heal_pals',
	DOWNLOAD_SAVE_FILE = 'download_save_file',
	ERROR = 'error',
	WARNING = 'warning',
	GET_PLAYERS = 'get_players',
	GET_PAL_DETAILS = 'get_pal_details',
	LOAD_SAVE_FILE = 'load_save_file',
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
	GET_ELEMENTS = 'get_elements',
	GET_ITEMS = 'get_items',
	GET_PALS = 'get_pals',
	OPEN_IN_BROWSER = 'open_in_browser',
	GET_EXP_DATA = 'get_exp_data'
}

interface UpdateSaveFileData {
	modifiedPals: Record<string, Pal>;
	modifiedPlayers: Record<string, Player>;
}

export interface Message {
	type: MessageType;
	data?: any | UpdateSaveFileData;
}
