import type { FillLayerSpecification } from '@maplibre/maplibre-gl-style-spec';
import type { RawLayerProps } from '../types.js';

export interface FillLayerProps extends RawLayerProps {
	paint?: FillLayerSpecification['paint'];
	layout?: FillLayerSpecification['layout'];
}
