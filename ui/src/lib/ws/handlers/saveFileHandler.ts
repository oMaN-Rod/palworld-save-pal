import { getAppState, getToastState } from '$states';
import { MessageType } from '$types';
import {
	clearSessionPersistence,
	consumeReattachPending,
	getStoredSelectedPlayerUid,
	setStoredSessionId
} from '$lib/utils/sessionPersistence';
import type { WSMessageHandler } from '../types';

export const noFileSelectedHandler: WSMessageHandler = {
	type: MessageType.NO_FILE_SELECTED,
	async handle(_: string, { goto }) {
		const toast = getToastState();
		toast.add('No file was selected', 'Warning', 'warning');
		await goto('/file');
	}
};

export const loadedSaveFilesHandler: WSMessageHandler = {
	type: MessageType.LOADED_SAVE_FILES,
	async handle(data) {
		const appState = getAppState();
		const { level, players, world_name, type, has_gps, session_id, size, world_option_present } =
			data;
		console.log('Loaded save files', level, players, 'has_gps:', has_gps);
		appState.resetState();
		appState.saveFile = {
			name: level,
			world_name,
			type,
			size,
			world_option_present: world_option_present ?? false
		};
		appState.playerSaveFiles = players.map((p: any) => ({ name: p }));
		appState.hasGpsAvailable = has_gps ?? false;

		if (session_id) {
			setStoredSessionId(session_id);
		}

		// This overview came from a reattach — re-select the player the user had
		// open before the refresh.
		if (consumeReattachPending()) {
			const storedPlayerUid = getStoredSelectedPlayerUid();
			if (storedPlayerUid) {
				appState.selectPlayerLazy(storedPlayerUid, 'reattach');
			}
		}
	}
};

export const sessionNotFoundHandler: WSMessageHandler = {
	type: MessageType.SESSION_NOT_FOUND,
	async handle(_, { goto }) {
		clearSessionPersistence();
		await goto('/file');
	}
};

export const saveModdedSaveHandler: WSMessageHandler = {
	type: MessageType.SAVE_MODDED_SAVE,
	async handle(data, { goto }) {
		const toast = getToastState();
		toast.add(data, 'Saved!', 'success');
		await goto('/file');
	}
};

export const downloadSaveFileHandler: WSMessageHandler = {
	type: MessageType.DOWNLOAD_SAVE_FILE,
	async handle(data, { goto }) {
		console.log('Download save files', data);
		const files = data as Array<{ name: string; content: string }>;

		for (const { name, content } of files) {
			const binaryString = atob(content);
			const bytes = new Uint8Array(binaryString.length);
			for (let i = 0; i < binaryString.length; i++) {
				bytes[i] = binaryString.charCodeAt(i);
			}

			const blob = new Blob([bytes], { type: 'application/octet-stream' });
			const url = URL.createObjectURL(blob);
			const a = document.createElement('a');
			a.href = url;
			a.download = name;
			a.click();
			URL.revokeObjectURL(url);
		}

		await goto('/file');
	}
};

export const updateSaveFileHandler: WSMessageHandler = {
	type: MessageType.UPDATE_SAVE_FILE,
	async handle(data) {
		const appState = getAppState();
		console.log('Save file updated', data);
		await new Promise((resolve) => setTimeout(resolve, 500));
		appState.autoSave = false;
	}
};

export const selectGamepassSaveHandler: WSMessageHandler = {
	type: MessageType.SELECT_GAMEPASS_SAVE,
	async handle(data, { goto }) {
		const appState = getAppState();
		appState.resetState();
		appState.gamepassSaves = data;
		await goto('/file');
	}
};

export const saveFileHandlers = [
	loadedSaveFilesHandler,
	saveModdedSaveHandler,
	downloadSaveFileHandler,
	updateSaveFileHandler,
	noFileSelectedHandler,
	selectGamepassSaveHandler,
	sessionNotFoundHandler
];
