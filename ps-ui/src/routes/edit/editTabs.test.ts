import { describe, expect, it } from 'vitest';

import { EDIT_TABS, shortcutMap, toContextItems, type EditTabContext } from './editTabs';

const full: EditTabContext = { hasSave: true, hasPlayer: true, hasDps: true, hasGps: true };

describe('EDIT_TABS', () => {
	it('gives every tab a unique id and a unique shortcut', () => {
		expect(new Set(EDIT_TABS.map((t) => t.id)).size).toBe(EDIT_TABS.length);
		expect(new Set(EDIT_TABS.map((t) => t.shortcut)).size).toBe(EDIT_TABS.length);
	});

	it('derives the shortcut map from the same array', () => {
		const map = shortcutMap();
		for (const tab of EDIT_TABS) {
			expect(map[tab.shortcut]).toBe(tab.id);
		}
		expect(Object.keys(map)).toHaveLength(EDIT_TABS.length);
	});

	it("navigates every tab's shortcut to that tab's own href", () => {
		const map = shortcutMap();
		const byId = new Map(EDIT_TABS.map((tab) => [tab.id, tab]));

		for (const [code, id] of Object.entries(map)) {
			const tab = byId.get(id);
			expect(tab).toBeDefined();
			expect(tab?.shortcut).toBe(code);
			expect(tab?.href).toBe(EDIT_TABS.find((t) => t.shortcut === code)?.href);
		}
	});

	it('preserves the exact shortcut key for every tab', () => {
		expect(shortcutMap()).toEqual({
			KeyL: 'player',
			KeyT: 'technologies',
			KeyB: 'palbox',
			KeyF: 'effigies',
			KeyD: 'dps',
			KeyG: 'guild',
			KeyM: 'missions',
			KeyS: 'gps'
		});
	});

	it('points the gps shortcut at the real /gps route, not /edit/gps', () => {
		const gps = EDIT_TABS.find((t) => t.id === 'gps');
		expect(gps?.href).toBe('/gps');
	});

	it('flags exactly one tab as shortcut-only, and it is gps', () => {
		const shortcutOnly = EDIT_TABS.filter((t) => t.shortcutOnly).map((t) => t.id);
		expect(shortcutOnly).toEqual(['gps']);
	});

	it('excludes shortcut-only tabs from the rendered strip, in order', () => {
		const strip = EDIT_TABS.filter((tab) => !tab.shortcutOnly).map((t) => t.id);
		expect(strip).toEqual([
			'player',
			'technologies',
			'palbox',
			'effigies',
			'dps',
			'guild',
			'missions'
		]);
	});

	it('hides the dps tab when the player has no dps container', () => {
		const ids = toContextItems(EDIT_TABS, { ...full, hasDps: false })
			.filter((i) => i.available !== false)
			.map((i) => i.id);
		expect(ids).not.toContain('dps');
	});

	it('hides the gps tab when there is no gps container', () => {
		const ids = toContextItems(EDIT_TABS, { ...full, hasGps: false })
			.filter((i) => i.available !== false)
			.map((i) => i.id);
		expect(ids).not.toContain('gps');
	});

	it('hides every tab when no player is selected', () => {
		const ids = toContextItems(EDIT_TABS, {
			hasSave: true,
			hasPlayer: false,
			hasDps: false,
			hasGps: false
		})
			.filter((i) => i.available !== false)
			.map((i) => i.id);
		expect(ids).toEqual([]);
	});

	it('shows every tab with a fully loaded save', () => {
		const ids = toContextItems(EDIT_TABS, full)
			.filter((i) => i.available !== false)
			.map((i) => i.id);
		expect(ids).toContain('player');
		expect(ids).toContain('palbox');
		expect(ids).toContain('guild');
	});
});
