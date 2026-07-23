import { describe, expect, it } from 'vitest';
import { MAP_SIZE, worldToPixel } from './utils';
import { lngLatToPixel } from './mercator';
import {
	buildBaseRadiusFC,
	buildFastTravelFC,
	buildMapObjectFC,
	buildRelicFC,
	emptyFC
} from './features';
import type { MapObject, MapUnlockPoint, RelicPoint } from '$types';

const TREE_POINT = { x: 512112, y: -510663 };
const MAIN_POINT = { x: -343155, y: 244585 };

describe('emptyFC', () => {
	it('is a valid empty FeatureCollection', () => {
		expect(emptyFC()).toEqual({ type: 'FeatureCollection', features: [] });
	});
});

describe('buildFastTravelFC', () => {
	const points: MapUnlockPoint[] = [
		{ guid: 'A', x: MAIN_POINT.x, y: MAIN_POINT.y, localized_name: 'Main', unlocked: false },
		{ guid: 'B', x: TREE_POINT.x, y: TREE_POINT.y, localized_name: 'Tree', unlocked: true }
	];

	it('keeps only points belonging to the requested area', () => {
		expect(buildFastTravelFC(points, 'MainMap').features).toHaveLength(1);
		expect(buildFastTravelFC(points, 'MainMap').features[0].properties.key).toBe('A');
		expect(buildFastTravelFC(points, 'Tree').features[0].properties.key).toBe('B');
	});

	it('places features at pixelToLngLat(worldToPixel(...))', () => {
		const [feature] = buildFastTravelFC(points, 'MainMap').features;
		const [expectedPx, expectedPy] = worldToPixel(MAIN_POINT.x, MAIN_POINT.y, 'MainMap');
		const [px, py] = lngLatToPixel(...feature.geometry.coordinates);
		expect(px).toBeCloseTo(expectedPx, 3);
		expect(py).toBeCloseTo(expectedPy, 3);
	});

	it('assigns unique numeric ids', () => {
		const ids = buildFastTravelFC(
			[points[0], { ...points[0], guid: 'C' }],
			'MainMap'
		).features.map((f) => f.id);
		expect(new Set(ids).size).toBe(2);
	});

	it('carries only primitive properties', () => {
		for (const feature of buildFastTravelFC(points, 'MainMap').features) {
			for (const value of Object.values(feature.properties)) {
				expect(['string', 'number', 'boolean', 'undefined']).toContain(typeof value);
			}
		}
	});

	it('preserves the undefined unlocked tri-state', () => {
		const noPlayer: MapUnlockPoint[] = [
			{ guid: 'A', x: MAIN_POINT.x, y: MAIN_POINT.y, localized_name: 'Main' }
		];
		expect(buildFastTravelFC(noPlayer, 'MainMap').features[0].properties.locked).toBe(false);
		expect(buildFastTravelFC(points, 'MainMap').features[0].properties.locked).toBe(true);
	});

	it('flags watchtowers by class', () => {
		const towers: MapUnlockPoint[] = [
			{
				guid: 'W',
				x: MAIN_POINT.x,
				y: MAIN_POINT.y,
				localized_name: 'Tower',
				class: 'BP_LevelObject_UnlockMapPoint_C'
			}
		];
		expect(buildFastTravelFC(towers, 'MainMap').features[0].properties.watchtower).toBe(true);
		expect(buildFastTravelFC(points, 'MainMap').features[0].properties.watchtower).toBe(false);
	});
});

describe('buildRelicFC', () => {
	const points: RelicPoint[] = [
		{
			guid: 'R1',
			x: MAIN_POINT.x,
			y: MAIN_POINT.y,
			localized_name: 'Effigy',
			relic_type: 'capture_power',
			unlocked: false
		}
	];

	it('exposes relic_type and collected for expressions', () => {
		const [feature] = buildRelicFC(points, 'MainMap').features;
		expect(feature.properties.relicType).toBe('capture_power');
		expect(feature.properties.collected).toBe(false);
		expect(feature.properties.icon).toBe('relic:capture_power');
	});

	it('treats undefined unlocked as collected, matching relicStyle', () => {
		const noPlayer: RelicPoint[] = [{ ...points[0], unlocked: undefined }];
		expect(buildRelicFC(noPlayer, 'MainMap').features[0].properties.collected).toBe(true);
	});
});

describe('buildMapObjectFC', () => {
	const objects: MapObject[] = [
		{
			x: MAIN_POINT.x,
			y: MAIN_POINT.y,
			type: 'alpha_pal',
			localized_name: 'Chillet',
			pal: 'Chillet'
		}
	];

	it('stamps the requested feature type and a pal icon id', () => {
		const [feature] = buildMapObjectFC(objects, 'alpha_pal', 'MainMap').features;
		expect(feature.properties.type).toBe('alpha_pal');
		expect(feature.properties.icon).toBe('pal:alpha:Chillet');
	});
});

describe('buildBaseRadiusFC', () => {
	it('emits a closed polygon ring sized by area_range', () => {
		const entries = [
			{
				base: {
					id: 'b1',
					area_range: 3500,
					location: { x: MAIN_POINT.x, y: MAIN_POINT.y, z: 0 }
				},
				guildName: 'G'
			}
		];
		const [feature] = buildBaseRadiusFC(entries as never, 'MainMap').features;
		const ring = feature.geometry.coordinates[0];
		expect(ring.length).toBeGreaterThan(3);
		expect(ring[0]).toEqual(ring[ring.length - 1]);
	});
});

describe('area filtering', () => {
	it('drops points that belong to no area', () => {
		const orphan: MapObject[] = [
			{ x: 5_000_000, y: 5_000_000, type: 'dungeon', localized_name: 'X', pal: '' }
		];
		expect(buildMapObjectFC(orphan, 'dungeon', 'MainMap').features).toHaveLength(0);
	});
});

describe('MAP_SIZE sanity', () => {
	it('is the extent the mercator mapping assumes', () => {
		expect(MAP_SIZE).toBe(8192);
	});
});
