import { describe, expect, it, vi } from 'vitest';

vi.mock('$lib/data', () => ({
	elementsData: { elements: { Fire: { icon: 'fire' }, Water: { icon: 'water' } } }
}));

import { classifyPalCategory, palMatchesFilter } from './palFilters';
import type { PalData } from '$types';

const catalogPal = (over: Partial<PalData> = {}): PalData =>
	({
		is_pal: true,
		is_boss: false,
		is_tower_boss: false,
		is_raid_boss: false,
		predator: false,
		...over
	}) as PalData;

const pal = (over: Record<string, unknown> = {}) =>
	({
		character_id: 'SheepBall',
		is_boss: false,
		is_lucky: false,
		is_awakened: false,
		is_imported: false,
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

	it('matches awakened and imported off the pal flags', () => {
		expect(palMatchesFilter(pal({ is_awakened: true }), palData(), 'awakened')).toBe(true);
		expect(palMatchesFilter(pal(), palData(), 'awakened')).toBe(false);
		expect(palMatchesFilter(pal({ is_imported: true }), palData(), 'imported')).toBe(true);
		expect(palMatchesFilter(pal(), palData(), 'imported')).toBe(false);
	});
});

describe('classifyPalCategory', () => {
	it('classifies a plain pal as normal', () => {
		expect(classifyPalCategory('SheepBall', catalogPal())).toBe('normal');
	});

	it('classifies quest-prefixed keys as quest', () => {
		expect(classifyPalCategory('Quest_Farmer03_SheepBall', catalogPal())).toBe('quest');
		expect(classifyPalCategory('AmaterasuWolf_Dark_Quest_Enemy', catalogPal())).toBe('quest');
	});

	it('classifies bosses via flag OR prefix', () => {
		expect(classifyPalCategory('GYM_BlackGriffon', catalogPal({ is_tower_boss: true }))).toBe(
			'boss'
		);
		expect(classifyPalCategory('BOSS_DarkTrader', catalogPal({ is_boss: true }))).toBe('boss');
	});

	it('classifies special entries via flag OR prefix', () => {
		expect(classifyPalCategory('RAID_DarkMechaDragon', catalogPal({ is_raid_boss: true }))).toBe(
			'special'
		);
		expect(classifyPalCategory('PREDATOR_AmaterasuWolf', catalogPal({ predator: true }))).toBe(
			'special'
		);
		// SUMMON_DarkAlien has predator=true flag but no PREDATOR_ prefix — still special.
		expect(classifyPalCategory('SUMMON_DarkAlien', catalogPal({ predator: true }))).toBe(
			'special'
		);
		expect(classifyPalCategory('Baphomet_Dark_Oilrig', catalogPal())).toBe('special');
	});

	it('classifies human NPCs (non-pal) as other', () => {
		expect(classifyPalCategory('Believer_CrossBow_Tower', catalogPal({ is_pal: false }))).toBe(
			'other'
		);
	});

	it('prioritizes quest over boss and special', () => {
		expect(
			classifyPalCategory(
				'Quest_Hunter_GYM_Raider',
				catalogPal({ is_boss: true, is_raid_boss: true })
			)
		).toBe('quest');
	});

	it('prioritizes boss over special', () => {
		expect(
			classifyPalCategory('GYM_BlackGriffon', catalogPal({ is_boss: true, predator: true }))
		).toBe('boss');
	});
});
