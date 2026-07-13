import { goto } from '$app/navigation';
import { UpdateAvailableModal } from '$components/modals';
import { setLocale } from '$i18n/runtime';
import { dismissedUpdateVersion, getAppState, getModalState } from '$states';
import { MessageType } from '$types';
import { isUpdateAvailableOnGitHub } from '$utils/appVersion';
import type { WSMessageHandler } from '../types';

export const progressMessageHandler: WSMessageHandler = {
	type: MessageType.PROGRESS_MESSAGE,
	async handle(data) {
		const appState = getAppState();
		appState.progressMessage = data;
	}
};

export const getVersionHandler: WSMessageHandler = {
	type: MessageType.GET_VERSION,
	async handle(data) {
		const appState = getAppState();
		const modal = getModalState();
		appState.version = data;

		// Check for updates, but only nag once per release
		const latestVersion = await isUpdateAvailableOnGitHub(data);
		if (latestVersion && latestVersion !== dismissedUpdateVersion.current) {
			dismissedUpdateVersion.current = latestVersion;
			// @ts-ignore-next-line
			await modal.showModal<string>(UpdateAvailableModal, {});
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
		const appState = getAppState();
		appState.settings = data;
		setLocale(appState.settings.language);
	}
};

export const appStateHandlers = [
	getVersionHandler,
	progressMessageHandler,
	errorHandler,
	settingsHandler
];
