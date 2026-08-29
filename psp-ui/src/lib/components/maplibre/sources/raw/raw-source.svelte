<script lang="ts">
	import { onDestroy, untrack } from 'svelte';
	import type { SourceSpecification, Source as MaplibreSource } from 'maplibre-gl';
	import { getMapContext, SourceContext, setSourceContext } from '../../contexts.svelte.js';
	import { generateId } from '../../utils.js';
	import type { RawSourceProps } from './types.js';

	let {
		id = generateId('source'),
		type,
		source = $bindable<MaplibreSource | undefined>(undefined),
		children,
		...spec
	}: RawSourceProps = $props();

	const ctx = getMapContext();

	const sourceId = untrack(() => id);
	setSourceContext(new SourceContext(sourceId));

	ctx.whenLoaded(() => {
		ctx.addSource(sourceId, untrack(() => ({ type, ...spec })) as SourceSpecification);
		source = ctx.map!.getSource(sourceId);
	});

	onDestroy(() => {
		source = undefined;
		ctx.removeSource(sourceId);
	});
</script>

{#if ctx.loaded}
	{@render children?.()}
{/if}
