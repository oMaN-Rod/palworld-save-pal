import type { FastTravelPoint, MapUnlockPoint, Relic, RelicPoint } from '$types';
import { describe, expect, it } from 'vitest';
import { WATCHTOWER_CLASS } from './fastTravel';
import {
	buildFastTravelRingFC,
	buildMapObjectItems,
	buildRelicRingFC,
	fastTravelStateOf,
	relicStateOf
} from './mapObjectItems';
import {
	FAST_TRAVEL_RADIUS_CM,
	fastTravelPortalColor,
	RELIC_RADIUS_CM,
	relicPortalColor
} from './mapObjectPortal';
import { lngLatToPixel } from './mercator';
import { cmPerPx, worldToPixel } from './utils';

const FT_POINT: MapUnlockPoint = { guid: 'ft1', x: 100, y: 200, z: 500, localized_name: 'Statue' };
const FT_SOURCE: FastTravelPoint = {
	class: 'BP_LevelObject_TowerFastTravelPoint_C',
	x: 100,
	y: 200,
	z: 500,
	id: 'ft1'
};

const WT_POINT: MapUnlockPoint = {
	guid: 'wt1',
	x: 500,
	y: 600,
	z: 700,
	localized_name: 'Tower',
	class: WATCHTOWER_CLASS
};
const WT_SOURCE: FastTravelPoint = {
	class: WATCHTOWER_CLASS,
	x: 500,
	y: 600,
	z: 700,
	id: 'wt1'
};

const RELIC_POINT: RelicPoint = {
	guid: 'r1',
	x: 300,
	y: 400,
	z: 0,
	localized_name: 'Relic',
	relic_type: 'jump_power'
};
const RELIC_SOURCE: Relic = {
	class: 'BP_LevelObject_Relic_FlameBambi_C',
	x: 300,
	y: 400,
	z: 600,
	rot: [0, 50, 0],
	relic_type: 'jump_power'
};

