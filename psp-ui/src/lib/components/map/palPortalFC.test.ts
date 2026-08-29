import { describe, it, expect } from 'vitest';
import { buildPalPortalFC } from './palPortalFC';
import { cmPerPx, worldToPixel } from './utils';
import { lngLatToPixel } from './mercator';
import { PORTAL_RADIUS_CM } from './palPortal';
import type { PalBoss, PalPredator } from './palLayer';

const PAL_SCALE = 30;

const boss = (x: number, y: number, defeated: boolean): PalBoss => ({
	key: 'anubis',
	x,
	y,
	z: 0,
	defeated
});

const predator = (x: number, y: number): PalPredator => ({ key: 'sifudog', x, y, z: 0 });

describe('buildPalPortalFC', () => {
	it('emits one closed polygon per boss', () => {
		const fc = buildPalPortalFC(
			[boss(-400000, -300000, false), boss(-401000, -300000, true)],
			[],
			'MainMap',
			PAL_SCALE
		);
		expect(fc.type).toBe('FeatureCollection');
		expect(fc.features.length).toBe(2);
		for (const f of fc.features) {
			expect(f.geometry.type).toBe('Polygon');
			const ring = (f.geometry as GeoJSON.Polygon).coordinates[0];
			expect(ring.length).toBeGreaterThan(8);
			expect(ring[0]).toEqual(ring[ring.length - 1]);
		}
	});

	it('carries the defeated flag per feature so paint can be data-driven', () => {
		const fc = buildPalPortalFC(
			[boss(-400000, -300000, false), boss(-401000, -300000, true)],
			[],
			'MainMap',
			PAL_SCALE
		);
		expect(fc.features[0].properties?.defeated).toBe(false);
		expect(fc.features[1].properties?.defeated).toBe(true);
	});

	it('centres each ring on its boss', () => {
		const fc = buildPalPortalFC([boss(-400000, -300000, false)], [], 'MainMap', PAL_SCALE);
		const ring = (fc.features[0].geometry as GeoJSON.Polygon).coordinates[0];
		// A lng/lat bounding-box midpoint is not the true centre: Mercator's y is
		// nonlinear, so degree-space min/max drifts by more than float noise.
		// Round-tripping to pixel space, where the ring was built, recovers it exactly.
		const pxs = ring.map((c) => lngLatToPixel(c[0], c[1])[0]);
		const pys = ring.map((c) => lngLatToPixel(c[0], c[1])[1]);
		const cx = (Math.min(...pxs) + Math.max(...pxs)) / 2;
		const cy = (Math.min(...pys) + Math.max(...pys)) / 2;
		const [expectedPx, expectedPy] = worldToPixel(-400000, -300000, 'MainMap');
		expect(cx).toBeCloseTo(expectedPx, 6);
		expect(cy).toBeCloseTo(expectedPy, 6);
	});

	it('sizes the ring to PORTAL_RADIUS_CM * PAL_SCALE converted to pixels', () => {
		const fc = buildPalPortalFC([boss(-400000, -300000, false)], [], 'MainMap', PAL_SCALE);
		const ring = (fc.features[0].geometry as GeoJSON.Polygon).coordinates[0];
		// Same reasoning as the centring test: measure in pixel space, where the
		// ring is an exact circle, not lng/lat, where Mercator would distort it.
		const [cx, cy] = worldToPixel(-400000, -300000, 'MainMap');
		const expectedRadiusPx = (PORTAL_RADIUS_CM * PAL_SCALE) / cmPerPx('MainMap');
		for (const [lng, lat] of ring) {
			const [vx, vy] = lngLatToPixel(lng, lat);
			const radiusPx = Math.hypot(vx - cx, vy - cy);
			expect(radiusPx).toBeCloseTo(expectedRadiusPx, 6);
		}
	});

	it('returns an empty collection for no bosses', () => {
		expect(buildPalPortalFC([], [], 'MainMap', PAL_SCALE).features.length).toBe(0);
	});

	it('emits a ring for a predator too, not just bosses', () => {
		const fc = buildPalPortalFC([], [predator(-400000, -300000)], 'MainMap', PAL_SCALE);
		expect(fc.features.length).toBe(1);
		expect(fc.features[0].properties?.state).toBe('predator');
	});

	it('tags a boss feature with state "boss" and a predator feature with state "predator"', () => {
		const fc = buildPalPortalFC(
			[boss(-400000, -300000, false)],
			[predator(-401000, -300000)],
			'MainMap',
			PAL_SCALE
		);
		const states = fc.features.map((f) => f.properties?.state);
		expect(states.sort()).toEqual(['boss', 'predator']);
	});

	it('always marks a predator feature as not defeated', () => {
		const fc = buildPalPortalFC([], [predator(-400000, -300000)], 'MainMap', PAL_SCALE);
		expect(fc.features[0].properties?.defeated).toBe(false);
	});
});
