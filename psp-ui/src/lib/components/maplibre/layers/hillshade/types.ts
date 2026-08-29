import type { HillshadeLayerSpecification } from '@maplibre/maplibre-gl-style-spec';
import type { RawLayerProps } from '../types.js';

export interface HillshadeLayerProps extends RawLayerProps {
	paint?: HillshadeLayerSpecification['paint'];
	layout?: HillshadeLayerSpecification['layout'];
}
