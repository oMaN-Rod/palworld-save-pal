import { describe, expect, it } from 'vitest';
import {
	createLocaleRuntime,
	installLocaleRuntime,
	localePrefixOf,
	resolveInitialLocale,
	type LocaleRuntimeDeps,
	type SiteLocale
} from './localeRuntime';

function deps(
	stored: string | undefined,
	calls: string[] = []
): LocaleRuntimeDeps & { calls: string[]; stored: () => string | undefined } {
	let current = stored;
	return {
		calls,
		stored: () => current,
		storedLocale: () => current,
		persistLocale: (code) => {
			current = code;
			calls.push(`persist:${code}`);
		},
		bump: () => {
			calls.push('bump');
		},
		setHtmlLang: (tag) => {
			calls.push(`lang:${tag}`);
		}
	};
}

describe('localePrefixOf', () => {
	it('reads an explicit locale segment', () => {
		expect(localePrefixOf('/de')).toBe('de');
		expect(localePrefixOf('/de/map')).toBe('de');
		expect(localePrefixOf('/pt-br/wiki')).toBe('pt-br');
	});

	it('maps a slug back to its locale when the two differ', () => {
		expect(localePrefixOf('/zh')).toBe('zh-hans');
		expect(localePrefixOf('/zh-hant/breeding')).toBe('zh-hant');
	});

	it('treats the unprefixed root as having no locale', () => {
		expect(localePrefixOf('/')).toBeUndefined();
		expect(localePrefixOf('')).toBeUndefined();
	});

	// The English slug is empty, so /edit and /debug must not be read as a
	// prefix just because they start with the letters of one.
	it('ignores segments that merely begin with a locale slug', () => {
		expect(localePrefixOf('/debug')).toBeUndefined();
		expect(localePrefixOf('/edit')).toBeUndefined();
		expect(localePrefixOf('/es-mx-something')).toBeUndefined();
	});
});

describe('resolveInitialLocale', () => {
	it('prefers an explicit url prefix over the stored preference', () => {
		expect(resolveInitialLocale({ pathname: '/de/map', stored: 'fr' })).toBe('de');
	});

	it('uses the stored preference when the url carries no prefix', () => {
		expect(resolveInitialLocale({ pathname: '/', stored: 'fr' })).toBe('fr');
		expect(resolveInitialLocale({ pathname: '/file', stored: 'fr' })).toBe('fr');
	});

	it('falls back to the base locale with neither prefix nor preference', () => {
		expect(resolveInitialLocale({ pathname: '/', stored: undefined })).toBe('en');
	});

	it('ignores a stored value that is not a supported locale', () => {
		expect(resolveInitialLocale({ pathname: '/', stored: 'klingon' })).toBe('en');
	});
});

describe('createLocaleRuntime', () => {
	it('reports the locale it was seeded with', () => {
		const runtime = createLocaleRuntime('fr', deps('fr'));
		expect(runtime.getLocale()).toBe('fr');
	});

	// The regression this whole change exists for: resolving the locale on a
	// hub route must not write the url's locale over what the user chose.
	it('does not touch the stored preference just by being read', () => {
		const d = deps('de');
		const runtime = createLocaleRuntime('en', d);
		runtime.getLocale();
		runtime.getLocale();
		expect(d.calls).toEqual([]);
		expect(d.stored()).toBe('de');
	});

	it('persists and re-renders on an explicit switch', () => {
		const d = deps('en');
		const runtime = createLocaleRuntime('en', d);
		runtime.setLocale('de');
		expect(runtime.getLocale()).toBe('de');
		expect(d.calls).toEqual(['persist:de', 'lang:de', 'bump']);
	});

	it('keeps the html lang tag in bcp-47 form', () => {
		const d = deps('en');
		createLocaleRuntime('en', d).setLocale('pt-br');
		expect(d.calls).toContain('lang:pt-BR');
	});

	it('is a no-op when the locale does not change', () => {
		const d = deps('de');
		const runtime = createLocaleRuntime('de', d);
		runtime.setLocale('de');
		expect(d.calls).toEqual([]);
	});

	it('rejects a locale outside the project set', () => {
		const d = deps('en');
		const runtime = createLocaleRuntime('en', d);
		expect(() => runtime.setLocale('klingon')).toThrow();
		expect(runtime.getLocale()).toBe('en');
	});
});

describe('createLocaleRuntime syncFromPath', () => {
	it('adopts and persists a locale the url states explicitly', () => {
		const d = deps('en');
		const runtime = createLocaleRuntime('en', d);
		runtime.syncFromPath('/de/map');
		expect(runtime.getLocale()).toBe('de');
		expect(d.stored()).toBe('de');
	});

	it('leaves the locale alone on an unprefixed route', () => {
		const d = deps('de');
		const runtime = createLocaleRuntime('de', d);
		runtime.syncFromPath('/edit');
		expect(runtime.getLocale()).toBe('de');
		expect(d.calls).toEqual([]);
	});
});

function installDeps(pathname: string, stored: string | undefined) {
	const calls: string[] = [];
	const d = deps(stored, calls);
	let installedGet: (() => SiteLocale) | undefined;
	let installedSet: ((code: SiteLocale) => void) | undefined;
	return {
		...d,
		pathname: () => pathname,
		overwriteGetLocale: (fn: () => SiteLocale) => {
			installedGet = fn;
			calls.push('overwriteGetLocale');
		},
		overwriteSetLocale: (fn: (code: SiteLocale) => void) => {
			installedSet = fn;
			calls.push('overwriteSetLocale');
		},
		get: () => installedGet?.(),
		set: (code: SiteLocale) => installedSet?.(code)
	};
}

describe('installLocaleRuntime', () => {
	it('hands paraglide both of its resolution functions', () => {
		const d = installDeps('/edit', 'de');
		installLocaleRuntime(d);
		expect(d.calls).toContain('overwriteGetLocale');
		expect(d.calls).toContain('overwriteSetLocale');
		expect(d.get()).toBe('de');
	});

	// The desktop app opens at `/` every launch. Resolving there must not write
	// anything, or the settings echo reconciles the write back into the database
	// and the chosen language is gone for good.
	it('writes nothing when the url states no locale', () => {
		const d = installDeps('/', 'de');
		installLocaleRuntime(d);
		expect(d.calls.filter((c) => c.startsWith('persist:'))).toEqual([]);
		expect(d.stored()).toBe('de');
		expect(d.get()).toBe('de');
	});

	it('adopts and stores the locale an explicit url states', () => {
		const d = installDeps('/de/map', 'fr');
		installLocaleRuntime(d);
		expect(d.get()).toBe('de');
		expect(d.stored()).toBe('de');
	});

	it('leaves the preference alone when the url only repeats it', () => {
		const d = installDeps('/de', 'de');
		installLocaleRuntime(d);
		expect(d.calls.filter((c) => c.startsWith('persist:'))).toEqual([]);
	});

	it('corrects the html lang tag baked into the prerendered page', () => {
		const d = installDeps('/edit', 'pt-br');
		installLocaleRuntime(d);
		expect(d.calls).toContain('lang:pt-BR');
	});

	it('routes a later switch through the installed setter', () => {
		const d = installDeps('/edit', 'en');
		installLocaleRuntime(d);
		d.set('fr');
		expect(d.get()).toBe('fr');
		expect(d.stored()).toBe('fr');
	});
});
