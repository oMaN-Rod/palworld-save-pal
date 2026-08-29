export const SITE_ORIGIN = 'https://palworldsavepal.app';

/**
 * @typedef {'en'|'es'|'de'|'es-mx'|'fr'|'id-id'|'it'|'ko'|'pl'|'pt-br'|'ru'|'th'|'tr'|'vi'|'zh-hans'|'zh-hant'} SiteLocale
 */

/**
 * Mirrors psp-ui/project.inlang/settings.json. Keep the two in sync.
 * @type {readonly SiteLocale[]}
 */
export const siteLocales = Object.freeze([
	'en',
	'es',
	'de',
	'es-mx',
	'fr',
	'id-id',
	'it',
	'ko',
	'pl',
	'pt-br',
	'ru',
	'th',
	'tr',
	'vi',
	'zh-hans',
	'zh-hant'
]);

/**
 * URL segment per locale. English is empty so existing URLs never change.
 * @type {Readonly<Record<SiteLocale, string>>}
 */
export const localeSlugs = Object.freeze({
	en: '',
	es: 'es',
	de: 'de',
	'es-mx': 'es-mx',
	fr: 'fr',
	'id-id': 'id-id',
	it: 'it',
	ko: 'ko',
	pl: 'pl',
	'pt-br': 'pt-br',
	ru: 'ru',
	th: 'th',
	tr: 'tr',
	vi: 'vi',
	'zh-hans': 'zh',
	'zh-hant': 'zh-hant'
});

/**
 * BCP-47 tags for <html lang> and rel=alternate hreflang.
 * @type {Readonly<Record<SiteLocale, string>>}
 */
export const htmlLanguageTags = Object.freeze({
	en: 'en',
	es: 'es',
	de: 'de',
	'es-mx': 'es-MX',
	fr: 'fr',
	'id-id': 'id-ID',
	it: 'it',
	ko: 'ko',
	pl: 'pl',
	'pt-br': 'pt-BR',
	ru: 'ru',
	th: 'th',
	tr: 'tr',
	vi: 'vi',
	'zh-hans': 'zh-Hans',
	'zh-hant': 'zh-Hant'
});

export const hrefLanguageTags = htmlLanguageTags;

/**
 * Paths that get locale prefixes and full hreflang alternates. Wiki entity
 * pages are deliberately absent: 5,013 x 15 locales would add 75,195 files and
 * break the Cloudflare static-asset limit.
 */
export const LOCALIZED_PATHS = Object.freeze(['/', '/map', '/wiki', '/breeding', '/about']);

/**
 * @param {string} pathname
 * @param {SiteLocale} locale
 * @returns {string}
 */
export function localizedPath(pathname, locale) {
	const trimmed = pathname.replace(/^\/+|\/+$/g, '');
	const normalized = trimmed === '' ? '/' : `/${trimmed}`;
	const slug = localeSlugs[locale];
	if (!slug) return normalized;
	return normalized === '/' ? `/${slug}` : `/${slug}${normalized}`;
}

/**
 * @param {string} pathname
 * @returns {boolean}
 */
export function isLocalizedPath(pathname) {
	return LOCALIZED_PATHS.includes(pathname);
}

const ORIGIN_PATTERN = ':protocol://:domain(.*)::port?';

/**
 * @param {string} pathPattern
 * @returns {{ pattern: string, localized: Array<[SiteLocale, string]> }}
 */
function routePattern(pathPattern) {
	return {
		pattern: `${ORIGIN_PATTERN}${pathPattern}`,
		localized: siteLocales.map((locale) => [
			locale,
			`${ORIGIN_PATTERN}${localizedPath(pathPattern, locale)}`
		])
	};
}

/** Only the hub set uses URL locale detection; editor routes stay unprefixed. */
export const paraglideUrlPatterns = LOCALIZED_PATHS.map(routePattern);
