import type { LineLayerSpecification } from '@maplibre/maplibre-gl-style-spec';
import type { RawLayerProps } from '../types.js';

export interface LineLayerProps extends RawLayerProps {
	paint?: LineLayerSpecification['paint'];
	layout?: LineLayerSpecification['layout'];
}
