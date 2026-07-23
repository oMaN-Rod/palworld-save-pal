import type { ExpressionSpecification } from 'maplibre-gl';

export const ICON_ZOOM_MIN = 2;
export const ICON_ZOOM_MAX = 7;

type StopValue = number | ExpressionSpecification;

// icon-size is a layout property, so it must stay state-constant: MapLibre rejects the
// whole layer at addLayer if a feature-state reference appears anywhere inside it, and
// the zoom input has to be fed to a top-level interpolate rather than nested in another
// operator. Hover emphasis therefore lives on icon-opacity, which is paint.
export function zoomScaledIconSize(small: StopValue, large: StopValue): ExpressionSpecification {
	return ['interpolate', ['linear'], ['zoom'], ICON_ZOOM_MIN, small, ICON_ZOOM_MAX, large];
}
