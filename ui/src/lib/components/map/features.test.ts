import { describe, expect, it } from 'vitest';
import { MAP_SIZE, worldToPixel } from './utils';
import { lngLatToPixel } from './mercator';
import {
	buildBaseRadiusFC,
	buildFastTravelFC,
	buildMapObjectFC,
	buildRelicFC,
	buildStructureFC,
	emptyFC,
	lookupFootprint,
	structureCentroid,
	type StructureFC
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

const footprint = { sx: 400, sy: 200, sz: 300, ox: 0, oy: 0, oz: 0, typeA: 'Storage' };

const structure = (over = {}) => ({
	instance_id: 'i1', map_object_id: 'Box',
	x: 0, y: 0, z: 1000, yaw: 0,
	scale_x: 1, scale_y: 1, scale_z: 1,
	hp_current: 850, hp_max: 1000, build_player_uid: 'player-1',
	...over
});

describe('buildStructureFC', () => {
	it('emits one closed five-point ring per structure', () => {
		const fc = buildStructureFC([structure()], { Box: footprint }, 1000, 'MainMap');

		expect(fc.features).toHaveLength(1);
		const ring = fc.features[0].geometry.coordinates[0];
		expect(ring).toHaveLength(5);
		expect(ring[0]).toEqual(ring[4]);
	});

	it('anchors height above the base camp z', () => {
		const fc = buildStructureFC([structure({ z: 1150 })], { Box: footprint }, 1000, 'MainMap');

		expect(fc.features[0].properties.b).toBeCloseTo(0, 6);
		expect(fc.features[0].properties.h).toBeCloseTo(300, 6);
	});

	it('lifts a structure stacked above the base camp z', () => {
		const fc = buildStructureFC([structure({ z: 1450 })], { Box: footprint }, 1000, 'MainMap');

		expect(fc.features[0].properties.b).toBeCloseTo(300, 6);
		expect(fc.features[0].properties.h).toBeCloseTo(600, 6);
	});

	it('never emits a negative base', () => {
		const fc = buildStructureFC([structure({ z: -50000 })], { Box: footprint }, 1000, 'MainMap');

		expect(fc.features[0].properties.b).toBe(0);
	});

	it('scales the box by the saved scale', () => {
		const plain = buildStructureFC([structure()], { Box: footprint }, 1000, 'MainMap');
		const doubled = buildStructureFC(
			[structure({ scale_z: 2 })], { Box: footprint }, 1000, 'MainMap'
		);

		expect(doubled.features[0].properties.h - doubled.features[0].properties.b).toBeCloseTo(
			(plain.features[0].properties.h - plain.features[0].properties.b) * 2, 6
		);
	});

	// Spans are measured in PIXELS, not degrees. These structures project to roughly
	// 68 degrees latitude, where a degree of latitude covers far more pixels than a
	// degree of longitude - so comparing raw lng/lat spans would be off by ~2.7x.
	const pixelSpans = (fc: StructureFC) => {
		const pts = fc.features[0].geometry.coordinates[0]
			.slice(0, 4)
			.map(([lng, lat]) => lngLatToPixel(lng, lat));
		const xs = pts.map(([px]) => px);
		const ys = pts.map(([, py]) => py);
		return [Math.max(...xs) - Math.min(...xs), Math.max(...ys) - Math.min(...ys)];
	};

	it('swaps the footprint axes under a quarter turn', () => {
		const [plainX, plainY] = pixelSpans(
			buildStructureFC([structure()], { Box: footprint }, 1000, 'MainMap')
		);
		const [yawedX, yawedY] = pixelSpans(
			buildStructureFC([structure({ yaw: Math.PI / 2 })], { Box: footprint }, 1000, 'MainMap')
		);

		expect(yawedX).toBeCloseTo(plainY, 6);
		expect(yawedY).toBeCloseTo(plainX, 6);
	});

	// Sign check. The axis swap in worldToPixel is a reflection, so a +90 degree world
	// yaw must read as -90 degrees in pixel space. A span comparison alone cannot catch
	// an inverted sign - only the direction the mount offset swings can.
	it('rotates the mount offset in the correct direction', () => {
		const offsetBox = { ...footprint, ox: 100, oy: 0 };
		const centre = (fc: StructureFC) => {
			const pts = fc.features[0].geometry.coordinates[0]
				.slice(0, 4)
				.map(([lng, lat]) => lngLatToPixel(lng, lat));
			return [
				pts.reduce((acc, [px]) => acc + px, 0) / 4,
				pts.reduce((acc, [, py]) => acc + py, 0) / 4
			];
		};

		const [ux, uy] = centre(
			buildStructureFC([structure()], { Box: offsetBox }, 1000, 'MainMap')
		);
		const [rx, ry] = centre(
			buildStructureFC([structure({ yaw: Math.PI / 2 })], { Box: offsetBox }, 1000, 'MainMap')
		);

		expect(rx).toBeGreaterThan(ux);
		expect(ry).toBeLessThan(uy);
	});

	it('falls back to the default box for an unknown map object id', () => {
		const fc = buildStructureFC([structure({ map_object_id: 'Nope' })], {}, 1000, 'MainMap');

		expect(fc.features).toHaveLength(1);
		expect(fc.features[0].properties.typeA).toBe('Other');
	});
});

describe('structureCentroid', () => {
	it('returns the mean of the ring corners, ignoring the closing point', () => {
		const fc = buildStructureFC([structure()], { Box: footprint }, 1000, 'MainMap');
		const [lng, lat] = structureCentroid(fc.features[0]);
		const ring = fc.features[0].geometry.coordinates[0].slice(0, 4);
		const mx = ring.reduce((a, [x]) => a + x, 0) / 4;
		const my = ring.reduce((a, [, y]) => a + y, 0) / 4;
		expect(lng).toBeCloseTo(mx, 12);
		expect(lat).toBeCloseTo(my, 12);
	});

	it('sits inside the footprint it describes', () => {
		const fc = buildStructureFC([structure()], { Box: footprint }, 1000, 'MainMap');
		const ring = fc.features[0].geometry.coordinates[0];
		const [lng, lat] = structureCentroid(fc.features[0]);
		const xs = ring.map(([x]) => x);
		const ys = ring.map(([, y]) => y);
		expect(lng).toBeGreaterThan(Math.min(...xs));
		expect(lng).toBeLessThan(Math.max(...xs));
		expect(lat).toBeGreaterThan(Math.min(...ys));
		expect(lat).toBeLessThan(Math.max(...ys));
	});
});

describe('buildStructureFC identity', () => {
	it('does not assign positional ids, so promoteId can own identity', () => {
		const fc = buildStructureFC([structure(), structure({ instance_id: 'i2' })], { Box: footprint }, 1000, 'MainMap');
		for (const f of fc.features) expect(f.id).toBeUndefined();
	});

	it('keys every feature on its instance id', () => {
		const fc = buildStructureFC(
			[structure({ instance_id: 'aaa' }), structure({ instance_id: 'bbb' })],
			{ Box: footprint },
			1000,
			'MainMap'
		);
		expect(fc.features.map((f) => f.properties.key)).toEqual(['aaa', 'bbb']);
	});
});

describe('lookupFootprint', () => {
	const fp = (sx: number) => ({ sx, sy: 1, sz: 1, ox: 0, oy: 0, oz: 0, typeA: 'Foundation' });
	// Saves spell some ids with different casing than the data table row key.
	const registry = { Stone_foundation: fp(320), Stone_Pillar: fp(50), Stone_pillar: fp(60) };

	it('finds an exact match', () => {
		expect(lookupFootprint(registry, 'Stone_foundation')?.sx).toBe(320);
	});

	it('falls back to a case-insensitive match', () => {
		expect(lookupFootprint(registry, 'Stone_Foundation')?.sx).toBe(320);
	});

	it('prefers the exact match when ids collide only by case', () => {
		expect(lookupFootprint(registry, 'Stone_Pillar')?.sx).toBe(50);
		expect(lookupFootprint(registry, 'Stone_pillar')?.sx).toBe(60);
	});

	it('returns undefined for an unknown id', () => {
		expect(lookupFootprint(registry, 'NotAThing')).toBeUndefined();
	});

	it('rebuilds its index when given a different registry', () => {
		expect(lookupFootprint(registry, 'Stone_Foundation')?.sx).toBe(320);
		expect(lookupFootprint({ Other_thing: fp(7) }, 'OTHER_THING')?.sx).toBe(7);
		expect(lookupFootprint({ Other_thing: fp(7) }, 'Stone_Foundation')).toBeUndefined();
	});
});
