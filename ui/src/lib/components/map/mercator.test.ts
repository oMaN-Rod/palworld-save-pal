import { describe, expect, it } from 'vitest';
import { MAP_SIZE } from './utils';
import { MERCATOR_LAT_LIMIT, lngLatToPixel, pixelCirclePolygon, pixelToLngLat } from './mercator';

describe('pixelToLngLat', () => {
	it('maps the pixel extent onto the whole mercator world', () => {
		expect(pixelToLngLat(0, 0)[0]).toBeCloseTo(-180, 9);
		expect(pixelToLngLat(0, 0)[1]).toBeCloseTo(-MERCATOR_LAT_LIMIT, 6);

		expect(pixelToLngLat(MAP_SIZE, MAP_SIZE)[0]).toBeCloseTo(180, 9);
		expect(pixelToLngLat(MAP_SIZE, MAP_SIZE)[1]).toBeCloseTo(MERCATOR_LAT_LIMIT, 6);
	});

	it('puts the pixel centre at the origin of the world', () => {
		const [lng, lat] = pixelToLngLat(MAP_SIZE / 2, MAP_SIZE / 2);
		expect(lng).toBeCloseTo(0, 9);
		expect(lat).toBeCloseTo(0, 9);
	});

	it('treats py as y-up, matching the OpenLayers pixel extent', () => {
		expect(pixelToLngLat(0, MAP_SIZE)[1]).toBeGreaterThan(0);
		expect(pixelToLngLat(0, 0)[1]).toBeLessThan(0);
	});
});

describe('lngLatToPixel', () => {
	it('round-trips pixelToLngLat', () => {
		const cases: Array<[number, number]> = [
			[0, 0],
			[MAP_SIZE, MAP_SIZE],
			[MAP_SIZE / 2, MAP_SIZE / 2],
			[1234.5, 6789.25],
			[8191, 1]
		];
		for (const [px, py] of cases) {
			const [lng, lat] = pixelToLngLat(px, py);
			const [rx, ry] = lngLatToPixel(lng, lat);
			expect(rx).toBeCloseTo(px, 6);
			expect(ry).toBeCloseTo(py, 6);
		}
	});
});

describe('pixelCirclePolygon', () => {
	it('returns a closed ring', () => {
		const ring = pixelCirclePolygon(4096, 4096, 200, 32);
		expect(ring).toHaveLength(33);
		expect(ring[0][0]).toBeCloseTo(ring[32][0], 12);
		expect(ring[0][1]).toBeCloseTo(ring[32][1], 12);
	});

	it('produces vertices at the requested pixel radius', () => {
		const cx = 4096;
		const cy = 4096;
		const radius = 500;
		for (const [lng, lat] of pixelCirclePolygon(cx, cy, radius, 16)) {
			const [px, py] = lngLatToPixel(lng, lat);
			const dist = Math.hypot(px - cx, py - cy);
			expect(dist).toBeCloseTo(radius, 3);
		}
	});
});
