import type { Source as MaplibreSource, RasterDEMSourceSpecification } from 'maplibre-gl';
import type { Snippet } from 'svelte';

export interface RasterDEMSourceProps extends Omit<RasterDEMSourceSpecification, 'type'> {
	id?: string;
	source?: MaplibreSource | undefined;
	children?: Snippet;
}
