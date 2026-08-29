import type { Source as MaplibreSource, SourceSpecification } from 'maplibre-gl';
import type { Snippet } from 'svelte';

export interface RawSourceProps {
	id?: string;
	type: SourceSpecification['type'];
	source?: MaplibreSource | undefined;
	children?: Snippet;
	[key: string]: unknown;
}
