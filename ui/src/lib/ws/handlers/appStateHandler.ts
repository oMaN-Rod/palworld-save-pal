import { goto } from '$app/navigation';
import { UpdateAvailableModal } from '$components/modals';
import * as m from '$i18n/messages';
import { getLocale, setLocale } from '$i18n/runtime';
import { getAppState, getModalState, getToastState } from '$states';
import { bumpLocaleVersion } from '$states/localeState.svelte';
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

/** Non-fatal counterpart to errorHandler: surfaces the problem without
 *  navigating away from an app that still works. */
export const warningHandler: WSMessageHandler = {
	type: MessageType.WARNING,
	async handle(data) {
		const raw = typeof data === 'string' ? data : (data as { message?: unknown })?.message;
		if (!raw) return;
		getToastState().add(String(raw), m.warning(), 'warning');
	}
};

export const settingsHandler: WSMessageHandler = {
	type: MessageType.GET_SETTINGS,
	async handle(data) {
		const appState = getAppState();
		const previous = getLocale();
		appState.settings = data;
		setLocale(appState.settings.language);
		if (appState.settings.language !== previous) bumpLocaleVersion();
	}
};

export const appStateHandlers = [
	getVersionHandler,
	progressMessageHandler,
	errorHandler,
	warningHandler,
	settingsHandler
];
