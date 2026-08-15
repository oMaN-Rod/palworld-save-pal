// Builds the 3D map-object layer's items from Map.svelte's point lists and the
// source records behind them. Split out of Map.svelte, which has no test file, so
// the per-field mapping is asserted somewhere rather than only read.
import type { FastTravelPoint, MapUnlockPoint, Relic, RelicPoint } from '$types';
import { isWatchtower } from './fastTravel';
import type { MapObjectItem } from './mapObjectLayer';
import {
	FAST_TRAVEL_RADIUS_CM,
	fastTravelPortalColor,
	RELIC_RADIUS_CM,
	relicPortalColor,
	type FastTravelState,
	type RelicState
} from './mapObjectPortal';
import { buildRingFC } from './ringFC';
import type { MapArea } from './utils';

export function fastTravelStateOf(p: { unlocked?: boolean }): FastTravelState {
	return p.unlocked === undefined ? 'unknown' : p.unlocked ? 'unlocked' : 'locked';
}

export function relicStateOf(p: { unlocked?: boolean }): RelicState {
	return p.unlocked === undefined ? 'unknown' : p.unlocked ? 'collected' : 'uncollected';
}

export type FastTravelItemInput = {
	points: MapUnlockPoint[];
	sources: Record<string, FastTravelPoint>;
	size: number;
	/** Watchtowers share the fast travel list and layer but scale on their own. */
	watchtowerSize: number;
};

export type RelicItemInput = {
	show: boolean;
	points: RelicPoint[];
	sources: Record<string, Relic>;
	size: number;
};

// The point lists carry no height or actor class; the source records do. Fast
// travel points carry no rotation at all, so [0, 0, 0] is correct there rather
// than a placeholder, while relics carry their source's `rot` verbatim.
export function buildMapObjectItems(
	show3d: boolean,
	fastTravel: FastTravelItemInput,
	relics: RelicItemInput
): MapObjectItem[] {
	if (!show3d) return [];
	const items: MapObjectItem[] = [];

	for (const p of fastTravel.points) {
		const source = fastTravel.sources[p.guid];
		// The source record always carries a class; the point's is optional.
		if (source)
			items.push({
				x: p.x,
				y: p.y,
				z: source.z,
				actorClass: source.class,
				scale: isWatchtower(source) ? fastTravel.watchtowerSize : fastTravel.size,
				portalColor: `#${fastTravelPortalColor(fastTravelStateOf(p)).getHexString()}`,
				ringRadiusCm: FAST_TRAVEL_RADIUS_CM,
				rot: [0, 0, 0]
			});
	}

	if (relics.show) {
		for (const p of relics.points) {
			const source = relics.sources[p.guid];
			if (source)
				items.push({
					x: p.x,
					y: p.y,
					z: source.z,
					actorClass: source.class,
					scale: relics.size,
					portalColor: `#${relicPortalColor(relicStateOf(p)).getHexString()}`,
					ringRadiusCm: RELIC_RADIUS_CM,
					rot: source.rot
				});
		}
	}

	return items;
}

// The draped ground rings drawn under the 3D beam. Separate from
// buildMapObjectItems -- a ring needs only x/y and state -- but bound to the same
// radius constants. Exported per type rather than taking a radius argument so a
// test can prove each was never built from the other's radius.
export function buildFastTravelRingFC(
	points: MapUnlockPoint[],
	area: MapArea,
	scale: number,
	watchtowerScale: number
): GeoJSON.FeatureCollection {
	return buildRingFC(
		points,
		area,
		(p) => FAST_TRAVEL_RADIUS_CM * (isWatchtower(p) ? watchtowerScale : scale),
		(p) => ({ state: fastTravelStateOf(p) })
	);
}

export function buildRelicRingFC(
	points: RelicPoint[],
	area: MapArea,
	scale: number
): GeoJSON.FeatureCollection {
	return buildRingFC(points, area, RELIC_RADIUS_CM * scale, (p) => ({ state: relicStateOf(p) }));
}
