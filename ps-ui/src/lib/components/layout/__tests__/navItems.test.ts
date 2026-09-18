import { describe, expect, it } from 'vitest';
import { TITLE_BAR_ACTION_IDS, isTitleBarAction, navItems } from '../navItems';

describe('TITLE_BAR_ACTION_IDS', () => {
	it('names exactly the four relocated actions', () => {
		expect([...TITLE_BAR_ACTION_IDS]).toEqual(['save', 'eject', 'open-folder', 'settings']);
	});

	it('every id resolves to a real nav item that has an action', () => {
		for (const id of TITLE_BAR_ACTION_IDS) {
			const item = navItems.find((candidate) => candidate.id === id);
			expect(item, `no nav item with id ${id}`).toBeDefined();
			expect(item?.action, `nav item ${id} has no action`).toBeDefined();
		}
	});

	it('does not claim the sidebar collapse toggle', () => {
		expect(isTitleBarAction('menu')).toBe(false);
	});
});
