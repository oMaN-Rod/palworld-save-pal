import type { RasterLayerSpecification } from '@maplibre/maplibre-gl-style-spec';
import type { RawLayerProps } from '../types.js';

export interface RasterLayerProps extends RawLayerProps {
	paint?: RasterLayerSpecification['paint'];
	layout?: RasterLayerSpecification['layout'];
}
