import { getLocale, setLocale } from '$i18n/runtime';
import { send } from '$lib/utils/websocketUtils';
import { switchLocale } from '$lib/utils/localeSwitch';
import { MessageType } from '$types';
import type { SupportedLanguage } from '$types';
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

export function applyLocale(code: SupportedLanguage): boolean {
	const appState = getAppState();
	return switchLocale(code, {
		getLocale: () => getLocale(),
		setLocale: (next, opts) => setLocale(next as SupportedLanguage, opts),
		bump: bumpLocaleVersion,
		persist: (next) => {
			appState.settings.language = next as SupportedLanguage;
			send(MessageType.UPDATE_SETTINGS, { ...appState.settings });
		}
	});
}