describe('buildMapObjectItems', () => {
	it('returns nothing when show3d is off', () => {
		const items = buildMapObjectItems(
			false,
			{ points: [FT_POINT], sources: { ft1: FT_SOURCE }, size: 1, watchtowerSize: 1 },
			{ show: true, points: [RELIC_POINT], sources: { r1: RELIC_SOURCE }, size: 1 }
		);
		expect(items).toEqual([]);
	});

	it("carries the relic source's own rotation, not a zero placeholder", () => {
		const items = buildMapObjectItems(
			true,
			{ points: [], sources: {}, size: 1, watchtowerSize: 1 },
			{ show: true, points: [RELIC_POINT], sources: { r1: RELIC_SOURCE }, size: 1 }
		);
		expect(items).toHaveLength(1);
		expect(items[0].rot).toEqual([0, 50, 0]);
	});

	it('carries [0, 0, 0] for a fast travel item, which has no rotation data', () => {
		const items = buildMapObjectItems(
			true,
			{ points: [FT_POINT], sources: { ft1: FT_SOURCE }, size: 1, watchtowerSize: 1 },
			{ show: false, points: [], sources: {}, size: 1 }
		);
		expect(items).toHaveLength(1);
		expect(items[0].rot).toEqual([0, 0, 0]);
	});

	it('scales a fast travel item by fastTravel.size, not relics.size', () => {
		const items = buildMapObjectItems(
			true,
			{ points: [FT_POINT], sources: { ft1: FT_SOURCE }, size: 3, watchtowerSize: 9 },
			{ show: true, points: [RELIC_POINT], sources: { r1: RELIC_SOURCE }, size: 7 }
		);
		const ft = items.find((i) => i.actorClass === FT_SOURCE.class);
		expect(ft?.scale).toBe(3);
	});

	it('scales a watchtower item by fastTravel.watchtowerSize, not fastTravel.size', () => {
		const items = buildMapObjectItems(
			true,
			{
				points: [FT_POINT, WT_POINT],
				sources: { ft1: FT_SOURCE, wt1: WT_SOURCE },
				size: 3,
				watchtowerSize: 9
			},
			{ show: false, points: [], sources: {}, size: 1 }
		);
		const watchtower = items.find((i) => i.actorClass === WATCHTOWER_CLASS);
		expect(watchtower?.scale).toBe(9);
	});

	it('leaves a statue on fastTravel.size when a watchtower is sized separately', () => {
		const items = buildMapObjectItems(
			true,
			{
				points: [FT_POINT, WT_POINT],
				sources: { ft1: FT_SOURCE, wt1: WT_SOURCE },
				size: 3,
				watchtowerSize: 9
			},
			{ show: false, points: [], sources: {}, size: 1 }
		);
		const statue = items.find((i) => i.actorClass === FT_SOURCE.class);
		expect(statue?.scale).toBe(3);
	});

	it('scales a relic item by relics.size, not fastTravel.size', () => {
		const items = buildMapObjectItems(
			true,
			{ points: [FT_POINT], sources: { ft1: FT_SOURCE }, size: 3, watchtowerSize: 3 },
			{ show: true, points: [RELIC_POINT], sources: { r1: RELIC_SOURCE }, size: 7 }
		);
		const relic = items.find((i) => i.actorClass === RELIC_SOURCE.class);
		expect(relic?.scale).toBe(7);
	});

	it("carries world x/y from the point and z/actorClass from that point's source", () => {
		const items = buildMapObjectItems(
			true,
			{ points: [FT_POINT], sources: { ft1: FT_SOURCE }, size: 1, watchtowerSize: 1 },
			{ show: false, points: [], sources: {}, size: 1 }
		);
		expect(items[0]).toMatchObject({
			x: FT_POINT.x,
			y: FT_POINT.y,
			z: FT_SOURCE.z,
			actorClass: FT_SOURCE.class
		});
	});

	it('colours a locked fast travel item from the fast-travel palette, not the relic palette', () => {
		const items = buildMapObjectItems(
			true,
			{
				points: [{ ...FT_POINT, unlocked: false }],
				sources: { ft1: FT_SOURCE },
				size: 1,
				watchtowerSize: 1
			},
			{ show: false, points: [], sources: {}, size: 1 }
		);
		expect(items[0].portalColor).toBe(`#${fastTravelPortalColor('locked').getHexString()}`);
	});

	it('colours an uncollected relic item from the relic palette, not the fast-travel palette', () => {
		const items = buildMapObjectItems(
			true,
			{ points: [], sources: {}, size: 1, watchtowerSize: 1 },
			{
				show: true,
				points: [{ ...RELIC_POINT, unlocked: false }],
				sources: { r1: RELIC_SOURCE },
				size: 1
			}
		);
		expect(items[0].portalColor).toBe(`#${relicPortalColor('uncollected').getHexString()}`);
	});

	it('skips a point whose source record is missing rather than throwing', () => {
		const items = buildMapObjectItems(
			true,
			{ points: [FT_POINT], sources: {}, size: 1, watchtowerSize: 1 },
			{ show: false, points: [], sources: {}, size: 1 }
		);
		expect(items).toEqual([]);
	});

	it('omits relics entirely when relics.show is false, even with data present', () => {
		const items = buildMapObjectItems(
			true,
			{ points: [], sources: {}, size: 1, watchtowerSize: 1 },
			{ show: false, points: [RELIC_POINT], sources: { r1: RELIC_SOURCE }, size: 1 }
		);
		expect(items).toEqual([]);
	});

	it("gives a fast travel item its own ring radius, not relic's", () => {
		const items = buildMapObjectItems(
			true,
			{ points: [FT_POINT], sources: { ft1: FT_SOURCE }, size: 1, watchtowerSize: 1 },
			{ show: false, points: [], sources: {}, size: 1 }
		);
		expect(items[0].ringRadiusCm).toBe(FAST_TRAVEL_RADIUS_CM);
	});

	it("gives a relic item its own ring radius, not fast travel's", () => {
		const items = buildMapObjectItems(
			true,
			{ points: [], sources: {}, size: 1, watchtowerSize: 1 },
			{ show: true, points: [RELIC_POINT], sources: { r1: RELIC_SOURCE }, size: 1 }
		);
		expect(items[0].ringRadiusCm).toBe(RELIC_RADIUS_CM);
	});
});

