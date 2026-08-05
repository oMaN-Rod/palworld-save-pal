import type { SymbolLayerSpecification } from '@maplibre/maplibre-gl-style-spec';
import type { RawLayerProps } from '../types.js';

export interface SymbolLayerProps extends RawLayerProps {
	paint?: SymbolLayerSpecification['paint'];
	layout?: SymbolLayerSpecification['layout'];
}
