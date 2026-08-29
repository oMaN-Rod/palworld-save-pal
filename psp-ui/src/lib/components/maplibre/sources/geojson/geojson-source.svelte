<script lang="ts">
	import { onDestroy, untrack } from 'svelte';
	import type {
		GeoJSONSource as MaplibreGeoJSONSource,
		GeoJSONSourceSpecification
	} from 'maplibre-gl';
	import { getMapContext, SourceContext, setSourceContext } from '../../contexts.svelte.js';
	import { generateId } from '../../utils.js';
	import type { GeoJSONSourceProps } from './types.js';

	let {
		id = generateId('source'),
		data,
		cluster,
		clusterRadius,
		clusterMaxZoom,
		clusterMinPoints,
		clusterProperties,
		maxzoom,
		attribution,
		buffer,
		tolerance,
		lineMetrics,
		generateId: genId,
		promoteId,
		source = $bindable<MaplibreGeoJSONSource | undefined>(undefined),
		children
	}: GeoJSONSourceProps = $props();

	const ctx = getMapContext();

	const sourceId = untrack(() => id);
	setSourceContext(new SourceContext(sourceId));

	ctx.whenLoaded(() => {
		const spec: Record<string, unknown> = { type: 'geojson' };
		const init = untrack(() => ({
			data,
			cluster,
			clusterRadius,
			clusterMaxZoom,
			clusterMinPoints,
			clusterProperties,
			maxzoom,
			attribution,
			buffer,
			tolerance,
			lineMetrics,
			generateId: genId,
			promoteId
		}));

		if (init.data !== undefined) spec.data = init.data;
		if (init.cluster !== undefined) spec.cluster = init.cluster;
		if (init.clusterRadius !== undefined) spec.clusterRadius = init.clusterRadius;
		if (init.clusterMaxZoom !== undefined) spec.clusterMaxZoom = init.clusterMaxZoom;
		if (init.clusterMinPoints !== undefined) spec.clusterMinPoints = init.clusterMinPoints;
		if (init.clusterProperties !== undefined) spec.clusterProperties = init.clusterProperties;
		if (init.maxzoom !== undefined) spec.maxzoom = init.maxzoom;
		if (init.attribution !== undefined) spec.attribution = init.attribution;
		if (init.buffer !== undefined) spec.buffer = init.buffer;
		if (init.tolerance !== undefined) spec.tolerance = init.tolerance;
		if (init.lineMetrics !== undefined) spec.lineMetrics = init.lineMetrics;
		if (init.generateId !== undefined) spec.generateId = init.generateId;
		if (init.promoteId !== undefined) spec.promoteId = init.promoteId;

		ctx.addSource(sourceId, spec as GeoJSONSourceSpecification);
		source = ctx.map!.getSource(sourceId) as MaplibreGeoJSONSource;
	});

	$effect(() => {
		if (source && data !== undefined) {
			source.setData(data);
		}
	});

	onDestroy(() => {
		source = undefined;
		ctx.removeSource(sourceId);
	});
</script>

{#if ctx.loaded}
	{@render children?.()}
{/if}