describe('buildFastTravelRingFC', () => {
	function ringRadiusPx(fc: GeoJSON.FeatureCollection, index: number, point: MapUnlockPoint) {
		const ring = (fc.features[index].geometry as GeoJSON.Polygon).coordinates[0];
		const [cx, cy] = worldToPixel(point.x, point.y, 'MainMap');
		const [lng, lat] = ring[0];
		const [vx, vy] = lngLatToPixel(lng, lat);
		return Math.hypot(vx - cx, vy - cy);
	}

	it('sizes the ring to FAST_TRAVEL_RADIUS_CM * scale, not RELIC_RADIUS_CM', () => {
		const fc = buildFastTravelRingFC([FT_POINT], 'MainMap', 2, 5);
		const ring = (fc.features[0].geometry as GeoJSON.Polygon).coordinates[0];
		const [cx, cy] = worldToPixel(FT_POINT.x, FT_POINT.y, 'MainMap');
		const expectedRadiusPx = (FAST_TRAVEL_RADIUS_CM * 2) / cmPerPx('MainMap');
		for (const [lng, lat] of ring) {
			const [vx, vy] = lngLatToPixel(lng, lat);
			expect(Math.hypot(vx - cx, vy - cy)).toBeCloseTo(expectedRadiusPx, 6);
		}
	});

	it('sizes a watchtower ring by the watchtower scale, leaving a statue on its own', () => {
		const fc = buildFastTravelRingFC([FT_POINT, WT_POINT], 'MainMap', 2, 5);
		expect(ringRadiusPx(fc, 1, WT_POINT)).toBeCloseTo(
			(FAST_TRAVEL_RADIUS_CM * 5) / cmPerPx('MainMap'),
			6
		);
		expect(ringRadiusPx(fc, 0, FT_POINT)).toBeCloseTo(
			(FAST_TRAVEL_RADIUS_CM * 2) / cmPerPx('MainMap'),
			6
		);
	});
});

describe('buildRelicRingFC', () => {
	it('sizes the ring to RELIC_RADIUS_CM * scale, not FAST_TRAVEL_RADIUS_CM', () => {
		const fc = buildRelicRingFC([RELIC_POINT], 'MainMap', 2);
		const ring = (fc.features[0].geometry as GeoJSON.Polygon).coordinates[0];
		const [cx, cy] = worldToPixel(RELIC_POINT.x, RELIC_POINT.y, 'MainMap');
		const expectedRadiusPx = (RELIC_RADIUS_CM * 2) / cmPerPx('MainMap');
		for (const [lng, lat] of ring) {
			const [vx, vy] = lngLatToPixel(lng, lat);
			expect(Math.hypot(vx - cx, vy - cy)).toBeCloseTo(expectedRadiusPx, 6);
		}
	});
});

describe('fastTravelStateOf', () => {
	it('is unknown when unlocked is undefined', () => {
		expect(fastTravelStateOf({})).toBe('unknown');
	});

	it('is locked/unlocked otherwise', () => {
		expect(fastTravelStateOf({ unlocked: false })).toBe('locked');
		expect(fastTravelStateOf({ unlocked: true })).toBe('unlocked');
	});
});

describe('relicStateOf', () => {
	it('is unknown when unlocked is undefined', () => {
		expect(relicStateOf({})).toBe('unknown');
	});

	it('is uncollected/collected otherwise', () => {
		expect(relicStateOf({ unlocked: false })).toBe('uncollected');
		expect(relicStateOf({ unlocked: true })).toBe('collected');
	});
});
