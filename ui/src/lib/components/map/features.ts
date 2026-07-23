import type { Base, MapObject, MapUnlockPoint, RelicPoint, WorldMapPoint } from '$types';
import { pixelCirclePolygon, pixelToLngLat } from './mercator';
import { cmPerPx, mapOf, MAP_SIZE, mapToWorld, worldToPixel, type MapArea } from './utils';
import { isWatchtower } from './fastTravel';
import {
	ICON_BASE,
	ICON_BOSS,
	ICON_DUNGEON,
	ICON_FAST_TRAVEL,
	ICON_ORIGIN,
	ICON_PLAYER,
	ICON_WATCHTOWER,
	palIconId,
	relicIconId
} from './iconIds';

export type MapFeatureType =
	| 'origin'
	| 'player'
	| 'base'
	| 'fast_travel'
	| 'relic'
	| 'dungeon'
	| 'boss'
	| 'alpha_pal'
	| 'predator_pal';

export type MapFeatureProps = {
	key: string;
	type: MapFeatureType;
	[k: string]: string | number | boolean | undefined;
};

type PointFeature = {
	type: 'Feature';
	id: number;
	geometry: { type: 'Point'; coordinates: [number, number] };
	properties: MapFeatureProps;
};

type PolygonFeature = {
	type: 'Feature';
	id: number;
	geometry: { type: 'Polygon'; coordinates: [number, number][][] };
	properties: MapFeatureProps;
};

type LineFeature = {
	type: 'Feature';
	id: number;
	geometry: { type: 'LineString'; coordinates: [number, number][] };
	properties: MapFeatureProps;
};

export type PointFC = { type: 'FeatureCollection'; features: PointFeature[] };
export type PolygonFC = { type: 'FeatureCollection'; features: PolygonFeature[] };
export type LineFC = { type: 'FeatureCollection'; features: LineFeature[] };

export type PlayerLike = { uid: string; nickname?: string; location?: WorldMapPoint | null };
export type BaseEntry = { base: Base; guildName?: string };
export type BossView = {
	rowKey: string;
	spawner_id: string;
	character_id: string;
	level: number;
	x: number;
	y: number;
	defeated: boolean;
	localized_name?: string;
};

export function emptyFC(): PointFC {
	return { type: 'FeatureCollection', features: [] };
}

function point(
	id: number,
	worldX: number,
	worldY: number,
	area: MapArea,
	properties: MapFeatureProps
): PointFeature {
	const [px, py] = worldToPixel(worldX, worldY, area);
	return {
		type: 'Feature',
		id,
		geometry: { type: 'Point', coordinates: pixelToLngLat(px, py) },
		properties
	};
}

function inArea(x: number, y: number, area: MapArea): boolean {
	return mapOf(x, y) === area;
}

export function buildPlayerFC(players: PlayerLike[], area: MapArea): PointFC {
	const features: PointFeature[] = [];
	for (const player of players) {
		const loc = player.location;
		if (!loc || !inArea(loc.x, loc.y, area)) continue;
		features.push(
			point(features.length, loc.x, loc.y, area, {
				key: player.uid,
				type: 'player',
				name: player.nickname ?? '',
				icon: ICON_PLAYER
			})
		);
	}
	return { type: 'FeatureCollection', features };
}

export function buildBaseFC(entries: BaseEntry[], area: MapArea): PointFC {
	const features: PointFeature[] = [];
	for (const { base, guildName } of entries) {
		const loc = base.location;
		if (!loc || !inArea(loc.x, loc.y, area)) continue;
		features.push(
			point(features.length, loc.x, loc.y, area, {
				key: base.id,
				type: 'base',
				name: base.name ?? '',
				guild: guildName ?? '',
				icon: ICON_BASE
			})
		);
	}
	return { type: 'FeatureCollection', features };
}

