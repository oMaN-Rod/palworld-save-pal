import type { FastTravelPoint, MapUnlockPoint, Relic, RelicPoint } from '$types';
import { isWatchtower } from '../../features/fastTravel';
import type { MapObjectItem } from './mapObjectLayer';
import {
	FAST_TRAVEL_RADIUS_CM,
	fastTravelPortalColor,
	RELIC_RADIUS_CM,
	relicPortalColor,
	type FastTravelState,
	type RelicState
} from './mapObjectPortal';
import { buildRingFC } from '../mesh/ringFC';
import type { MapArea } from '../../geo/utils';

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
	watchtowerSize: number;
};

export type RelicItemInput = {
	show: boolean;
	points: RelicPoint[];
	sources: Record<string, Relic>;
	size: number;
};

export function buildMapObjectItems(
	show3d: boolean,
	fastTravel: FastTravelItemInput,
	relics: RelicItemInput
): MapObjectItem[] {
	if (!show3d) return [];
	const items: MapObjectItem[] = [];

	for (const p of fastTravel.points) {
		const source = fastTravel.sources[p.guid];
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
