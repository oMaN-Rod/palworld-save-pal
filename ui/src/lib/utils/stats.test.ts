import { describe, expect, it, vi } from 'vitest';

vi.mock('$lib/data', () => ({
	palsData: {
		getByKey: () => ({
			is_pal: true,
			is_tower_boss: false,
			is_raid_boss: false,
			scaling: { hp: 70, attack: 70, defense: 70 }
		})
	},
	passiveSkillsData: { getByKey: () => undefined }
}));

import { getStats } from './stats';

function palAt(level: number, isAwakened: boolean) {
	return {
		character_key: 'sheepball',
		level,
		rank: 1,
		rank_hp: 0,
		rank_attack: 0,
		rank_defense: 0,
		rank_craftspeed: 0,
		talent_hp: 0,
		talent_shot: 0,
		talent_defense: 0,
		passive_skills: [],
		is_boss: false,
		is_lucky: false,
		is_awakened: isAwakened,
		is_imported: false,
		max_hp: 0
	} as any;
}

describe('getStats awakening', () => {
	it('leaves a non-awakened pal on the existing numbers', () => {
		const pal = palAt(10, false);
		const stats = getStats(pal)!;
		// floor(500 + 5*10 + 70*0.5*10) = 900
		expect(pal.max_hp).toBe(900_000);
		// floor(70 * 0.075 * 10) = 52
		expect(stats.attack).toBe(52);
		// floor(50 + 70 * 0.075 * 10) = 102
		expect(stats.defense).toBe(102);
	});

	it('scales an awakened pal by 1.1', () => {
		const pal = palAt(10, true);
		const stats = getStats(pal)!;
		expect(pal.max_hp).toBe(990_000);
		expect(stats.attack).toBe(Math.floor(52 * 1.1));
		expect(stats.defense).toBe(Math.floor(102 * 1.1));
	});
});
