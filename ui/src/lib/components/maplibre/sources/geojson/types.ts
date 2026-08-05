import type { GeoJSONSource as MaplibreGeoJSONSource, GeoJSONSourceSpecification } from 'maplibre-gl';
import type { Snippet } from 'svelte';

export interface GeoJSONSourceProps extends Omit<GeoJSONSourceSpecification, 'type'> {
	id?: string;
	source?: MaplibreGeoJSONSource | undefined;
	children?: Snippet;
}
