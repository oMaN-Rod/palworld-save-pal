import type {
	Base,
	BaseStructure,
	Footprint,
	MapObject,
	MapUnlockPoint,
	RelicPoint,
	WorldMapPoint
} from '$types';
import { pixelCirclePolygon, pixelToLngLat } from '../geo/mercator';
import { cmPerPx, mapOf, MAP_SIZE, mapToWorld, worldToPixel, type MapArea } from '../geo/utils';
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
} from '../style/iconIds';

export type MapFeatureType =
	| 'origin'
	| 'player'
	| 'base'
	| 'fast_travel'
	| 'relic'
	| 'dungeon'
	| 'boss'
	| 'alpha_pal'
	| 'predator_pal'
	| 'bounty'
	| 'structure';

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

export type StructureFeature = {
	type: 'Feature';
	id?: string;
	geometry: { type: 'Polygon'; coordinates: [number, number][][] };
	properties: { key: string; type: 'structure'; typeA: string; b: number; h: number };
};

export type StructureFC = { type: 'FeatureCollection'; features: StructureFeature[] };

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
				? palIconId(p.pal ?? '', false)
				: type === 'predator_pal'
					? palIconId(p.pal ?? '', true)
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

export function buildBossFC(
	points: BossView[],
	area: MapArea,
	marker: { type: MapFeatureType; icon: string } = { type: 'boss', icon: ICON_BOSS }
): PointFC {
	const features: PointFeature[] = [];
	for (const b of points) {
		if (!inArea(b.x, b.y, area)) continue;
		features.push(
			point(features.length, b.x, b.y, area, {
				key: b.rowKey,
				type: marker.type,
				name: b.localized_name ?? '',
				level: b.level,
				defeated: b.defeated,
				icon: marker.icon
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

export const DEFAULT_STRUCTURE_FOOTPRINT: Footprint = {
	sx: 100, sy: 100, sz: 100, ox: 0, oy: 0, oz: 0, typeA: 'Other'
};

// A save's map_object_id does not always match the data table row key's casing
// ("Stone_Foundation" vs "Stone_foundation"), which silently drops the structure
// to the default footprint. Exact matches must still win: a few pillar ids exist
// in both casings as genuinely distinct rows.
let ciIndex = new Map<string, Footprint>();
let ciSource: Record<string, Footprint> | null = null;

export function lookupFootprint(
	footprints: Record<string, Footprint>,
	mapObjectId: string
): Footprint | undefined {
	const exact = footprints[mapObjectId];
	if (exact) return exact;
	if (ciSource !== footprints) {
		ciIndex = new Map();
		for (const [key, value] of Object.entries(footprints)) {
			const lower = key.toLowerCase();
			if (!ciIndex.has(lower)) ciIndex.set(lower, value);
		}
		ciSource = footprints;
	}
	return ciIndex.get(mapObjectId.toLowerCase());
}

export function buildStructureFC(
	structures: BaseStructure[],
	footprints: Record<string, Footprint>,
	baseCampZ: number,
	area: MapArea
): StructureFC {
	const features = structures.map((s) => {
		const fp = lookupFootprint(footprints, s.map_object_id) ?? DEFAULT_STRUCTURE_FOOTPRINT;
		const cos = Math.cos(s.yaw);
		const sin = Math.sin(s.yaw);

		const cx = s.x + fp.ox * s.scale_x * cos - fp.oy * s.scale_y * sin;
		const cy = s.y + fp.ox * s.scale_x * sin + fp.oy * s.scale_y * cos;
		const hx = (fp.sx * s.scale_x) / 2;
		const hy = (fp.sy * s.scale_y) / 2;
		const height = fp.sz * s.scale_z;

		const ring = [
			[-hx, -hy], [hx, -hy], [hx, hy], [-hx, hy], [-hx, -hy]
		].map(([dx, dy]) => {
			const wx = cx + dx * cos - dy * sin;
			const wy = cy + dx * sin + dy * cos;
			const [px, py] = worldToPixel(wx, wy, area);
			return pixelToLngLat(px, py);
		});

		const bottom = s.z + fp.oz * s.scale_z - height / 2;
		const base = Math.max(0, bottom - baseCampZ);

		return {
			type: 'Feature' as const,
			geometry: { type: 'Polygon' as const, coordinates: [ring] },
			properties: {
				key: s.instance_id,
				type: 'structure' as const,
				typeA: fp.typeA,
				b: base,
				h: base + height
			}
		};
	});

	return { type: 'FeatureCollection', features };
}

export function structureCentroid(feature: StructureFeature): [number, number] {
	// The ring is closed: its 5th point duplicates the 1st, so it must be excluded from the mean.
	const ring = feature.geometry.coordinates[0].slice(0, 4);
	let x = 0;
	let y = 0;
	for (const [lng, lat] of ring) {
		x += lng;
		y += lat;
	}
	return [x / ring.length, y / ring.length];
}
