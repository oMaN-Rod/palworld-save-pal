export { default as Map } from './map/Map.svelte';
export { default as FeatureState } from './utilities/feature-state/feature-state.svelte';
export { default as ImageLoader } from './utilities/image-loader/image-loader.svelte';
export { default as Terrain } from './utilities/terrain/terrain.svelte';

import GeoJSONSource from './sources/geojson/geojson-source.svelte';
import RasterDEMSource from './sources/raster-dem/raster-dem-source.svelte';
import RasterSource from './sources/raster/raster-source.svelte';
import RawSource from './sources/raw/raw-source.svelte';

export const Source = {
	Raw: RawSource,
	GeoJSON: GeoJSONSource,
	Raster: RasterSource,
	RasterDEM: RasterDEMSource
};

import CircleLayer from './layers/circle/circle-layer.svelte';
import FillExtrusionLayer from './layers/fill-extrusion/fill-extrusion-layer.svelte';
import FillLayer from './layers/fill/fill-layer.svelte';
import HillshadeLayer from './layers/hillshade/hillshade-layer.svelte';
import LineLayer from './layers/line/line-layer.svelte';
import RasterLayer from './layers/raster/raster-layer.svelte';
import RawLayer from './layers/raw/raw-layer.svelte';
import SymbolLayer from './layers/symbol/symbol-layer.svelte';

export const Layer = {
	Raw: RawLayer,
	Symbol: SymbolLayer,
	Line: LineLayer,
	Fill: FillLayer,
	FillExtrusion: FillExtrusionLayer,
	Raster: RasterLayer,
	Circle: CircleLayer,
	Hillshade: HillshadeLayer
};

import FullscreenControl from './controls/fullscreen/fullscreen-control.svelte';
import NavigationControl from './controls/navigation/navigation-control.svelte';

export const Control = { Navigation: NavigationControl, Fullscreen: FullscreenControl };

export {
	LayerContext,
	MapContext,
	SourceContext,
	getLayerContext,
	getMapContext,
	getSourceContext,
	setLayerContext,
	setMapContext,
	setSourceContext,
	tryGetSourceContext
} from './contexts.svelte.js';

export type { ControlPosition, Theme, TooltipFeature } from './types.js';
export { generateId } from './utils.js';
