import { page } from '$app/state';
import * as m from '$i18n/messages';
import { baseStructuresData } from '$lib/data';
import { getActiveDirectory, takeSaveTarget, writeSaveInPlace } from '$lib/fs';
import {
	clearSessionPersistence,
	consumeReattachPending,
	getStoredSelectedPlayerUid,
	setStoredSessionId
} from '$lib/utils/sessionPersistence';
import { getAppState, getToastState } from '$states';
import { MessageType } from '$types';
import { unzipSync } from 'fflate';
import type { WSMessageHandler } from '../types';

export const noFileSelectedHandler: WSMessageHandler = {
	type: MessageType.NO_FILE_SELECTED,
	async handle(_: string, { goto }) {
		const toast = getToastState();
		toast.add(m.save_no_file_selected(), m.warning(), 'warning');
		await goto('/file');
	}
};

export const loadedSaveFilesHandler: WSMessageHandler = {
	type: MessageType.LOADED_SAVE_FILES,
	async handle(data, { goto }) {
		const appState = getAppState();
		const { level, players, world_name, type, has_gps, session_id, size, world_option_present } =
			data;
		console.log('Loaded save files', level, players, 'has_gps:', has_gps);
		appState.resetState();
		baseStructuresData.reset();
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

		if (!page.url.pathname.startsWith('/bulk')) {
			await goto('/edit');
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
		toast.add(data, m.toast_saved(), 'success');
		await goto('/file');
	}
};

/** `bytes` from the worker; `content` (base64) only from the text-only socket. */
export type SaveOutputFile = { name: string; content?: string; bytes?: Uint8Array };

function fileBytes(file: SaveOutputFile): Uint8Array<ArrayBuffer> {
	if (file.bytes) return file.bytes as Uint8Array<ArrayBuffer>;
	const binary = atob(file.content ?? '');
	const bytes = new Uint8Array(binary.length);
	for (let i = 0; i < binary.length; i++) bytes[i] = binary.charCodeAt(i);
	return bytes;
}

export async function handleSaveOutput(
	files: SaveOutputFile[],
	download: (name: string, bytes: Uint8Array<ArrayBuffer>) => void,
	now: number
): Promise<'folder' | 'download'> {
	if (takeSaveTarget() === 'folder') {
		const { handle, writable } = getActiveDirectory();
		if (handle && writable && files.length > 0) {
			const unzipped = unzipSync(fileBytes(files[0]));
			const out = Object.entries(unzipped).map(([path, bytes]) => ({ path, bytes }));
			await writeSaveInPlace(handle, out, now);
			return 'folder';
		}
	}
	for (const file of files) download(file.name, fileBytes(file));
	return 'download';
}

export const downloadSaveFileHandler: WSMessageHandler = {
	type: MessageType.DOWNLOAD_SAVE_FILE,
	async handle(data, { goto }) {
		const files = data as SaveOutputFile[];
		const toast = getToastState();
		const download = (name: string, bytes: Uint8Array<ArrayBuffer>) => {
			const blob = new Blob([bytes], { type: 'application/octet-stream' });
			const url = URL.createObjectURL(blob);
			const a = document.createElement('a');
			a.href = url;
			a.download = name;
			a.click();
			URL.revokeObjectURL(url);
		};
		const mode = await handleSaveOutput(files, download, Date.now());
		if (mode === 'folder') {
			toast.add(m.save_saved_to_folder(), m.toast_saved(), 'success');
		} else {
			await goto('/file');
		}
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
		baseStructuresData.reset();
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
