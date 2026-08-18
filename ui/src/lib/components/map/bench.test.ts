import { describe, expect, it } from 'vitest';
import {
	BENCH_STOPS,
	benchResult,
	benchStopCenter,
	medianRenderSample,
	summarizeFrames
} from './bench';

describe('BENCH_STOPS', () => {
	it('names every stop uniquely', () => {
		const names = BENCH_STOPS.map((s) => s.name);
		expect(new Set(names).size).toBe(names.length);
	});

	it('covers a flat overview, the reference mountain framing, and a ground-level view', () => {
		expect(BENCH_STOPS.map((s) => s.name)).toEqual(['overview', 'mountain', 'horizon', 'ground']);
	});

	it('holds the mountain stop at the reference camera', () => {
		const mountain = BENCH_STOPS.find((s) => s.name === 'mountain')!;
		expect(mountain.zoom).toBe(4);
		expect(mountain.pitch).toBe(72.5);
		expect(mountain.bearing).toBe(0);
	});
});

describe('benchStopCenter', () => {
	it('converts a stop world position to lng/lat', () => {
		const [lng, lat] = benchStopCenter(BENCH_STOPS[0], 'MainMap');
		expect(Number.isFinite(lng)).toBe(true);
		expect(Number.isFinite(lat)).toBe(true);
		expect(Math.abs(lat)).toBeLessThan(85.1);
	});

	it('is stable for a given stop and area', () => {
		expect(benchStopCenter(BENCH_STOPS[1], 'MainMap')).toEqual(
			benchStopCenter(BENCH_STOPS[1], 'MainMap')
		);
	});
});

describe('summarizeFrames', () => {
	it('reduces frame durations to fps and percentiles', () => {
		const frames = [10, 10, 10, 10, 20];
		const s = summarizeFrames(frames);
		expect(s.p50).toBe(10);
		expect(s.p95).toBe(20);
		expect(s.fps).toBeCloseTo(1000 / 12, 5);
	});

	it('reports zeroes for an empty sample rather than NaN', () => {
		expect(summarizeFrames([])).toEqual({ fps: 0, p50: 0, p95: 0 });
	});
});

describe('medianRenderSample', () => {
	it('reduces per-frame render samples to their median calls and triangles', () => {
		const samples = [
			{ calls: 1, triangles: 100 },
			{ calls: 5, triangles: 10 },
			{ calls: 3, triangles: 50 }
		];
		expect(medianRenderSample(samples)).toEqual({ calls: 3, triangles: 50 });
	});

	it('is not skewed by a single anomalous frame', () => {
		const samples = [
			{ calls: 40, triangles: 4000 },
			{ calls: 42, triangles: 4200 },
			{ calls: 0, triangles: 0 },
			{ calls: 41, triangles: 4100 },
			{ calls: 43, triangles: 4300 }
		];
		expect(medianRenderSample(samples)).toEqual({ calls: 41, triangles: 4100 });
	});

	it('reports zeroes for an empty sample rather than NaN', () => {
		expect(medianRenderSample([])).toEqual({ calls: 0, triangles: 0 });
	});

	// At two samples floor(0.5*2) = 1 picks the larger of the pair, not an average,
	// so one contaminated leading sample becomes "the median". Callers must never
	// feed this a window that can shrink to two.
	it('resolves two samples to the larger one, not an average', () => {
		const samples = [
			{ calls: 10, triangles: 100 },
			{ calls: 999, triangles: 9999 }
		];
		expect(medianRenderSample(samples)).toEqual({ calls: 999, triangles: 9999 });
	});
});

describe('benchResult', () => {
	it('merges the stop name, frame summary and renderer counters', () => {
		const result = benchResult(BENCH_STOPS[0], [10, 10], {
			render: { calls: 42, triangles: 1234 },
			memory: { geometries: 7, textures: 3 }
		});
		expect(result).toEqual({
			stop: 'overview',
			outcome: 'settled',
			renderer: 'measured',
			fps: 100,
			p50: 10,
			p95: 10,
			draws: 42,
			triangles: 1234,
			geometries: 7,
			textures: 3
		});
	});

	it('carries a timed-out outcome through when the stop never idled', () => {
		const result = benchResult(
			BENCH_STOPS[0],
			[10, 10],
			{ render: { calls: 0, triangles: 0 }, memory: { geometries: 0, textures: 0 } },
			'timed-out'
		);
		expect(result.outcome).toBe('timed-out');
	});

	it('marks the renderer unavailable and nulls the counters rather than reporting a false zero', () => {
		const result = benchResult(BENCH_STOPS[0], [10, 10], null);
		expect(result.renderer).toBe('unavailable');
		expect(result.draws).toBeNull();
		expect(result.triangles).toBeNull();
		expect(result.geometries).toBeNull();
		expect(result.textures).toBeNull();
		// Frame timing comes from performance.now(), independent of the renderer --
		// a missing renderer must not blank out an otherwise valid fps reading.
		expect(result.fps).toBe(100);
	});
});
