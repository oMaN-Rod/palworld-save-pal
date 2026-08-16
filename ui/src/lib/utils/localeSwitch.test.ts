import { describe, expect, it } from 'vitest';
import { applyEditedSettings, switchLocale, type SettingsApplyDeps } from './localeSwitch';

function deps(current: string, calls: string[] = []): SettingsApplyDeps & { calls: string[] } {
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
		},
		persistAll: () => {
			calls.push('persistAll');
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

describe('applyEditedSettings', () => {
	// The settings echo no longer applies the backend's language, so the modal
	// that edited it has to switch the locale itself.
	it('switches the locale when the edit changed it', () => {
		const d = deps('en');
		applyEditedSettings('fr', d);
		expect(d.calls).toContain('setLocale:fr:reload=false');
	});

	it('does not persist twice when the locale switch already saved the settings', () => {
		const d = deps('en');
		applyEditedSettings('fr', d);
		expect(d.calls).not.toContain('persistAll');
	});

	it('persists the other edited settings when the locale is unchanged', () => {
		const d = deps('en');
		applyEditedSettings('en', d);
		expect(d.calls).toEqual(['persistAll']);
	});
});
