import type { Source as MaplibreSource, RasterSourceSpecification } from 'maplibre-gl';
import type { Snippet } from 'svelte';

export interface RasterSourceProps extends Omit<RasterSourceSpecification, 'type'> {
	id?: string;
	source?: MaplibreSource | undefined;
	children?: Snippet;
}
