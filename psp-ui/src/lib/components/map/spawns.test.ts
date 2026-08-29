import type { Spawn } from '$types';
import { describe, expect, it } from 'vitest';
import bossesJson from '../../../../../data/json/bosses.json';
import { partitionSpawns } from './spawns';

const fixture: Record<string, Spawn> = {
	'0': {
		spawn_type: 'alpha',
		spawner_id: 'alpha_spawner',
		character_id: 'BOSS_IceHorse_Dark',
		level: 60,
		x: 1,
		y: 1,
		z: 1
	},
	'1': {
		spawn_type: 'boss',
		spawner_id: 'human_spawner',
		character_id: 'None',
		level: 50,
		x: 2,
		y: 2,
		z: 2
	},
	'2': {
		spawn_type: 'boss',
		spawner_id: 'named_boss_spawner',
		character_id: 'BOSS_Something',
		level: 55,
		x: 3,
		y: 3,
		z: 3
	},
	'3': { spawn_type: 'predator', pal: 'SifuDog', x: 4, y: 4, z: 4 },
	'4': {
		spawn_type: 'bounty',
		spawner_id: 'BOSS_Viking',
		character_id: 'None',
		level: 57,
		x: 5,
		y: 5,
		z: 5
	}
};

describe('partitionSpawns', () => {
	it('puts every entry in exactly one bucket', () => {
		const { alpha, boss, predator, bounty } = partitionSpawns(fixture);
		const seen = new Map<string, number>();
		for (const row of [...alpha, ...boss, ...predator, ...bounty]) {
			seen.set(row.rowKey, (seen.get(row.rowKey) ?? 0) + 1);
		}
		expect([...seen.values()]).toEqual(seen.size > 0 ? Array(seen.size).fill(1) : []);
	});

	it('sums the four bucket counts to the input length', () => {
		const { alpha, boss, predator, bounty } = partitionSpawns(fixture);
		expect(alpha.length + boss.length + predator.length + bounty.length).toBe(
			Object.keys(fixture).length
		);
	});

	it('routes only spawn_type "alpha" rows into the alpha bucket', () => {
		const { alpha } = partitionSpawns(fixture);
		expect(alpha.every((row) => row.spawn_type === 'alpha')).toBe(true);
	});

	it('routes only spawn_type "predator" rows into the predator bucket', () => {
		const { predator } = partitionSpawns(fixture);
		expect(predator.every((row) => row.spawn_type === 'predator')).toBe(true);
	});

	it('routes a human boss (character_id "None") into the boss bucket', () => {
		const { boss } = partitionSpawns(fixture);
		expect(boss.map((b) => b.rowKey)).toContain('1');
	});

	it('keeps alpha and predator rows out of the boss bucket', () => {
		const { boss } = partitionSpawns(fixture);
		expect(boss.map((b) => b.rowKey)).not.toContain('0');
		expect(boss.map((b) => b.rowKey)).not.toContain('3');
	});

	it('routes only spawn_type "bounty" rows into the bounty bucket', () => {
		const { bounty } = partitionSpawns(fixture);
		expect(bounty.map((row) => row.rowKey)).toEqual(['4']);
		expect(bounty.every((row) => row.spawn_type === 'bounty')).toBe(true);
	});

	it('keeps bounty rows out of the boss bucket', () => {
		const { boss } = partitionSpawns(fixture);
		expect(boss.map((b) => b.rowKey)).not.toContain('4');
	});

	it('drops a row with an unrecognized spawn_type instead of bucketing it as boss', () => {
		const withUnknown: Record<string, Spawn> = {
			...fixture,
			'9': { spawn_type: 'raid', x: 9, y: 9, z: 9 } as unknown as Spawn
		};
		const { alpha, boss, predator, bounty } = partitionSpawns(withUnknown);
		const allRowKeys = [...alpha, ...boss, ...predator, ...bounty].map((row) => row.rowKey);
		expect(allRowKeys).not.toContain('9');
		expect(allRowKeys.sort()).toEqual(['0', '1', '2', '3', '4']);
	});

	it('keeps rowKey as the original record key', () => {
		const { alpha } = partitionSpawns(fixture);
		expect(alpha[0].rowKey).toBe('0');
	});
});

describe('partitionSpawns over the real bosses.json', () => {
	it('pins the 72/21/29/33 alpha/boss/predator/bounty split', () => {
		const { alpha, boss, predator, bounty } = partitionSpawns(
			bossesJson as Record<string, Spawn>
		);
		expect(alpha.length).toBe(72);
		expect(boss.length).toBe(21);
		expect(predator.length).toBe(29);
		expect(bounty.length).toBe(33);
		expect(alpha.length + boss.length + predator.length + bounty.length).toBe(
			Object.keys(bossesJson).length
		);
	});
});
