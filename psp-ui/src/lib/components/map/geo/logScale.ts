// Logarithmic because the useful resolution sits near the top of these ranges:
// a linear slider spends most of its travel in sizes too small to see.
export function sliderToScale(position: number, min: number, max: number): number {
	const t = Math.min(1, Math.max(0, position));
	return min * (max / min) ** t;
}

export function scaleToSlider(scale: number, min: number, max: number): number {
	const clamped = Math.min(max, Math.max(min, scale));
	return Math.log(clamped / min) / Math.log(max / min);
}
