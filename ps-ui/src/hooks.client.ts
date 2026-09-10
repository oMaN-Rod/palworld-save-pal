import { installAppLocale } from '$lib/i18n/appLocale';
import type { ClientInit } from '@sveltejs/kit';

// Before the first render: every message accessor asks paraglide for the locale,
// and paraglide's own answer would come from the url and overwrite the stored one.
export const init: ClientInit = () => {
	installAppLocale();
};
