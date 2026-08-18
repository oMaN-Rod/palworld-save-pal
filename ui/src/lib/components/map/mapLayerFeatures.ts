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
} from './iconIds';
import { mapLayerLabel } from './layerPanelModel';
import type { MapLayerId, MapLayerPoint, MapLayerSelection, MapLayerShape } from './layerRegistry';
import { pixelToLngLat } from './mercator';
import { mapOf, worldToPixel, type MapArea } from './utils';

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
	ancient_ruins: 0.6
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

/** Where a point draws on `area`, or null if it draws nowhere. The single test
 *  for "does this entry become a marker", so the panel count and the map cannot
 *  disagree about it. */
function placement(point: MapLayerPoint, area: MapArea): { x: number; y: number } | null {
	const x = coordinate(point.entry?.x);
	const y = coordinate(point.entry?.y);
	if (x === null || y === null) return null;
	return mapOf(x, y) === area ? { x, y } : null;
}

/**
 * Markers this layer contributes to `area`. The panel shows this beside the
 * layer name, so it has to be what the map draws rather than the artifact's row
 * count: skill_fruits carries 188 rows of which 141 are positionless location
 * components, and the rest split across two maps.
 */
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

/**
 * `localized_name` is folded in only where an l10n table exists, so it is
 * present on towers and absent on notes, camps and eggs_spawners. A keyed
 * artifact falls back to its object key, which carries meaning (`Day-01`, a
 * boss battle name); an array artifact's key is a GUID or UAID blob, so it
 * falls back to the layer's own label instead.
 */
export function mapLayerDisplayName(
	point: MapLayerPoint,
	shape: MapLayerShape,
	id: MapLayerId
): string {
	const localized = point.entry?.localized_name;
	if (typeof localized === 'string' && localized !== '') return localized;
	return shape === 'keyed' ? point.key : mapLayerLabel(id);
}

/**
 * Markers for one layer, dropping anything without a usable position. These
 * artifacts are hand-maintained extracts and carry null or missing coordinates;
 * placing those would put a marker at the map origin or feed MapLibre a NaN,
 * which it drops silently rather than reporting.
 */
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
