import { describe, expect, it } from 'vitest';
import type { PlayerRow } from './leaderboard.utils';
import { LEADERBOARD_METRICS, metricValue, sortPlayersForMetric } from './leaderboard.utils';

function player(partial: Partial<PlayerRow> & { uid: string; nickname: string }): PlayerRow {
	return {
		level: null,
		pal_count: 0,
		lucky_count: 0,
		avg_pal_level: null,
		max_pal_level: null,
		total_power: 0,
		dps_pal_count: 0,
		...partial
	};
}

describe('sortPlayersForMetric', () => {
	it('ranks by the selected metric descending', () => {
		const players = [
			player({ uid: 'a', nickname: 'Few', pal_count: 2, lucky_count: 0 }),
			player({ uid: 'b', nickname: 'Many', pal_count: 30, lucky_count: 5 }),
			player({ uid: 'c', nickname: 'Mid', pal_count: 10, lucky_count: 2 })
		];
		expect(sortPlayersForMetric(players, 'pal_count').map((p) => p.nickname)).toEqual([
			'Many',
			'Mid',
			'Few'
		]);
		expect(sortPlayersForMetric(players, 'lucky_count').map((p) => p.nickname)).toEqual([
			'Many',
			'Mid',
			'Few'
		]);
	});

	it('breaks level ties by pal count, like the request describes', () => {
		const players = [
			player({ uid: 'a', nickname: 'SameLevelFewPals', level: 50, pal_count: 3 }),
			player({ uid: 'b', nickname: 'SameLevelManyPals', level: 50, pal_count: 12 }),
			player({ uid: 'c', nickname: 'Higher', level: 55, pal_count: 1 })
		];
		expect(sortPlayersForMetric(players, 'level').map((p) => p.nickname)).toEqual([
			'Higher',
			'SameLevelManyPals',
			'SameLevelFewPals'
		]);
	});

	it('sorts null metric values last and never throws', () => {
		const players = [
			player({ uid: 'a', nickname: 'Unknown', avg_pal_level: null, level: null }),
			player({ uid: 'b', nickname: 'Known', avg_pal_level: 12.5, level: 9 })
		];
		expect(sortPlayersForMetric(players, 'avg_pal_level').map((p) => p.nickname)).toEqual([
			'Known',
			'Unknown'
		]);
		expect(sortPlayersForMetric(players, 'level')[0].nickname).toBe('Known');
	});

	it('falls back to nickname when the whole tie ladder is equal', () => {
		const players = [
			player({ uid: 'b', nickname: 'Zoe', pal_count: 5, level: 20 }),
			player({ uid: 'a', nickname: 'Able', pal_count: 5, level: 20 })
		];
		expect(sortPlayersForMetric(players, 'total_power').map((p) => p.nickname)).toEqual([
			'Able',
			'Zoe'
		]);
	});

	it('does not mutate the input array', () => {
		const players = [
			player({ uid: 'a', nickname: 'A', pal_count: 1 }),
			player({ uid: 'b', nickname: 'B', pal_count: 9 })
		];
		sortPlayersForMetric(players, 'pal_count');
		expect(players.map((p) => p.nickname)).toEqual(['A', 'B']);
	});

	it('covers every selectable metric', () => {
		const row = player({
			uid: 'a',
			nickname: 'A',
			level: 10,
			pal_count: 4,
			lucky_count: 2,
			avg_pal_level: 33.3,
			max_pal_level: 44,
			total_power: 5000,
			dps_pal_count: 6
		});
		for (const metric of LEADERBOARD_METRICS) {
			expect(metricValue(row, metric)).toBeGreaterThan(0);
		}
	});
});
