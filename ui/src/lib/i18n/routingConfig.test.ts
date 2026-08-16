import { describe, expect, it } from 'vitest';
import {
	LOCALIZED_PATHS,
	SITE_ORIGIN,
	hrefLanguageTags,
	isLocalizedPath,
	localeSlugs,
	localizedPath,
	paraglideUrlPatterns,
	siteLocales
} from './routingConfig.js';

describe('routingConfig', () => {
	it('uses the apex domain as canonical origin', () => {
		expect(SITE_ORIGIN).toBe('https://palworldsavepal.app');
	});

	it('covers exactly the 16 project locales', () => {
		expect(siteLocales).toHaveLength(16);
		expect(siteLocales).toContain('en');
		expect(siteLocales).toContain('zh-hans');
	});

	it('leaves English unprefixed so current URLs stay valid', () => {
		expect(localizedPath('/', 'en')).toBe('/');
		expect(localizedPath('/map', 'en')).toBe('/map');
	});

	it('prefixes non-English locales', () => {
		expect(localizedPath('/', 'fr')).toBe('/fr');
		expect(localizedPath('/map', 'fr')).toBe('/fr/map');
	});

	it('serves zh-hans at the short /zh slug', () => {
		expect(localeSlugs['zh-hans']).toBe('zh');
		expect(localizedPath('/wiki', 'zh-hans')).toBe('/zh/wiki');
	});

	it('normalizes stray slashes', () => {
		expect(localizedPath('map/', 'de')).toBe('/de/map');
		expect(localizedPath('/map/', 'de')).toBe('/de/map');
	});

	it('maps locales to valid BCP-47 hreflang tags', () => {
		expect(hrefLanguageTags['pt-br']).toBe('pt-BR');
		expect(hrefLanguageTags['zh-hant']).toBe('zh-Hant');
		expect(hrefLanguageTags['id-id']).toBe('id-ID');
	});

	it('produces unique paths per locale for every localized path', () => {
		for (const pathname of LOCALIZED_PATHS) {
			const paths = siteLocales.map((locale) => localizedPath(pathname, locale));
			expect(new Set(paths).size).toBe(siteLocales.length);
		}
	});

	it('recognizes only the hub set as localized', () => {
		expect(isLocalizedPath('/')).toBe(true);
		expect(isLocalizedPath('/breeding')).toBe(true);
		expect(isLocalizedPath('/wiki/pals/lamball')).toBe(false);
	});

	it('emits one paraglide pattern per localized path', () => {
		expect(paraglideUrlPatterns).toHaveLength(LOCALIZED_PATHS.length);
		for (const entry of paraglideUrlPatterns) {
			expect(entry.localized).toHaveLength(siteLocales.length);
		}
	});
});
