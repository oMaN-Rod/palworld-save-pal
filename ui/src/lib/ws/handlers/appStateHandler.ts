import { goto } from '$app/navigation';
import UpdateAvailableModal from '$components/modals/update-available/UpdateAvailableModal.svelte';
import { getAppState, getModalState } from '$states';
import { MessageType } from '$types';
import { isUpdateAvailableOnGitHub } from '$utils/appVersion';
import type { WSMessageHandler } from '../types';

const appState = getAppState();
const modal = getModalState();

export const progressMessageHandler: WSMessageHandler = {
	type: MessageType.PROGRESS_MESSAGE,
	async handle(data) {
		appState.progressMessage = data;
	}
};

export const getVersionHandler: WSMessageHandler = {
	type: MessageType.GET_VERSION,
	async handle(data) {
		appState.version = data;

		// Check for updates
		const isUpdateAvailable = await isUpdateAvailableOnGitHub(data);
		if (isUpdateAvailable) {
			// @ts-ignore-next-line
			const result = await modal.showModal<string>(UpdateAvailableModal, {});
		}
	}
};

export const errorHandler: WSMessageHandler = {
	type: MessageType.ERROR,
	async handle(data) {
		const errorMessage = data as { message: string; trace: string };
		goto('/error', {
			state: {
				message: errorMessage.message,
				trace: errorMessage.trace
			}
		});
	}
};

export const settingsHandler: WSMessageHandler = {
	type: MessageType.GET_SETTINGS,
	async handle(data) {
		appState.settings = data;
	}
};

export const appStateHandlers = [
	getVersionHandler,
	progressMessageHandler,
	errorHandler,
	settingsHandler
];
