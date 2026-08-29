import { describe, expect, it } from 'vitest';
import {
	availableModes,
	isModeAvailable,
	leaveIsSafe,
	MODE_LABELS,
	PANE_MODES,
	pluginIdFromPath,
	resolveMode
} from './pluginPane';

const user = { bundled: false };
const bundled = { bundled: true };

describe('availableModes', () => {
	it('offers run and code for a user plugin', () => {
		expect(availableModes(user)).toEqual(['run', 'code']);
	});

	/// A bundled plugin's code is the worked example authors copy from, so it is
	/// readable here. The editor is what refuses to save one.
	it('offers run and code for a bundled plugin too', () => {
		expect(availableModes(bundled)).toEqual(['run', 'code']);
	});

	it('never offers a mode outside the declared set', () => {
		for (const mode of availableModes(user)) {
			expect(PANE_MODES).toContain(mode);
		}
	});
});

describe('resolveMode', () => {
	it('defaults to run when the url says nothing', () => {
		expect(resolveMode(null, user)).toBe('run');
	});

	it('honours a valid available mode', () => {
		expect(resolveMode('code', user)).toBe('code');
	});

	it('falls back to run for an unrecognised mode rather than rendering nothing', () => {
		expect(resolveMode('designer', user)).toBe('run');
		expect(resolveMode('', user)).toBe('run');
		expect(resolveMode('CODE', user)).toBe('run');
	});

	it('honours code for a bundled plugin, whose source is readable', () => {
		expect(resolveMode('code', bundled)).toBe('code');
	});

	it('falls back to run when there is no plugin to decide against', () => {
		expect(resolveMode('code', undefined)).toBe('run');
		expect(resolveMode(null, undefined)).toBe('run');
	});
});

describe('MODE_LABELS', () => {
	it('labels every declared mode', () => {
		for (const mode of PANE_MODES) {
			expect(MODE_LABELS[mode]).toBeTypeOf('string');
			expect(MODE_LABELS[mode]).not.toBe('');
		}
	});

	it('labels nothing outside the declared set', () => {
		expect(Object.keys(MODE_LABELS).sort()).toEqual([...PANE_MODES].sort());
	});

	it('gives each mode a distinct label', () => {
		const labels = PANE_MODES.map((mode) => MODE_LABELS[mode]);
		expect(new Set(labels).size).toBe(labels.length);
	});
});

describe('pluginIdFromPath', () => {
	it('reads the id out of a plugin detail path', () => {
		expect(pluginIdFromPath('/plugins/user.one')).toBe('user.one');
		expect(pluginIdFromPath('/plugins/user.one/')).toBe('user.one');
	});

	it('decodes an escaped id', () => {
		expect(pluginIdFromPath('/plugins/my%20plugin')).toBe('my plugin');
	});

	it('is null for the list route, which leaves the detail pane', () => {
		expect(pluginIdFromPath('/plugins')).toBe(null);
		expect(pluginIdFromPath('/plugins/')).toBe(null);
	});

	it('is null for anywhere outside the section', () => {
		expect(pluginIdFromPath('/')).toBe(null);
		expect(pluginIdFromPath('/map')).toBe(null);
		expect(pluginIdFromPath('/pluginsomething/one')).toBe(null);
		expect(pluginIdFromPath('/plugins/one/two')).toBe(null);
	});

	it('is null when there is no target at all', () => {
		expect(pluginIdFromPath(null)).toBe(null);
		expect(pluginIdFromPath(undefined)).toBe(null);
	});

	it('keeps a malformed escape rather than throwing', () => {
		expect(pluginIdFromPath('/plugins/%E0%A4%A')).toBe('%E0%A4%A');
	});
});

describe('isModeAvailable', () => {
	it('agrees with availableModes', () => {
		for (const plugin of [user, bundled]) {
			for (const mode of PANE_MODES) {
				expect(isModeAvailable(mode, plugin)).toBe(availableModes(plugin).includes(mode));
			}
		}
	});
});

describe('leaveIsSafe', () => {
	it('is safe when the editor holds nothing', () => {
		expect(leaveIsSafe({ pluginId: null, dirty: false }, 'user.two')).toBe(true);
	});

	it('is safe when there are no unsaved edits', () => {
		expect(leaveIsSafe({ pluginId: 'user.one', dirty: false }, 'user.two')).toBe(true);
	});

	it('is safe when reselecting the plugin already open, dirty or not', () => {
		expect(leaveIsSafe({ pluginId: 'user.one', dirty: true }, 'user.one')).toBe(true);
	});

	it('is unsafe when switching away with unsaved edits', () => {
		expect(leaveIsSafe({ pluginId: 'user.one', dirty: true }, 'user.two')).toBe(false);
	});

	it('is safe to close the editor in place when there are no unsaved edits', () => {
		expect(leaveIsSafe({ pluginId: 'user.one', dirty: false }, null)).toBe(true);
	});

	it('is unsafe to close the editor in place with unsaved edits, even for the same plugin', () => {
		expect(leaveIsSafe({ pluginId: 'user.one', dirty: true }, null)).toBe(false);
	});
});
