export enum MessageType {
	GET_PLAYERS = 'get_players',
	GET_PAL_DETAILS = 'get_pal_details',
	DOWNLOAD_SAVE_FILE = 'download_save_file',
	LOAD_SAVE_FILE = 'load_save_file',
	UPDATE_SAVE_FILE = 'update_save_file',
	PROGRESS_MESSAGE = 'progress_message',
	ERROR = 'error',
	SYNC_APP_STATE = "sync_app_state",
}

export interface Message {
	type: MessageType;
	data?: any;
}
