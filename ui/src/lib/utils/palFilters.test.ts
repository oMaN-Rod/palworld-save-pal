import { describe, expect, it, vi } from 'vitest';

vi.mock('$lib/data', () => ({
	elementsData: { elements: { Fire: { icon: 'fire' }, Water: { icon: 'water' } } }
}));

import { palMatchesFilter } from './palFilters';

const pal = (over: Record<string, unknown> = {}) =>
	({
		character_id: 'SheepBall',
		is_boss: false,
		is_lucky: false,
		...over
	}) as any;

const palData = (over: Record<string, unknown> = {}) =>
	({ is_pal: true, element_types: ['Fire'], ...over }) as any;

describe('palMatchesFilter', () => {
	it('matches everything when no filter is selected', () => {
		expect(palMatchesFilter(pal(), palData(), 'All')).toBe(true);
	});

	it('matches on element type, case-insensitively', () => {
		expect(palMatchesFilter(pal(), palData(), 'Fire')).toBe(true);
		expect(palMatchesFilter(pal(), palData(), 'Water')).toBe(false);
	});

	it('matches alpha and lucky off the pal flags', () => {
		expect(palMatchesFilter(pal({ is_boss: true }), palData(), 'alpha')).toBe(true);
		expect(palMatchesFilter(pal(), palData(), 'alpha')).toBe(false);
		expect(palMatchesFilter(pal({ is_lucky: true }), palData(), 'lucky')).toBe(true);
	});

	it('matches human off palData, not the pal', () => {
		expect(palMatchesFilter(pal(), palData({ is_pal: false }), 'human')).toBe(true);
		expect(palMatchesFilter(pal(), palData(), 'human')).toBe(false);
	});

	it('matches predator, oilrig and summon off the character id', () => {
		expect(palMatchesFilter(pal({ character_id: 'PREDATOR_Wolf' }), palData(), 'predator')).toBe(
			true
		);
		expect(palMatchesFilter(pal({ character_id: 'Sheep_oilrig' }), palData(), 'oilrig')).toBe(true);
		expect(palMatchesFilter(pal({ character_id: 'SUMMON_Rock' }), palData(), 'summon')).toBe(true);
		expect(palMatchesFilter(pal(), palData(), 'predator')).toBe(false);
	});
});
