import { describe, expect, it } from 'vitest';
import { reconcileSettingsLocale, type LocaleReconcileDeps } from './localeReconcile';

function deps(
	stored: string | undefined,
	current = 'en',
	calls: string[] = []
): LocaleReconcileDeps & { calls: string[] } {
	return {
		calls,
		storedLocale: () => stored,
		getLocale: () => current,
		setLocale: (code, opts) => {
			calls.push(`setLocale:${code}:reload=${opts.reload}`);
		},
		bump: () => {
			calls.push('bump');
		},
		persist: (code) => {
			calls.push(`persist:${code}`);
		}
	};
}

describe('reconcileSettingsLocale', () => {
	it('keeps the locale the browser stores when the backend row disagrees', () => {
		const d = deps('fr', 'fr');
		expect(reconcileSettingsLocale('en', d)).toBe('fr');
		expect(d.calls).not.toContain('setLocale:en:reload=false');
	});

	it('corrects the backend row instead of adopting its language', () => {
		const d = deps('fr', 'fr');
		reconcileSettingsLocale('en', d);
		expect(d.calls).toContain('persist:fr');
	});

	it('leaves the backend alone when it already mirrors the stored locale', () => {
		const d = deps('fr', 'fr');
		expect(reconcileSettingsLocale('fr', d)).toBe('fr');
		expect(d.calls).toEqual([]);
	});

	it('adopts the backend language when the browser stores no locale of its own', () => {
		const d = deps(undefined, 'en');
		expect(reconcileSettingsLocale('de', d)).toBe('de');
		expect(d.calls).toContain('setLocale:de:reload=false');
	});

	it('bumps the reactivity signal when it adopts the backend language', () => {
		const d = deps(undefined, 'en');
		reconcileSettingsLocale('de', d);
		expect(d.calls).toContain('bump');
	});

	it('does not push the adopted language back to the backend it came from', () => {
		const d = deps(undefined, 'en');
		reconcileSettingsLocale('de', d);
		expect(d.calls).not.toContain('persist:de');
	});

	it('is a no-op when nothing is stored and the backend matches the active locale', () => {
		const d = deps(undefined, 'en');
		expect(reconcileSettingsLocale('en', d)).toBe('en');
		expect(d.calls).toEqual([]);
	});
});
