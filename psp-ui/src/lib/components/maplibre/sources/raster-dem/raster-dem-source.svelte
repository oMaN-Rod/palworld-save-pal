<script lang="ts">
	import type { Source as MaplibreSource } from 'maplibre-gl';
	import RawSource from '../raw/raw-source.svelte';
	import type { RasterDEMSourceProps } from './types.js';

	let {
		source = $bindable<MaplibreSource | undefined>(undefined),
		children,
		...rest
	}: RasterDEMSourceProps = $props();
</script>

<RawSource type="raster-dem" bind:source {...rest}>
	{#if children}
		{@render children()}
	{/if}
</RawSource>
