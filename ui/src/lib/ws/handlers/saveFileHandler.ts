import { getAppState, getToastState } from '$states';
import { MessageType } from '$types';
import type { WSMessageHandler } from '../types';

const appState = getAppState();

export const loadedSaveFilesHandler: WSMessageHandler = {
	type: MessageType.LOADED_SAVE_FILES,
	async handle(data) {
		const { sav_file_name, players, world_name } = data;
		console.log('Loaded save files', sav_file_name, players);
		appState.resetState();
		appState.saveFile = { name: sav_file_name, world_name };
		appState.playerSaveFiles = players.map((p: any) => ({ name: p }));
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

export const loadZipFileHandler: WSMessageHandler = {
	type: MessageType.LOAD_ZIP_FILE,
	async handle(data, { goto }) {
		const file = data as { name: string; size: number };

		appState.resetState();
		appState.saveFile = file;
		await goto('/edit');
	}
};

export const downloadSaveFileHandler: WSMessageHandler = {
	type: MessageType.DOWNLOAD_SAVE_FILE,
	async handle(data, { goto }) {
		console.log('Download save file', data);
		const { name, content } = data as { name: string; content: string };

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
		await goto('/file');
	}
};

export const updateSaveFileHandler: WSMessageHandler = {
	type: MessageType.UPDATE_SAVE_FILE,
	async handle(data, { goto }) {
		console.log('Save file updated', data);
		await goto('/edit');
	}
};

export const saveFileHandlers = [
	loadedSaveFilesHandler,
	saveModdedSaveHandler,
	loadZipFileHandler,
	downloadSaveFileHandler,
	updateSaveFileHandler
];
