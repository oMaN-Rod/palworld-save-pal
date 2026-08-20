import type { ExpressionSpecification } from 'maplibre-gl';

export const ICON_ZOOM_MIN = 2;
export const ICON_ZOOM_MAX = 7;

export type StopValue = number | ExpressionSpecification;

export const ICON_SCALE = 1.15;

function scaleStop(value: StopValue): StopValue {
	return typeof value === 'number' ? value * ICON_SCALE : ['*', value, ICON_SCALE];
}

// icon-size is a layout property, so it must stay state-constant: MapLibre rejects the
// whole layer at addLayer if a feature-state reference appears anywhere inside it, and
// the zoom input has to be fed to a top-level interpolate rather than nested in another
// operator. Hover emphasis therefore lives on icon-opacity, which is paint.
export function zoomScaledIconSize(small: StopValue, large: StopValue): ExpressionSpecification {
	return [
		'interpolate',
		['linear'],
		['zoom'],
		ICON_ZOOM_MIN,
		scaleStop(small),
		ICON_ZOOM_MAX,
		scaleStop(large)
	];
}

export const HALO_PAD = 3;

// Callers pass the artwork's measured visible extent, not the source canvas -- every
// icon carries transparent padding, so the canvas overstates how much space it occupies.
export function haloRadiusPx(artPx: number, iconSize: number): number {
	return (artPx * iconSize * ICON_SCALE) / 2 + HALO_PAD;
}

// Unlike zoomScaledIconSize this does not apply ICON_SCALE -- haloRadiusPx already has.
export function zoomScaledRadius(small: StopValue, large: StopValue): ExpressionSpecification {
	return ['interpolate', ['linear'], ['zoom'], ICON_ZOOM_MIN, small, ICON_ZOOM_MAX, large];
}
