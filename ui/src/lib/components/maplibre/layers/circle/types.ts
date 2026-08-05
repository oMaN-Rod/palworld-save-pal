import type { CircleLayerSpecification } from '@maplibre/maplibre-gl-style-spec';
import type { RawLayerProps } from '../types.js';

export interface CircleLayerProps extends RawLayerProps {
	paint?: CircleLayerSpecification['paint'];
	layout?: CircleLayerSpecification['layout'];
}
