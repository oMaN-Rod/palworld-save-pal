import { describe, expect, it } from 'vitest';
import { LngLat } from 'maplibre-gl';
import { worldFittingConstrain, type ConstrainTransform } from './constrain';

const FULL_LAT_RANGE: [number, number] = [-85.051129, 85.051129];

function makeTransform(overrides: Partial<ConstrainTransform> = {}): ConstrainTransform {
	return {
		minZoom: 0,
		maxZoom: 7,
		width: 1856,
		height: 889,
		tileSize: 512,
		latRange: FULL_LAT_RANGE,
		...overrides
	};
}

describe('worldFittingConstrain', () => {
	it('fits to viewport height instead of forcing a horizontal zoom-in', () => {
		const tf = makeTransform({ width: 1856, height: 889 });
		const constrain = worldFittingConstrain(tf);
		const result = constrain(new LngLat(0, 0), 0);
		expect(result.zoom).toBeCloseTo(Math.log2(889 / 512), 6);
	});

	it('centres the map horizontally when the world is narrower than the viewport', () => {
		const tf = makeTransform({ width: 1856, height: 889 });
		const constrain = worldFittingConstrain(tf);
		const result = constrain(new LngLat(120, 0), 0);
		expect(result.center.lng).toBeCloseTo(0, 6);
	});

	it('does not zoom out past the height fit', () => {
		const tf = makeTransform({ width: 1856, height: 889 });
		const constrain = worldFittingConstrain(tf);
		const result = constrain(new LngLat(0, 0), -5);
		expect(result.zoom).toBeCloseTo(Math.log2(889 / 512), 6);
	});

	it('clamps horizontal panning when the world is wider than the viewport', () => {
		const tf = makeTransform({ width: 1856, height: 889 });
		const constrain = worldFittingConstrain(tf);
		const zoom = 4;
		const worldSize = tf.tileSize * Math.pow(2, zoom);
		const result = constrain(new LngLat(179.9, 0), zoom);
		expect(worldSize).toBeGreaterThan(tf.width);
		const mercLng = ((result.center.lng + 180) / 360) * worldSize;
		expect(mercLng + tf.width / 2).toBeLessThanOrEqual(worldSize + 1e-6);
	});

	it('clamps vertical panning near the pole when the world is wider than the viewport', () => {
		const tf = makeTransform({ width: 1856, height: 889 });
		const constrain = worldFittingConstrain(tf);
		const result = constrain(new LngLat(0, 85), 4);
		expect(result.center.lat).toBeLessThan(85);
	});

	it('clamps zoom to maxZoom', () => {
		const tf = makeTransform();
		const constrain = worldFittingConstrain(tf);
		const result = constrain(new LngLat(0, 0), 99);
		expect(result.zoom).toBe(7);
	});

	it('does not raise the zoom floor above the height fit on a portrait viewport', () => {
		const tf = makeTransform({ width: 800, height: 1400 });
		const constrain = worldFittingConstrain(tf);
		const result = constrain(new LngLat(0, 0), 0);
		expect(result.zoom).toBeCloseTo(Math.log2(1400 / 512), 4);
	});

	it('centres and clamps horizontally without crashing when latRange is null', () => {
		const tf = makeTransform({ latRange: null });
		const constrain = worldFittingConstrain(tf);
		const result = constrain(new LngLat(0, 0), -5);
		expect(result.zoom).toBeCloseTo(0, 6);
		expect(result.center.lng).toBeCloseTo(0, 6);
	});
});
