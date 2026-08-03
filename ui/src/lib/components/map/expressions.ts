import type { ExpressionSpecification } from 'maplibre-gl';

export const ICON_ZOOM_MIN = 2;
export const ICON_ZOOM_MAX = 7;

type StopValue = number | ExpressionSpecification;

/** Every marker renders this much larger than the size its call site declares. */
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
