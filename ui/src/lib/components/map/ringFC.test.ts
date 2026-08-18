import { describe, expect, it } from 'vitest';
import { buildRingFC, RING_SEGMENTS } from './ringFC';

const item = (x: number, y: number, dim = false) => ({ x, y, dim });

describe('buildRingFC', () => {
	it('emits one polygon per item', () => {
		const fc = buildRingFC([item(0, 0), item(100, 100)], 'MainMap', 180, (i) => ({ dim: i.dim }));
		expect(fc.features).toHaveLength(2);
		expect(fc.features[0].geometry.type).toBe('Polygon');
	});

	it('closes each ring with the configured segment count', () => {
		const fc = buildRingFC([item(0, 0)], 'MainMap', 180, () => ({}));
		const ring = (fc.features[0].geometry as GeoJSON.Polygon).coordinates[0];
		expect(ring.length).toBeGreaterThanOrEqual(RING_SEGMENTS);
		expect(ring[0]).toEqual(ring[ring.length - 1]);
	});

	it('carries the caller-s properties onto each feature', () => {
		const fc = buildRingFC([item(0, 0, true)], 'MainMap', 180, (i) => ({ dim: i.dim }));
		expect(fc.features[0].properties).toEqual({ dim: true });
	});

	it('grows the ring with the radius', () => {
		const small = buildRingFC([item(0, 0)], 'MainMap', 100, () => ({}));
		const large = buildRingFC([item(0, 0)], 'MainMap', 400, () => ({}));
		const span = (fc: GeoJSON.FeatureCollection) => {
			const ring = (fc.features[0].geometry as GeoJSON.Polygon).coordinates[0];
			return Math.max(...ring.map((c) => c[0])) - Math.min(...ring.map((c) => c[0]));
		};
		expect(span(large)).toBeGreaterThan(span(small));
	});

	it('handles an empty list', () => {
		expect(buildRingFC([], 'MainMap', 180, () => ({})).features).toEqual([]);
	});
});
