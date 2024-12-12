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

export const appStateHandlers = [getVersionHandler, progressMessageHandler];
