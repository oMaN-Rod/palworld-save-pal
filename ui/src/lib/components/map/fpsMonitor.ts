// Counts MapLibre paint frames to report the 3D map's real render rate.
//
// The map renders on demand, so a plain requestAnimationFrame ticker would
// read the display's refresh rate while the map sits idle. Counting 'render'
// events instead measures exactly the frames the renderer (base map plus the
// three.js custom layers) actually painted: idle windows surface as
// `rendered: false` rather than a fake 120fps.

import type { Map as MLMap } from 'maplibre-gl';

export type RenderFpsSample = {
	/** Painted frames per second over the last window; 0 when idle. */
	fps: number;
	/** False when no frame painted in the window (map idle). */
	rendered: boolean;
};

export type RenderFpsMonitor = {
	/** Starts the sampling windows; returns a stop function. */
	start(): () => void;
	/** Records one painted frame. */
	bump(): void;
	/** Latest window's sample (initially idle). */
	sample(): RenderFpsSample;
	/** Invoked once per sampling window with the fresh sample. */
	onSample(cb: (sample: RenderFpsSample) => void): () => void;
};

export const FPS_SAMPLE_INTERVAL_MS = 500;

export function createRenderFpsMonitor(intervalMs = FPS_SAMPLE_INTERVAL_MS): RenderFpsMonitor {
	let frames = 0;
	let lastWindowStart = Date.now();
	let last: RenderFpsSample = { fps: 0, rendered: false };
	const listeners = new Set<(sample: RenderFpsSample) => void>();
	let timer: ReturnType<typeof setInterval> | null = null;

	function tick() {
		const now = Date.now();
		const elapsed = now - lastWindowStart;
		const fps = elapsed > 0 ? (frames * 1000) / elapsed : 0;
		last = { fps, rendered: frames > 0 };
		frames = 0;
		lastWindowStart = now;
		for (const cb of listeners) cb(last);
	}

	return {
		start() {
			if (timer !== null) return () => undefined;
			frames = 0;
			lastWindowStart = Date.now();
			timer = setInterval(tick, intervalMs);
			return () => {
				if (timer === null) return;
				clearInterval(timer);
				timer = null;
			};
		},
		bump() {
			frames += 1;
		},
		sample() {
			return last;
		},
		onSample(cb) {
			listeners.add(cb);
			return () => listeners.delete(cb);
		}
	};
}

/** Wires a monitor to a live map's paint events. Returns a detach function. */
export function attachRenderFpsMonitor(monitor: RenderFpsMonitor, map: MLMap): () => void {
	const handler = () => monitor.bump();
	map.on('render', handler);
	return () => {
		map.off('render', handler);
	};
}
