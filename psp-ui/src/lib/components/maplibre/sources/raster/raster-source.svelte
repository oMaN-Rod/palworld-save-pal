<script lang="ts">
	import type { Source as MaplibreSource } from 'maplibre-gl';
	import RawSource from '../raw/raw-source.svelte';
	import type { RasterSourceProps } from './types.js';

	let {
		source = $bindable<MaplibreSource | undefined>(undefined),
		children,
		...rest
	}: RasterSourceProps = $props();
</script>

<RawSource type="raster" bind:source {...rest}>
	{#if children}
		{@render children()}
	{/if}
</RawSource>
