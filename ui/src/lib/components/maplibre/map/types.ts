import type {
	MapOptions,
	StyleSpecification,
	MapMouseEvent,
	MapTouchEvent,
	MapLibreEvent,
	MapLibreZoomEvent,
	MapWheelEvent,
	MapTerrainEvent,
	MapDataEvent,
	MapSourceDataEvent,
	MapStyleDataEvent,
	MapContextEvent,
	MapStyleImageMissingEvent
} from 'maplibre-gl';
import type maplibregl from 'maplibre-gl';
import type { Snippet } from 'svelte';
import type { Theme } from '../types.js';

// --- Map event types ---
export type MapEventType = maplibregl.MapEventType;

export type MapEventProps = {
	[K in keyof maplibregl.MapEventType as `on${K}`]?: (ev: maplibregl.MapEventType[K]) => void;
};

// --- Map props ---

export interface MapProps extends Omit<MapOptions, 'container' | 'style'>, MapEventProps {
	style: string | StyleSpecification;
	transformStyle?: (style: StyleSpecification) => StyleSpecification;
	projection?: 'mercator' | 'globe';
	theme?: Theme;
	map?: maplibregl.Map;

	// Bindable camera state
	center?: [number, number];
	bearing?: number;
	pitch?: number;
	zoom?: number;

	// Accessors from https://maplibre.org/maplibre-gl-js/docs/API/classes/Map/#accessors
	repaint?: boolean;
	showCollisionBoxes?: boolean;
	showOverdrawInspector?: boolean;
	showPadding?: boolean;
	showTileBoundaries?: boolean;

	children?: Snippet;
	class?: string;
}
