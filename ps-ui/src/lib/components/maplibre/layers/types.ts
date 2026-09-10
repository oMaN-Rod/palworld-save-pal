import maplibregl from 'maplibre-gl';
import type {
	FilterSpecification
} from 'maplibre-gl';
import type { Snippet } from 'svelte';

export type LayerEventType = maplibregl.MapLayerEventType;

export type LayerEventProps = {
	[K in keyof maplibregl.MapLayerEventType as `on${K}`]?: (ev: maplibregl.MapLayerEventType[K]) => void;
};

export interface RawLayerProps extends Omit<maplibregl.LayerSpecification, 'id' | 'source' | 'source-layer' | 'type'>, LayerEventProps {
	id?: string;
	type?: string;
	source?: string;
	sourceLayer?: string;
	filter?: FilterSpecification;
	beforeId?: string;
	visible?: boolean;
	children?: Snippet;
}
