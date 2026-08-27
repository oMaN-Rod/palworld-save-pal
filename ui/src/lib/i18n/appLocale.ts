import {
	cookieMaxAge,
	cookieName,
	extractLocaleFromCookie,
	overwriteGetLocale,
	overwriteSetLocale
} from '$i18n/runtime';
import { installLocaleRuntime, type LocaleRuntime, type SiteLocale } from './localeRuntime';
import { bumpLocaleVersion } from './localeVersion.svelte';

let runtime: LocaleRuntime | undefined;

export function installAppLocale(): void {
	if (runtime) return;
	runtime = installLocaleRuntime({
		pathname: () => window.location.pathname,
		storedLocale: () => extractLocaleFromCookie(),
		persistLocale: (code: SiteLocale) => {
			document.cookie = `${cookieName}=${code}; path=/; max-age=${cookieMaxAge}`;
		},
		bump: bumpLocaleVersion,
		setHtmlLang: (tag: string) => {
			document.documentElement.lang = tag;
		},
		overwriteGetLocale,
		overwriteSetLocale
	});
}

export function syncLocaleToPath(pathname: string): void {
	runtime?.syncFromPath(pathname);
}
