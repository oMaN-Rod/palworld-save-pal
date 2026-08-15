<script lang="ts">
	import type { Map as MLMap } from 'maplibre-gl';
	import type { WebGLRenderer } from 'three';
	import {
		BENCH_STOPS,
		benchStopCenter,
		benchResult,
		medianRenderSample,
		type BenchResult,
		type BenchStop,
		type RendererInfo,
		type RenderSample,
		type StopOutcome
	} from './bench';
	import type { MapArea } from './utils';
	import { peekSharedRenderer } from './structureLayer';

	let { map, area }: { map: MLMap | undefined; area: MapArea } = $props();

	const IDLE_TIMEOUT_MS = 5000;
	const IDLE_SETTLE_MS = 300;
	const SAMPLE_MS = 3000;

	let running = $state(false);
	let results = $state<BenchResult[]>([]);

	function wait(ms: number): Promise<void> {
		return new Promise((resolve) => setTimeout(resolve, ms));
	}

	function flyToStop(instance: MLMap, stop: BenchStop) {
		instance.jumpTo({
			center: benchStopCenter(stop, area),
			zoom: stop.zoom,
			pitch: stop.pitch,
			bearing: stop.bearing
		});
	}

	// A fixed post-jumpTo wait doesn't cover tile loading, and sampling into that
	// window measures load stalls rather than steady-state render. 'idle' is
	// MapLibre's own signal that nothing is left in flight; the timeout keeps a
	// stop that never idles from hanging the run.
	function waitForIdle(instance: MLMap, timeoutMs: number): Promise<StopOutcome> {
		return new Promise((resolve) => {
			let settled = false;
			const onIdle = () => {
				if (settled) return;
				settled = true;
				clearTimeout(timer);
				resolve('settled');
			};
			const timer = setTimeout(() => {
				if (settled) return;
				settled = true;
				instance.off('idle', onIdle);
				resolve('timed-out');
			}, timeoutMs);
			instance.once('idle', onIdle);
		});
	}

	// The shared renderer is set in a custom layer's onAdd, which fires
	// asynchronously and, for structures, only once zoom crosses their minimum.
	// Resolving it once up front would cache null for the whole run even though a
	// layer mounts moments later, so every read below is fresh.
	function ensureAutoResetOff(renderer: WebGLRenderer, tracked: Map<WebGLRenderer, boolean>) {
		if (tracked.has(renderer)) return;
		tracked.set(renderer, renderer.info.autoReset);
		renderer.info.autoReset = false;
	}

	// Reads and resets renderer.info once per animation frame. Several custom
	// layers call renderer.render() within one frame, and with autoReset on three
	// zeroes info.render at the start of each, so a trailing read reflects only
	// the last layer rather than the frame's total. `tracked` remembers every
	// renderer touched so autoReset can be restored at the end. A null
	// lastRenderer means none was ever found, which marks the counters
	// unavailable rather than a false zero.
	//
	// With autoReset off, info.render accumulates until something reads and
	// resets it, so any read not preceded by this function's own reset is
	// contaminated. Two such backlogs are discarded rather than sampled: the gap
	// between stops (hence the reset before the loop, not just inside it), and
	// the first tick of a renderer discovered mid-run.
	function sampleFrames(
		instance: MLMap,
		ms: number,
		tracked: Map<WebGLRenderer, boolean>
	): Promise<{ frameMs: number[]; renderSamples: RenderSample[]; lastRenderer: WebGLRenderer | null }> {
		return new Promise((resolve) => {
			const frameMs: number[] = [];
			const renderSamples: RenderSample[] = [];
			let lastRenderer: WebGLRenderer | null = null;

			const initialRenderer = peekSharedRenderer();
			if (initialRenderer) {
				ensureAutoResetOff(initialRenderer, tracked);
				initialRenderer.info.reset();
				lastRenderer = initialRenderer;
			}

			let previous = performance.now();
			const deadline = previous + ms;
			const tick = () => {
				const now = performance.now();
				frameMs.push(now - previous);
				previous = now;
				const renderer = peekSharedRenderer();
				if (renderer) {
					const justDiscovered = !tracked.has(renderer);
					ensureAutoResetOff(renderer, tracked);
					if (!justDiscovered) {
						renderSamples.push({
							calls: renderer.info.render.calls,
							triangles: renderer.info.render.triangles
						});
					}
					renderer.info.reset();
					lastRenderer = renderer;
				}
				instance.triggerRepaint();
				if (now < deadline) requestAnimationFrame(tick);
				else resolve({ frameMs, renderSamples, lastRenderer });
			};
			requestAnimationFrame(tick);
		});
	}

	async function run() {
		const instance = map;
		if (!instance || running) return;
		running = true;
		results = [];
		const autoResetOriginals = new Map<WebGLRenderer, boolean>();
		try {
			for (const stop of BENCH_STOPS) {
				flyToStop(instance, stop);
				const outcome = await waitForIdle(instance, IDLE_TIMEOUT_MS);
				await wait(IDLE_SETTLE_MS);
				const { frameMs, renderSamples, lastRenderer } = await sampleFrames(
					instance,
					SAMPLE_MS,
					autoResetOriginals
				);
				// Empty even with a lastRenderer if it was only just discovered in this
				// window; that stays unavailable rather than a fabricated zero.
				const info: RendererInfo | null =
					lastRenderer && renderSamples.length > 0
						? {
								render: medianRenderSample(renderSamples),
								memory: {
									geometries: lastRenderer.info.memory.geometries,
									textures: lastRenderer.info.memory.textures
								}
							}
						: null;
				results = [...results, benchResult(stop, frameMs, info, outcome)];
			}
		} finally {
			for (const [renderer, original] of autoResetOriginals) renderer.info.autoReset = original;
			running = false;
		}
		const json = JSON.stringify(results, null, 2);
		console.log(json);
		await navigator.clipboard?.writeText(json).catch(() => {});
	}
</script>

<div class="bg-surface-900/90 absolute bottom-2 left-2 z-50 rounded p-2 font-mono text-xs">
	<button class="btn preset-filled-primary-500" onclick={run} disabled={running}>
		{running ? 'benchmarking…' : 'run bench'}
	</button>
	<div class="mt-1 flex gap-1">
		{#each BENCH_STOPS as stop (stop.name)}
			<button
				class="btn btn-sm preset-tonal-primary"
				onclick={() => map && flyToStop(map, stop)}
				disabled={running}
			>
				{stop.name}
			</button>
		{/each}
	</div>
	{#each results as result (result.stop)}
		<div>
			{result.stop}: {result.fps.toFixed(1)} fps, p95 {result.p95.toFixed(1)} ms,
			{result.renderer === 'measured' ? `${result.draws} draws, ${result.triangles} tris` : 'no renderer'}
			{result.outcome === 'timed-out' ? ' [timed out]' : ''}
		</div>
	{/each}
</div>
