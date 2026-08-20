import { getLocale, setLocale } from '$i18n/runtime';
import { applyEditedSettings, switchLocale } from '$lib/utils/localeSwitch';
import { send } from '$lib/utils/websocketUtils';
import type { SupportedLanguage } from '$types';
import { MessageType } from '$types';
import { getAppState } from './appState.svelte';

let version = $state(0);

// Paraglide message accessors read module-scoped state, so a locale change does
// not re-render on its own. The layout keys on this counter to force it.
export const localeState = {
	get version(): number {
		return version;
	}
};

export function bumpLocaleVersion(): void {
	version += 1;
}

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
