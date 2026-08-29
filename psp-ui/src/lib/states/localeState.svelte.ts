import { getLocale, setLocale } from '$i18n/runtime';
import { bumpLocaleVersion, localeState } from '$lib/i18n/localeVersion.svelte';
import { applyEditedSettings, switchLocale } from '$lib/utils/localeSwitch';
import { send } from '$lib/utils/websocketUtils';
import type { SupportedLanguage } from '$types';
import { MessageType } from '$types';
import { getAppState } from './appState.svelte';

export { bumpLocaleVersion, localeState };

function localeDeps() {
	const appState = getAppState();
	return {
		getLocale: () => getLocale(),
		setLocale: (next: string, opts: { reload: boolean }) =>
			setLocale(next as SupportedLanguage, opts),
		bump: bumpLocaleVersion,
		persist: (next: string) => {
			appState.settings.language = next as SupportedLanguage;
			send(MessageType.UPDATE_SETTINGS, { ...appState.settings });
		},
		persistAll: () => send(MessageType.UPDATE_SETTINGS, { ...appState.settings })
	};
}

export function applyLocale(code: SupportedLanguage): boolean {
	return switchLocale(code, localeDeps());
}

export function applySettings(): void {
	const appState = getAppState();
	applyEditedSettings(appState.settings.language, localeDeps());
}
