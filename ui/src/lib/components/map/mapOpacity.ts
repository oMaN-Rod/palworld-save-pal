// The raster never fades out completely: the hillshade beneath it carries no
// colour of its own, so at zero the map reads as an empty void.
export const MAP_OPACITY_MIN = 0.1;

// Clamped on read, not just at the slider: a value persisted before the floor
// existed can still be below it, and 0 slips past `?? 1`.
export function clampMapOpacity(value: number | undefined): number {
	return Math.max(MAP_OPACITY_MIN, value ?? 1);
}
