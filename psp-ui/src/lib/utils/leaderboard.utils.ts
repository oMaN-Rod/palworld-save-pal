import type { OverviewStats } from '$states';

export type PlayerRow = OverviewStats['top_players'][number];

/** The leaderboard's selectable ranking metrics. */
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

/**
 * Sorts players for one ranking metric, descending. Null values (no known
 * level / no readable pals) sort below every real value, and ties walk the
 * same deterministic ladder the backend's default ordering uses: primary
 * metric, then pal count, then player level, then nickname.
 */
export function sortPlayersForMetric(players: PlayerRow[], metric: LeaderboardMetric): PlayerRow[] {
	return [...players].sort((a, b) => {
		const primary = metricValue(b, metric) - metricValue(a, metric);
		if (primary !== 0) return primary;
		const pals = b.pal_count - a.pal_count;
		if (pals !== 0) return pals;
		const level = (b.level ?? 0) - (a.level ?? 0);
		if (level !== 0) return level;
		return a.nickname.localeCompare(b.nickname);
	});
}

/** The numeric sort key for a metric; `null` reads as -1 so it ranks last. */
export function metricValue(player: PlayerRow, metric: LeaderboardMetric): number {
	const value = player[metric];
	return value == null ? -1 : value;
}
