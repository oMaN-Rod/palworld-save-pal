import type { OverviewStats } from '$states';

export type PlayerRow = OverviewStats['top_players'][number];

export type LeaderboardMetric =
	| 'pal_count'
	| 'level'
	| 'avg_pal_level'
	| 'max_pal_level'
	| 'lucky_count'
	| 'total_power'
	| 'dps_pal_count';

export const LEADERBOARD_METRICS: LeaderboardMetric[] = [
	'pal_count',
	'level',
	'avg_pal_level',
	'max_pal_level',
	'lucky_count',
	'total_power',
	'dps_pal_count'
];

/** Descending by `metric`; ties fall to pal count, then level, then nickname. */
export function sortPlayersForMetric(players: PlayerRow[], metric: LeaderboardMetric): PlayerRow[] {
	return [...players].sort(
		(a, b) =>
			metricValue(b, metric) - metricValue(a, metric) ||
			b.pal_count - a.pal_count ||
			(b.level ?? 0) - (a.level ?? 0) ||
			a.nickname.localeCompare(b.nickname)
	);
}

/** Unknown values read as -1 so they rank below every real value. */
export function metricValue(player: PlayerRow, metric: LeaderboardMetric): number {
	return player[metric] ?? -1;
}
