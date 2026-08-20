import { goto } from '$app/navigation';
import { UpdateAvailableModal } from '$components/modals';
import * as m from '$i18n/messages';
import { extractLocaleFromCookie, getLocale, setLocale } from '$i18n/runtime';
import { send } from '$lib/utils/websocketUtils';
import { getAppState, getModalState, getToastState } from '$states';
import { bumpLocaleVersion } from '$states/localeState.svelte';
import { MessageType, type AppSettings, type SupportedLanguage } from '$types';
import { isUpdateAvailableOnGitHub } from '$utils/appVersion';
import { reconcileSettingsLocale } from '$utils/localeReconcile';
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
		const settings = data as AppSettings;
		const language = reconcileSettingsLocale(settings.language, {
			storedLocale: () => extractLocaleFromCookie(),
			getLocale: () => getLocale(),
			setLocale: (next, opts) => setLocale(next as SupportedLanguage, opts),
			bump: bumpLocaleVersion,
			persist: (next) => send(MessageType.UPDATE_SETTINGS, { ...settings, language: next })
		});
		appState.settings = { ...settings, language: language as SupportedLanguage };
	}
};

export const appStateHandlers = [
	getVersionHandler,
	progressMessageHandler,
	errorHandler,
	warningHandler,
	settingsHandler
];
