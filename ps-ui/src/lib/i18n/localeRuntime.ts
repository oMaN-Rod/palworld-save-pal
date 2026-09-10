import { htmlLanguageTags, localeSlugs, siteLocales } from './routingConfig.js';

export type SiteLocale = (typeof siteLocales)[number];

export type LocaleRuntimeDeps = {
	storedLocale: () => string | undefined;
	persistLocale: (code: SiteLocale) => void;
	bump: () => void;
	setHtmlLang: (tag: string) => void;
};

const localeBySlug = new Map(
	siteLocales.filter((locale) => localeSlugs[locale] !== '').map((l) => [localeSlugs[l], l])
);

function isSiteLocale(value: string | undefined): value is SiteLocale {
	return value !== undefined && (siteLocales as readonly string[]).includes(value);
}

/**
 * The locale a path states outright. English has an empty slug, so an
 * unprefixed path — `/`, `/edit`, `/debug` — states nothing and returns
 * undefined rather than `en`.
 */
export function localePrefixOf(pathname: string): SiteLocale | undefined {
	return localeBySlug.get(pathname.split('/')[1] ?? '');
}

export function resolveInitialLocale(input: {
	pathname: string;
	stored: string | undefined;
}): SiteLocale {
	const fromUrl = localePrefixOf(input.pathname);
	if (fromUrl) return fromUrl;
	if (isSiteLocale(input.stored)) return input.stored;
	return 'en';
}

/**
 * Owns the active locale outright so paraglide never resolves one itself.
 * Its own resolution re-derives the locale per call and writes the answer back
 * to the cookie, which on a locale-prefixed hub URL silently overwrites the
 * language the user picked. Reading here is pure; only an explicit switch or an
 * explicitly locale-prefixed URL persists anything.
 */
export function createLocaleRuntime(initial: SiteLocale, deps: LocaleRuntimeDeps) {
	let current: SiteLocale = initial;

	function assign(next: SiteLocale): void {
		if (next === current) return;
		current = next;
		deps.persistLocale(next);
		deps.setHtmlLang(htmlLanguageTags[next]);
		deps.bump();
	}

	return {
		getLocale: (): SiteLocale => current,
		setLocale: (code: string): void => {
			if (!isSiteLocale(code)) throw new Error(`Unsupported locale: ${code}`);
			assign(code);
		},
		syncFromPath: (pathname: string): void => {
			const fromUrl = localePrefixOf(pathname);
			if (fromUrl) assign(fromUrl);
		}
	};
}

export type LocaleRuntime = ReturnType<typeof createLocaleRuntime>;

export type LocaleInstallDeps = LocaleRuntimeDeps & {
	pathname: () => string;
	overwriteGetLocale: (fn: () => SiteLocale) => void;
	overwriteSetLocale: (fn: (code: SiteLocale) => void) => void;
};

export function installLocaleRuntime(deps: LocaleInstallDeps): LocaleRuntime {
	const pathname = deps.pathname();
	const stored = deps.storedLocale();
	const initial = resolveInitialLocale({ pathname, stored });
	const runtime = createLocaleRuntime(initial, deps);

	// A locale-prefixed link is a choice too, so it sticks past the hub pages.
	// An unprefixed url states nothing and must leave the preference untouched.
	if (localePrefixOf(pathname) && initial !== stored) deps.persistLocale(initial);

	// Prerendered pages bake the lang of the url they were built for, not of the
	// preference this visitor arrives with.
	deps.setHtmlLang(htmlLanguageTags[initial]);

	deps.overwriteGetLocale(runtime.getLocale);
	deps.overwriteSetLocale(runtime.setLocale);
	return runtime;
}
