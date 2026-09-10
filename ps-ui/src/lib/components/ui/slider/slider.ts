export type SliderSize = 'xs' | 'sm' | 'md';

/** Decimals carried by `step`, so a snapped value comes back as 0.3 rather
 *  than 0.30000000000000004. */
function precisionOf(step: number): number {
	const text = String(step);
	const dot = text.indexOf('.');
	return dot < 0 ? 0 : text.length - dot - 1;
}

function clamp(value: number, min: number, max: number): number {
	return Math.max(min, Math.min(max, value));
}

/** Snap `value` to the `step` grid anchored at `min`, clamped to the track.
 *  `max` stays reachable even when it sits off the grid -- a half-step short
 *  of the end snaps to the end, so dragging fully right always lands on max.
 *  A non-positive or non-finite `step` disables snapping. */
export function quantize(value: number, min: number, max: number, step: number): number {
	if (!Number.isFinite(value)) return min;
	const clamped = clamp(value, min, max);
	if (!Number.isFinite(step) || step <= 0) return clamped;
	if (clamped >= max - step / 2) return max;
	const snapped = min + Math.round((clamped - min) / step) * step;
	return Number(clamp(snapped, min, max).toFixed(precisionOf(step)));
}

/** Where `value` sits on the track, as 0..1. An empty track reads as empty. */
export function fractionOf(value: number, min: number, max: number): number {
	if (max <= min) return 0;
	return clamp((value - min) / (max - min), 0, 1);
}

/** The stepped value a pointer at `fraction` across the track selects. */
export function valueFromFraction(
	fraction: number,
	min: number,
	max: number,
	step: number
): number {
	return quantize(min + clamp(fraction, 0, 1) * (max - min), min, max, step);
}

/** The Shift-arrow step: a twentieth of the track, rounded onto the step grid
 *  and never smaller than one step. */
export function coarseStep(min: number, max: number, step: number): number {
	if (!Number.isFinite(step) || step <= 0) return 1;
	const coarse = Math.round((max - min) / 20 / step) * step;
	return Number(Math.max(step, coarse).toFixed(precisionOf(step)));
}
