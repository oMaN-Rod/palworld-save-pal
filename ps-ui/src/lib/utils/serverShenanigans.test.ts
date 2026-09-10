import type { OverviewStats } from '$states';
import { describe, expect, it } from 'vitest';
import {
	computeWorldLevel,
	generateShenanigans,
	type ShenaniganKind,
	type SkillNames
} from './serverShenanigans';

const names: SkillNames = {
	passive: (key) => ({ Noukin: 'Musclehead' })[key] ?? key,
	active: (key) => ({ 'EPalWazaID::AirCanon': 'Air Cannon' })[key] ?? key,
	species: (key) => ({ Sheepball: 'Lamball' })[key] ?? key
};

const stats = (overrides: Partial<OverviewStats> = {}): OverviewStats =>
	({
		totals: {
			players: 4,
			pals: 120,
			creature_pals: 118,
			human_npcs: 6,
			species: 40,
			guilds: 2,
			bases: 3,
			containers: 18
		},
		traits: { boss_pals: 4, rare_pals: 3, awakened_pals: 2 },
		condition: { sick_pals: 1, fainted_pals: 2 },
		composition: {
			avg_level: 25,
			gender: { male: 70, female: 48, unknown: 2 },
			level_brackets: [
				{ label: '1-20', count: 50 },
				{ label: '21-40', count: 40 },
				{ label: '41-60', count: 20 },
				{ label: '61-80', count: 10 }
			],
			talent_avg: { hp: 80, attack: 90, defense: 60 },
			top_passives: [
				{ skill: 'Noukin', count: 30 },
				{ skill: 'CraftSpeed_up1', count: 12 }
			],
			top_actives: [{ skill: 'EPalWazaID::AirCanon', count: 25 }]
		},
		top_species: [
			{ key: 'Sheepball', count: 30 },
			{ key: 'PinkCat', count: 10 }
		],
		top_players: [
			{
				uid: '11111111-1111-1111-1111-111111111111',
				nickname: 'NexusKnight',
				level: 50,
				pal_count: 40
			},
			{
				uid: '22222222-2222-2222-2222-222222222222',
				nickname: 'RunnerUp',
				level: 45,
				pal_count: 39
			}
		],
		anomalies: {
			pal_count: 5,
			danger_count: 1,
			by_code: [{ code: 'ILLEGAL_HP', count: 5 }],
			flagged: []
		},
		...overrides
	}) as OverviewStats;

const VALID_KINDS = [
	'science',
	'roast',
	'conspiracy',
	'prophecy',
	'hr',
	'nature',
	'weather',
	'commentary'
] as const satisfies readonly ShenaniganKind[];

describe('computeWorldLevel', () => {
	it('applies the certified formula', () => {
		const report = computeWorldLevel(stats());
		// player levels (50 + 45) ×2 = 190; pals 120 × 25 = 3000;
		// alphas 4 ×5 = 20; luckies 3 ×7 = 21; awakened 2 ×3 = 6 → 3237.
		expect(report.rawPower).toBe(3237);
		expect(report.level).toBe(32);
		expect(report.palEquivalents).toBe(54);
		expect(report.over9000).toBe(false);
	});

	it('flags over-9000 power on stacked servers', () => {
		const stacked = stats({
			totals: { ...stats().totals, pals: 500 },
			composition: {
				...stats().composition,
				avg_level: 50
			}
		});
		const report = computeWorldLevel(stacked);
		expect(report.rawPower).toBeGreaterThan(9000);
		expect(report.over9000).toBe(true);
		expect(report.headline).toContain('OVER 9,000');
	});

	it('hands out tier names as the level climbs', () => {
		expect(computeWorldLevel(stats()).tier).toBe('Chikipin Daycare');
		const fresh = stats({
			totals: { ...stats().totals, pals: 0 },
			composition: { ...stats().composition, avg_level: 0 },
			traits: { boss_pals: 0, rare_pals: 0, awakened_pals: 0 },
			top_players: []
		});
		expect(computeWorldLevel(fresh).tier).toBe('Fresh Save Energy');
	});
});

describe('generateShenanigans', () => {
	it('serves the requested amount without repeats', () => {
		for (let i = 0; i < 25; i += 1) {
			const batch = generateShenanigans(stats(), names, { count: 3 });
			expect(batch).toHaveLength(3);
			const texts = new Set(batch.map((s) => s.text));
			expect(texts.size).toBe(3);
			for (const shenanigan of batch) {
				expect(VALID_KINDS).toContain(shenanigan.kind);
				expect(shenanigan.text.length).toBeGreaterThan(10);
			}
		}
	});

	it('avoids the previous batch when enough material exists', () => {
		const first = generateShenanigans(stats(), names, { count: 3 });
		const second = generateShenanigans(stats(), names, {
			count: 3,
			avoid: first.map((s) => s.text)
		});
		const previousTexts = new Set(first.map((s) => s.text));
		for (const shenanigan of second) {
			expect(previousTexts.has(shenanigan.text)).toBe(false);
		}
	});

	it('still has something to say about an empty world', () => {
		const empty = stats({
			totals: {
				players: 0,
				pals: 0,
				creature_pals: 0,
				human_npcs: 0,
				species: 0,
				guilds: 0,
				bases: 0,
				containers: 0
			},
			traits: { boss_pals: 0, rare_pals: 0, awakened_pals: 0 },
			condition: { sick_pals: 0, fainted_pals: 0 },
			composition: {
				avg_level: 0,
				gender: { male: 0, female: 0, unknown: 0 },
				level_brackets: [],
				talent_avg: { hp: 0, attack: 0, defense: 0 },
				top_passives: [],
				top_actives: []
			},
			top_species: [],
			top_players: [],
			anomalies: { pal_count: 0, danger_count: 0, by_code: [], flagged: [] }
		});
		const batch = generateShenanigans(empty, names, { count: 3 });
		expect(batch.length).toBeGreaterThan(0);
	});

	it('roasts the leaderboard and Musclehead metas with localized names', () => {
		let sawPlayer = false;
		let sawMusclehead = false;
		for (let i = 0; i < 60 && !(sawPlayer && sawMusclehead); i += 1) {
			for (const shenanigan of generateShenanigans(stats(), names, { count: 3 })) {
				sawPlayer ||= shenanigan.text.includes('NexusKnight');
				sawMusclehead ||= shenanigan.text.includes('Musclehead');
			}
		}
		expect(sawPlayer).toBe(true);
		expect(sawMusclehead).toBe(true);
	});
});
