export { default as Map } from './map/Map.svelte';
export { default as ImageLoader } from './utilities/image-loader/image-loader.svelte';
export { default as FeatureState } from './utilities/feature-state/feature-state.svelte';
export { default as Terrain } from './utilities/terrain/terrain.svelte';

import RawSource from './sources/raw/raw-source.svelte';
import GeoJSONSource from './sources/geojson/geojson-source.svelte';
import RasterSource from './sources/raster/raster-source.svelte';
import RasterDEMSource from './sources/raster-dem/raster-dem-source.svelte';

export const Source = { Raw: RawSource, GeoJSON: GeoJSONSource, Raster: RasterSource, RasterDEM: RasterDEMSource };

import RawLayer from './layers/raw/raw-layer.svelte';
import SymbolLayer from './layers/symbol/symbol-layer.svelte';
import LineLayer from './layers/line/line-layer.svelte';
import FillLayer from './layers/fill/fill-layer.svelte';
import FillExtrusionLayer from './layers/fill-extrusion/fill-extrusion-layer.svelte';
import RasterLayer from './layers/raster/raster-layer.svelte';
import CircleLayer from './layers/circle/circle-layer.svelte';

export const Layer = {
	Raw: RawLayer,
	Symbol: SymbolLayer,
	Line: LineLayer,
	Fill: FillLayer,
	FillExtrusion: FillExtrusionLayer,
	Raster: RasterLayer,
	Circle: CircleLayer
};

import NavigationControl from './controls/navigation/navigation-control.svelte';
import FullscreenControl from './controls/fullscreen/fullscreen-control.svelte';

export const Control = { Navigation: NavigationControl, Fullscreen: FullscreenControl };

export {
	MapContext,
	SourceContext,
	LayerContext,
	setMapContext,
	getMapContext,
	setSourceContext,
	getSourceContext,
	tryGetSourceContext,
	setLayerContext,
	getLayerContext
} from './contexts.svelte.js';

export { generateId } from './utils.js';
export type { ControlPosition, Theme, TooltipFeature } from './types.js';
