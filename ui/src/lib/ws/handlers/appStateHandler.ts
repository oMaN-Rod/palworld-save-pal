import { goto } from '$app/navigation';
import { getAppState } from '$states';
import { MessageType } from '$types';
import type { WSMessageHandler } from '../types';

const appState = getAppState();

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
