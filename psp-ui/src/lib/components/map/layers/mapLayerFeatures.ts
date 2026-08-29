import {
	ICON_ANCIENT_RUIN,
	ICON_CAMP,
	ICON_DUNGEON,
	ICON_EGG,
	ICON_FAST_TRAVEL,
	ICON_JOURNAL,
	ICON_KINSHIP_PEACH,
	ICON_SKILL_FRUIT,
	ICON_TOWER_BOSS,
	ICON_WATCHTOWER
} from '../style/iconIds';
import { mapLayerLabel } from './layerPanelModel';
import type { MapLayerId, MapLayerPoint, MapLayerSelection, MapLayerShape } from './layerRegistry';
import { pixelToLngLat } from '../geo/mercator';
import { mapOf, worldToPixel, type MapArea } from '../geo/utils';

export type MapLayerFeatureProps = {
	key: string;
	type: 'map_layer';
	layer: MapLayerId;
	name: string;
	icon: string;
};

export type MapLayerFeature = {
	type: 'Feature';
	id: number;
	geometry: { type: 'Point'; coordinates: [number, number] };
	properties: MapLayerFeatureProps;
};

export type MapLayerFC = { type: 'FeatureCollection'; features: MapLayerFeature[] };

const ICONS: Partial<Record<MapLayerId, string>> = {
	fast_travel: ICON_FAST_TRAVEL,
	watchtower: ICON_WATCHTOWER,
	tower_boss: ICON_TOWER_BOSS,
	dungeons: ICON_DUNGEON,
	eggs: ICON_EGG,
	journals: ICON_JOURNAL,
	camps: ICON_CAMP,
	skill_fruits: ICON_SKILL_FRUIT,
	kinship_peach: ICON_KINSHIP_PEACH,
	ancient_ruins: ICON_ANCIENT_RUIN
};

export function mapLayerIcon(id: MapLayerId): string {
	return ICONS[id] ?? ICON_DUNGEON;
}

const ICON_SCALES: Partial<Record<MapLayerId, number>> = {
	journals: 0.25,
	kinship_peach: 0.25,
	ancient_ruins: 0.6,
};

export function mapLayerIconScale(id: MapLayerId): number {
	return ICON_SCALES[id] ?? 1;
}

export function emptyMapLayerFC(): MapLayerFC {
	return { type: 'FeatureCollection', features: [] };
}

function coordinate(value: unknown): number | null {
	return typeof value === 'number' && Number.isFinite(value) ? value : null;
}

function placement(point: MapLayerPoint, area: MapArea): { x: number; y: number } | null {
	const x = coordinate(point.entry?.x);
	const y = coordinate(point.entry?.y);
	if (x === null || y === null) return null;
	return mapOf(x, y) === area ? { x, y } : null;
}

export function mapLayerMarkerCount(
	_id: MapLayerId,
	selection: MapLayerSelection | undefined,
	area: MapArea
): number {
	if (!selection) return 0;
	let count = 0;
	for (const point of selection.points) if (placement(point, area)) count += 1;
	return count;
}

export function mapLayerDisplayName(
	point: MapLayerPoint,
	shape: MapLayerShape,
	id: MapLayerId
): string {
	const localized = point.entry?.localized_name;
	if (typeof localized === 'string' && localized !== '') return localized;
	return shape === 'keyed' ? point.key : mapLayerLabel(id);
}

// Drops anything without a usable position: these artifacts carry null or missing
// coordinates, and feeding MapLibre a NaN gets dropped silently rather than reported.
export function buildMapLayerFC(
	id: MapLayerId,
	selection: MapLayerSelection | undefined,
	area: MapArea
): MapLayerFC {
	if (!selection) return emptyMapLayerFC();
	const icon = mapLayerIcon(id);
	const features: MapLayerFeature[] = [];
	for (const point of selection.points) {
		const placed = placement(point, area);
		if (!placed) continue;
		const { x, y } = placed;
		const [px, py] = worldToPixel(x, y, area);
		features.push({
			type: 'Feature',
			id: features.length,
			geometry: { type: 'Point', coordinates: pixelToLngLat(px, py) },
			properties: {
				key: `${id}:${point.key}`,
				type: 'map_layer',
				layer: id,
				name: mapLayerDisplayName(point, selection.shape, id),
				icon
			}
		});
	}
	return { type: 'FeatureCollection', features };
}
