import { describe, it, expect } from 'vitest';
import { switchLocale, type LocaleSwitchDeps } from './localeSwitch';

function deps(current: string, calls: string[] = []): LocaleSwitchDeps & { calls: string[] } {
	return {
		calls,
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

describe('switchLocale', () => {
	it('is a no-op when the locale is unchanged', () => {
		const d = deps('en');
		expect(switchLocale('en', d)).toBe(false);
		expect(d.calls).toEqual([]);
	});

	it('sets the locale without reloading', () => {
		const d = deps('en');
		expect(switchLocale('fr', d)).toBe(true);
		expect(d.calls).toContain('setLocale:fr:reload=false');
	});

	it('sets the locale before persisting so the settings echo cannot trigger a reload', () => {
		const d = deps('en');
		switchLocale('fr', d);
		expect(d.calls.indexOf('setLocale:fr:reload=false')).toBeLessThan(
			d.calls.indexOf('persist:fr')
		);
	});

	it('bumps the reactivity signal so message accessors re-evaluate', () => {
		const d = deps('en');
		switchLocale('fr', d);
		expect(d.calls).toContain('bump');
	});

	it('persists the new locale', () => {
		const d = deps('en');
		switchLocale('fr', d);
		expect(d.calls).toContain('persist:fr');
	});
});
