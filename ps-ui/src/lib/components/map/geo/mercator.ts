import { MAP_SIZE } from './utils';

export const MERCATOR_LAT_LIMIT = 85.0511287798;

// Inset from the antimeridian by the same epsilon MapLibre uses internally. Its
// defaultConstrain passes each maxBounds longitude edge through wrap(x, 0, worldSize),
// which returns worldSize for both 0 and worldSize -- so edges on exactly +/-180 collapse
// onto each other, the span becomes zero, and zoom is driven to Infinity.
const ALMOST_180 = 180 - 1e-10;

export const MAP_MAX_BOUNDS: [[number, number], [number, number]] = [
	[-ALMOST_180, -MERCATOR_LAT_LIMIT],
	[ALMOST_180, MERCATOR_LAT_LIMIT]
];

export function pixelToLngLat(px: number, py: number): [number, number] {
	const lng = (px / MAP_SIZE) * 360 - 180;
	const n = Math.PI * ((2 * py) / MAP_SIZE - 1);
	const lat = (Math.atan(Math.sinh(n)) * 180) / Math.PI;
	return [lng, lat];
}

export function lngLatToPixel(lng: number, lat: number): [number, number] {
	const px = ((lng + 180) / 360) * MAP_SIZE;
	const rad = (lat * Math.PI) / 180;
	const n = Math.log(Math.tan(rad) + 1 / Math.cos(rad));
	const py = ((n / Math.PI + 1) / 2) * MAP_SIZE;
	return [px, py];
}

export function pixelCirclePolygon(
	cx: number,
	cy: number,
	radiusPx: number,
	segments = 64
): [number, number][] {
	const ring: [number, number][] = [];
	for (let i = 0; i <= segments; i++) {
		const theta = (i / segments) * Math.PI * 2;
		ring.push(pixelToLngLat(cx + radiusPx * Math.cos(theta), cy + radiusPx * Math.sin(theta)));
	}
	return ring;
}

export const EARTH_CIRCUMFERENCE_M = 40075016.686;

export function verticalScaleFactor(centerLat: number, cmPerPx: number): number {
	const worldSpanCm = cmPerPx * MAP_SIZE;
	return (Math.cos((centerLat * Math.PI) / 180) * EARTH_CIRCUMFERENCE_M) / worldSpanCm;
}
