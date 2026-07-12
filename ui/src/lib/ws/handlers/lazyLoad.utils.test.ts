import { describe, expect, it } from 'vitest';
import { resolvePlayerDetailsRouting } from './lazyLoad.utils';

describe('resolvePlayerDetailsRouting', () => {
	it('routes the edit origin to the selected player and navigates to the editor', () => {
		expect(resolvePlayerDetailsRouting('edit')).toEqual({
			target: 'selected',
			navigateTo: '/edit/player'
		});
	});

	it('defaults an undefined origin to the edit behaviour', () => {
		expect(resolvePlayerDetailsRouting(undefined)).toEqual({
			target: 'selected',
			navigateTo: '/edit/player'
		});
	});

	it('routes the bulk origin to the bulk detail panel without navigating', () => {
		expect(resolvePlayerDetailsRouting('bulk')).toEqual({
			target: 'bulkDetail',
			navigateTo: null
		});
	});

	it('routes the worldmap origin to the selected player without navigating', () => {
		expect(resolvePlayerDetailsRouting('worldmap')).toEqual({
			target: 'selected',
			navigateTo: null
		});
	});

	it('routes the reattach origin to the selected player without navigating', () => {
		expect(resolvePlayerDetailsRouting('reattach')).toEqual({
			target: 'selected',
			navigateTo: null
		});
	});
});
