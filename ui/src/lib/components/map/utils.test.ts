import { describe, expect, it } from 'vitest';
import {
	MAP_AREAS,
	MAP_SIZE,
	cmPerPx,
	mapOf,
	mapToWorld,
	pixelToGameCoords,
	pixelToWorld,
	worldToMap,
	worldToPixel
} from './utils';

describe('cmPerPx', () => {
	it('derives the scale from the DataTable bounds', () => {
		expect(cmPerPx('MainMap')).toBeCloseTo(176.85546875, 6);
		expect(cmPerPx('Tree')).toBeCloseTo(41.723266, 5);
	});
});

describe('worldToPixel', () => {
	it('maps each area corner to the extent corner', () => {
		for (const area of ['MainMap', 'Tree'] as const) {
			const { min, max } = MAP_AREAS[area];
			expect(worldToPixel(min.x, min.y, area)).toEqual([0, 0]);
			const [px, py] = worldToPixel(max.x, max.y, area);
			expect(px).toBeCloseTo(MAP_SIZE, 4);
			expect(py).toBeCloseTo(MAP_SIZE, 4);
		}
	});

	// Landmarks confirmed against the real 8192x8192 textures (Phase 0, Task 1).
	it('places World Tree fast-travel statues on their landmarks', () => {
		const [ax, ay] = worldToPixel(512112, -510663, 'Tree'); // WorldTree_A
		expect(ax).toBeCloseTo(7370.8, 1);
		expect(ay).toBeCloseTo(3948.89, 1);

		const [bx, by] = worldToPixel(501010, -748555, 'Tree'); // WorldTree_LastBoss
		expect(bx).toBeCloseTo(1669.14, 1);
		expect(by).toBeCloseTo(3682.8, 1);
	});
});

describe('pixelToWorld', () => {
	it('round-trips worldToPixel', () => {
		const cases: Array<[number, number, 'MainMap' | 'Tree']> = [
			[0, 0, 'MainMap'],
			[-343155, 244585, 'MainMap'],
			[512112, -510663, 'Tree']
		];
		for (const [x, y, area] of cases) {
			const [px, py] = worldToPixel(x, y, area);
			const { worldX, worldY } = pixelToWorld(px, py, area);
			expect(worldX).toBeCloseTo(x, 3);
			expect(worldY).toBeCloseTo(y, 3);
		}
	});
});

describe('worldToMap', () => {
	it('reports the in-game coordinates of the origin', () => {
		expect(worldToMap(0, 0)).toEqual({ x: -344, y: -270 });
	});

	it('round-trips mapToWorld', () => {
		const { x, y } = worldToMap(-343155, 244585);
		const world = mapToWorld(x, y);
		expect(worldToMap(world.x, world.y)).toEqual({ x, y });
	});
});

describe('pixelToGameCoords', () => {
	// The readout in the map corner must agree with every pin tooltip, which all
	// render `worldToMap(x, y).y * -1`.
	it('agrees with the tooltips for the same world position', () => {
		const cases: Array<[number, number, 'MainMap' | 'Tree']> = [
			[0, 0, 'MainMap'],
			[-343155, 244585, 'MainMap'],
			[-867560.9, -441338.22, 'MainMap'],
			[512112, -510663, 'Tree'],
			[621850, -742575, 'Tree']
		];
		for (const [x, y, area] of cases) {
			const [px, py] = worldToPixel(x, y, area);
			const readout = pixelToGameCoords(px, py, area);
			const tooltip = worldToMap(x, y);
			expect(readout.gameX).toBe(tooltip.x);
			expect(readout.gameY).toBe(tooltip.y * -1);
		}
	});
});

describe('mapOf', () => {
	it('puts the origin and known alphas on MainMap', () => {
		expect(mapOf(0, 0)).toBe('MainMap');
		expect(mapOf(-343155, 244585)).toBe('MainMap');
	});

	it('puts World Tree content on Tree', () => {
		expect(mapOf(512112, -510663)).toBe('Tree');
		expect(mapOf(621850, -742575)).toBe('Tree');
	});

	it('gives Tree priority in the overlapping sliver', () => {
		// X 347351.5..349400 x Y -724400..-476400 is inside BOTH rectangles.
		// The game breaks the tie with WorldMapPriority: Tree (1) wins.
		expect(mapOf(348000, -600000)).toBe('Tree');
	});

	it('returns null outside every area', () => {
		expect(mapOf(5_000_000, 5_000_000)).toBeNull();
	});
});
