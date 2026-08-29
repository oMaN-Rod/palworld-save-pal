import type { FillExtrusionLayerSpecification } from '@maplibre/maplibre-gl-style-spec';
import type { RawLayerProps } from '../types.js';

export interface FillExtrusionLayerProps extends RawLayerProps {
	paint?: FillExtrusionLayerSpecification['paint'];
	layout?: FillExtrusionLayerSpecification['layout'];
}
