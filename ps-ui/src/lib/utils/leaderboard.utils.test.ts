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

const names = (players: PlayerRow[]) => players.map((p) => p.nickname);

describe('sortPlayersForMetric', () => {
	it('ranks by the selected metric descending', () => {
		const players = [
			player({ uid: 'a', nickname: 'Few', pal_count: 2, lucky_count: 0 }),
			player({ uid: 'b', nickname: 'Many', pal_count: 30, lucky_count: 5 }),
			player({ uid: 'c', nickname: 'Mid', pal_count: 10, lucky_count: 2 })
		];
		expect(names(sortPlayersForMetric(players, 'pal_count'))).toEqual(['Many', 'Mid', 'Few']);
		expect(names(sortPlayersForMetric(players, 'lucky_count'))).toEqual(['Many', 'Mid', 'Few']);
	});

	it('breaks level ties by pal count', () => {
		const players = [
			player({ uid: 'a', nickname: 'SameLevelFewPals', level: 50, pal_count: 3 }),
			player({ uid: 'b', nickname: 'SameLevelManyPals', level: 50, pal_count: 12 }),
			player({ uid: 'c', nickname: 'Higher', level: 55, pal_count: 1 })
		];
		expect(names(sortPlayersForMetric(players, 'level'))).toEqual([
			'Higher',
			'SameLevelManyPals',
			'SameLevelFewPals'
		]);
	});

	it('sorts unknown values last, including unscanned DPS', () => {
		const players = [
			player({ uid: 'a', nickname: 'Unknown', avg_pal_level: null, dps_pal_count: null }),
			player({ uid: 'b', nickname: 'Known', avg_pal_level: 12.5, dps_pal_count: 0 })
		];
		expect(names(sortPlayersForMetric(players, 'avg_pal_level'))).toEqual(['Known', 'Unknown']);
		expect(names(sortPlayersForMetric(players, 'dps_pal_count'))).toEqual(['Known', 'Unknown']);
	});

	it('falls back to nickname when the whole tie ladder is equal', () => {
		const players = [
			player({ uid: 'b', nickname: 'Zoe', pal_count: 5, level: 20 }),
			player({ uid: 'a', nickname: 'Able', pal_count: 5, level: 20 })
		];
		expect(names(sortPlayersForMetric(players, 'total_power'))).toEqual(['Able', 'Zoe']);
	});

	it('does not mutate the input array', () => {
		const players = [
			player({ uid: 'a', nickname: 'A', pal_count: 1 }),
			player({ uid: 'b', nickname: 'B', pal_count: 9 })
		];
		sortPlayersForMetric(players, 'pal_count');
		expect(names(players)).toEqual(['A', 'B']);
	});

	it('reads every selectable metric off the row', () => {
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
