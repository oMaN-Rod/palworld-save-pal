// Render-quality tiers for the 3D live map. Each level maps to concrete
// renderer parameters; 'auto' lets a controller step between levels from
// measured render FPS. The parameters are consumed by Map.svelte and the
// layers it wires up -- this module is pure so the tier math and the auto
// controller's hysteresis stay unit-testable.

export type MapQualityLevel = 'very-low' | 'low' | 'medium' | 'high' | 'very-high';
export type MapQualitySetting = MapQualityLevel | 'auto';

export const MAP_QUALITY_LEVELS: MapQualityLevel[] = [
	'very-low',
	'low',
	'medium',
	'high',
	'very-high'
];

// The default tier reproduces the pre-quality-settings rendering exactly
// (device pixel ratio, the scenery 10px cull, user-chosen structure mode), so
// returning users see no change until they pick something else.
export const MAP_QUALITY_DEFAULT: MapQualitySetting = 'high';

export type QualityParams = {
	/** Canvas backing-store ratio; `null` leaves MapLibre's device default. */
	pixelRatio: number | null;
	/** Scenery instances below this on-screen diameter (CSS px) are culled. */
	sceneryMinPixels: number;
	/** Detailed structures draw as proxy boxes instead of loading their glbs. */
	forceStructuresProxy: boolean;
	/** Mesh-cache entries unused for this long (and not actively drawn) are
	 * disposed -- the dynamic offload of out-of-sight assets. */
	meshSweepAgeMs: number;
};

export function qualityParams(level: MapQualityLevel, devicePixelRatio: number): QualityParams {
	const dpr = Number.isFinite(devicePixelRatio) && devicePixelRatio > 0 ? devicePixelRatio : 1;
	switch (level) {
		case 'very-low':
			// Visibly sparse: sub-native backing store + aggressive scenery cull + proxy-only structures.
			// At dpr=1 this drops canvas pixels to 0.75x, ~44% fewer fragments.
			return {
				pixelRatio: Math.min(0.75, dpr),
				sceneryMinPixels: 36,
				forceStructuresProxy: true,
				meshSweepAgeMs: 15_000
			};
		case 'low':
			return {
				pixelRatio: 1,
				sceneryMinPixels: 22,
				forceStructuresProxy: true,
				meshSweepAgeMs: 25_000
			};
		case 'medium':
			return {
				pixelRatio: Math.min(dpr, 1.5),
				sceneryMinPixels: 12,
				forceStructuresProxy: false,
				meshSweepAgeMs: 45_000
			};
		case 'high':
			return {
				pixelRatio: null,
				sceneryMinPixels: 10,
				forceStructuresProxy: false,
				meshSweepAgeMs: 60_000
			};
		case 'very-high':
			// Supersampled above the device ratio where the cap allows; the max()
			// keeps a high-DPI display from being undersampled by the cap.
			return {
				pixelRatio: Math.max(dpr, Math.min(dpr * 1.5, 3)),
				sceneryMinPixels: 5,
				forceStructuresProxy: false,
				meshSweepAgeMs: 90_000
			};
	}
}

// --- Auto controller ------------------------------------------------------

// Below FPS_DOWN the renderer is struggling; above FPS_UP (or while idle --
// no frames painted means no load) it has headroom. Both thresholds sit wide
// of each other so a steady 45-55fps session never oscillates.
export const FPS_DOWN = 45;
export const FPS_UP = 55;
const PRESSURE_BUCKETS = 3; // ~1.5s of sustained low fps at the 500ms cadence
const RELAX_BUCKETS = 6; // ~3s of sustained headroom
const COOLDOWN_MS = 10_000;

// Auto never climbs into 'very-high' on its own: supersampling is a deliberate
// choice, not something to switch on under a user's feet.
const AUTO_MAX_LEVEL: MapQualityLevel = 'high';

export type AutoQualityState = {
	level: MapQualityLevel;
	pressure: number;
	relax: number;
	cooldownUntil: number;
};

export function createAutoQualityState(start: MapQualityLevel = 'high'): AutoQualityState {
	return { level: start, pressure: 0, relax: 0, cooldownUntil: 0 };
}

export type AutoStepResult = { state: AutoQualityState; level: MapQualityLevel; changed: boolean };

/** Feeds one sampled render-fps window (`null` when nothing painted) into the
 * controller and returns the possibly-stepped level. Pure: the input state is
 * never mutated. */
export function autoQualityStep(
	state: AutoQualityState,
	fps: number | null,
	nowMs: number
): AutoStepResult {
	const next: AutoQualityState = { ...state };
	if (nowMs < next.cooldownUntil) return { state: next, level: next.level, changed: false };

	if (fps !== null && fps < FPS_DOWN) {
		next.pressure += 1;
		next.relax = 0;
	} else if (fps === null || fps > FPS_UP) {
		next.relax += 1;
		next.pressure = 0;
	} else {
		next.pressure = 0;
		next.relax = 0;
	}

	if (next.pressure >= PRESSURE_BUCKETS) {
		const index = MAP_QUALITY_LEVELS.indexOf(next.level);
		const stepped = Math.max(0, index - 1);
		if (stepped !== index) {
			next.level = MAP_QUALITY_LEVELS[stepped];
			next.pressure = 0;
			next.relax = 0;
			next.cooldownUntil = nowMs + COOLDOWN_MS;
			return { state: next, level: next.level, changed: true };
		}
		next.pressure = 0;
	}

	if (next.relax >= RELAX_BUCKETS) {
		const index = MAP_QUALITY_LEVELS.indexOf(next.level);
		const ceiling = MAP_QUALITY_LEVELS.indexOf(AUTO_MAX_LEVEL);
		const stepped = Math.min(ceiling, index + 1);
		if (stepped !== index) {
			next.level = MAP_QUALITY_LEVELS[stepped];
			next.pressure = 0;
			next.relax = 0;
			next.cooldownUntil = nowMs + COOLDOWN_MS;
			return { state: next, level: next.level, changed: true };
		}
		next.relax = 0;
	}

	return { state: next, level: next.level, changed: false };
}

export function isMapQualitySetting(value: unknown): value is MapQualitySetting {
	return value === 'auto' || MAP_QUALITY_LEVELS.includes(value as MapQualityLevel);
}