export function buildBaseRadiusFC(entries: BaseEntry[], area: MapArea): PolygonFC {
	const features: PolygonFeature[] = [];
	const cm = cmPerPx(area);
	for (const { base } of entries) {
		const loc = base.location;
		if (!loc || !inArea(loc.x, loc.y, area)) continue;
		const [cx, cy] = worldToPixel(loc.x, loc.y, area);
		const radiusPx = (base.area_range || 3500) / cm;
		features.push({
			type: 'Feature',
			id: features.length,
			geometry: { type: 'Polygon', coordinates: [pixelCirclePolygon(cx, cy, radiusPx)] },
			properties: { key: base.id, type: 'base' }
		});
	}
	return { type: 'FeatureCollection', features };
}

export function buildFastTravelFC(points: MapUnlockPoint[], area: MapArea): PointFC {
	const features: PointFeature[] = [];
	for (const p of points) {
		if (!inArea(p.x, p.y, area)) continue;
		const watchtower = isWatchtower(p);
		features.push(
			point(features.length, p.x, p.y, area, {
				key: p.guid,
				type: 'fast_travel',
				name: p.localized_name ?? '',
				watchtower,
				locked: p.unlocked === false,
				icon: watchtower ? ICON_WATCHTOWER : ICON_FAST_TRAVEL
			})
		);
	}
	return { type: 'FeatureCollection', features };
}

export function buildRelicFC(points: RelicPoint[], area: MapArea): PointFC {
	const features: PointFeature[] = [];
	for (const p of points) {
		if (!inArea(p.x, p.y, area)) continue;
		features.push(
			point(features.length, p.x, p.y, area, {
				key: p.guid,
				type: 'relic',
				name: p.localized_name ?? '',
				relicType: p.relic_type,
				collected: p.unlocked !== false,
				icon: relicIconId(p.relic_type)
			})
		);
	}
	return { type: 'FeatureCollection', features };
}

export function buildMapObjectFC(
	points: MapObject[],
	type: MapFeatureType,
	area: MapArea
): PointFC {
	const features: PointFeature[] = [];
	for (const p of points) {
		if (!inArea(p.x, p.y, area)) continue;
		const icon =
			type === 'alpha_pal'
				? palIconId(p.pal, false)
				: type === 'predator_pal'
					? palIconId(p.pal, true)
					: ICON_DUNGEON;
		features.push(
			point(features.length, p.x, p.y, area, {
				key: `${type}:${p.x}:${p.y}`,
				type,
				name: p.localized_name ?? '',
				pal: p.pal ?? '',
				icon
			})
		);
	}
	return { type: 'FeatureCollection', features };
}

export function buildBossFC(points: BossView[], area: MapArea): PointFC {
	const features: PointFeature[] = [];
	for (const b of points) {
		if (!inArea(b.x, b.y, area)) continue;
		features.push(
			point(features.length, b.x, b.y, area, {
				key: b.rowKey,
				type: 'boss',
				name: b.localized_name ?? '',
				level: b.level,
				defeated: b.defeated,
				icon: ICON_BOSS
			})
		);
	}
	return { type: 'FeatureCollection', features };
}

function originPixel(area: MapArea): [number, number] {
	const world = mapToWorld(0, 0);
	return worldToPixel(world.x, world.y, area);
}

export function buildOriginFC(area: MapArea): PointFC {
	const world = mapToWorld(0, 0);
	return {
		type: 'FeatureCollection',
		features: [
			point(0, world.x, world.y, area, { key: 'origin', type: 'origin', icon: ICON_ORIGIN })
		]
	};
}

export function buildOriginCrosshairFC(area: MapArea): LineFC {
	const [ox, oy] = originPixel(area);
	const horizontal: [number, number][] = [pixelToLngLat(0, oy), pixelToLngLat(MAP_SIZE, oy)];
	const vertical: [number, number][] = [pixelToLngLat(ox, 0), pixelToLngLat(ox, MAP_SIZE)];
	return {
		type: 'FeatureCollection',
		features: [
			{
				type: 'Feature',
				id: 0,
				geometry: { type: 'LineString', coordinates: horizontal },
				properties: { key: 'origin-h', type: 'origin' }
			},
			{
				type: 'Feature',
				id: 1,
				geometry: { type: 'LineString', coordinates: vertical },
				properties: { key: 'origin-v', type: 'origin' }
			}
		]
	};
}
