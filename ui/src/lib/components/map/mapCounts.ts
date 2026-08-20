import { isWatchtower } from './fastTravel';
import { relicsByType, type RelicPlayerView } from './relics';
import { mapOf, type MapArea } from './utils';

export type FastTravelLike = { x: number; y: number; class?: string };
export type RelicLike = { x: number; y: number; relic_type: string };

export function pointsInArea<T extends { x: number; y: number }>(
	points: Record<string, T>,
	area: MapArea
): T[] {
	return Object.values(points).filter((point) => mapOf(point.x, point.y) === area);
}

export function areaFastTravelGuids(
	points: Record<string, FastTravelLike>,
	area: MapArea,
	watchtowers: boolean
): Set<string> {
	return new Set(
		Object.entries(points)
			.filter(
				([, point]) => isWatchtower(point) === watchtowers && mapOf(point.x, point.y) === area
			)
			.map(([guid]) => guid.toUpperCase())
	);
}

export function unlockedInArea(
	guids: Set<string>,
	unlocked: string[] | undefined
): number | undefined {
	if (!unlocked) return undefined;
	return unlocked.filter((guid) => guids.has(guid.toUpperCase())).length;
}

export function relicTypeStats(
	points: Record<string, RelicLike>,
	area: MapArea,
	player?: RelicPlayerView
): Record<string, { total: number; collected: number }> {
	const collectedSets: Record<string, Set<string>> = {};
	for (const [type, guids] of Object.entries(player ? relicsByType(player) : {})) {
		collectedSets[type] = new Set(guids.map((guid) => guid.toUpperCase()));
	}
	const stats: Record<string, { total: number; collected: number }> = {};
	for (const [guid, relic] of Object.entries(points)) {
		if (mapOf(relic.x, relic.y) !== area) continue;
		const entry = (stats[relic.relic_type] ??= { total: 0, collected: 0 });
		entry.total++;
		if (collectedSets[relic.relic_type]?.has(guid.toUpperCase())) entry.collected++;
	}
	return stats;
}

export function orderedRelicTypes(stats: Record<string, unknown>, gameOrder: string[]): string[] {
	const present = Object.keys(stats);
	const ordered = gameOrder.filter((type) => present.includes(type));
	return [...ordered, ...present.filter((type) => !ordered.includes(type))];
}
