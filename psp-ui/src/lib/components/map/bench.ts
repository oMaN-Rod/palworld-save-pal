import { worldToPixel, type MapArea } from './utils';
import { pixelToLngLat } from './mercator';

export type BenchStop = {
	name: string;
	worldX: number;
	worldY: number;
	zoom: number;
	pitch: number;
	bearing: number;
};

export type FrameSummary = { fps: number; p50: number; p95: number };

export type RenderSample = { calls: number; triangles: number };

export type RendererInfo = {
	render: RenderSample;
	memory: { geometries: number; textures: number };
};

export type StopOutcome = 'settled' | 'timed-out';

// 'unavailable' counts stay null rather than 0: a renderer reporting zero draws and a
// missing renderer are both falsy but mean opposite things.
export type RendererStatus = 'measured' | 'unavailable';

export type BenchResult = FrameSummary & {
	stop: string;
	outcome: StopOutcome;
	renderer: RendererStatus;
	draws: number | null;
	triangles: number | null;
	geometries: number | null;
	textures: number | null;
};

export const BENCH_STOPS: BenchStop[] = [
	{ name: 'overview', worldX: 0, worldY: 0, zoom: 1.5, pitch: 0, bearing: 0 },
	{ name: 'mountain', worldX: -328310, worldY: 150504, zoom: 4, pitch: 72.5, bearing: 0 },
	{ name: 'horizon', worldX: -328310, worldY: 150504, zoom: 3, pitch: 80, bearing: 0 },
	{ name: 'ground', worldX: -328310, worldY: 150504, zoom: 9, pitch: 80, bearing: 0 }
];

export function benchStopCenter(stop: BenchStop, area: MapArea): [number, number] {
	const [px, py] = worldToPixel(stop.worldX, stop.worldY, area);
	return pixelToLngLat(px, py);
}

function percentile(sorted: number[], fraction: number): number {
	const index = Math.min(sorted.length - 1, Math.floor(fraction * sorted.length));
	return sorted[index];
}

export function summarizeFrames(frameMs: number[]): FrameSummary {
	if (frameMs.length === 0) return { fps: 0, p50: 0, p95: 0 };
	const sorted = [...frameMs].sort((a, b) => a - b);
	const mean = frameMs.reduce((sum, ms) => sum + ms, 0) / frameMs.length;
	return { fps: 1000 / mean, p50: percentile(sorted, 0.5), p95: percentile(sorted, 0.95) };
}

function medianOf(values: number[]): number {
	const sorted = [...values].sort((a, b) => a - b);
	return percentile(sorted, 0.5);
}

// Several layers call renderer.render() within one map frame, so a single
// trailing read of info.render reflects only whichever rendered last. Callers
// reset once per frame and sample per frame; the median rather than the mean
// keeps one spiked frame from defining the reported counts.
export function medianRenderSample(samples: RenderSample[]): RenderSample {
	if (samples.length === 0) return { calls: 0, triangles: 0 };
	return {
		calls: medianOf(samples.map((s) => s.calls)),
		triangles: medianOf(samples.map((s) => s.triangles))
	};
}

export function benchResult(
	stop: BenchStop,
	frameMs: number[],
	info: RendererInfo | null,
	outcome: StopOutcome = 'settled'
): BenchResult {
	return {
		stop: stop.name,
		outcome,
		renderer: info ? 'measured' : 'unavailable',
		...summarizeFrames(frameMs),
		draws: info ? info.render.calls : null,
		triangles: info ? info.render.triangles : null,
		geometries: info ? info.memory.geometries : null,
		textures: info ? info.memory.textures : null
	};
}
