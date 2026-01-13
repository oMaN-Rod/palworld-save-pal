<script lang="ts">
	import type { MapObject } from '$types';
	import { worldToMap } from './utils';
	import { palsData } from '$lib/data';
	import { Globe, Map } from 'lucide-svelte';

	let {
		point,
		isPredator = false
	}: {
		point: MapObject;
		isPredator?: boolean;
	} = $props();

	const mapCoords = $derived(worldToMap(point.x, point.y));
	const palData = $derived(palsData.getByKey(point.pal));
</script>

<div class="popup-content">
	<h3 class="text-lg font-bold">
		{palData ? palData.localized_name : point.pal}
	</h3>
	<div class="mt-2 space-y-1">
		<div class="flex items-start gap-2">
			<Globe class="text-primary mt-0.5 h-3.5 w-3.5 shrink-0" />
			<div class="min-w-0 flex-1">
				<div class="text-muted-foreground text-xs font-medium">World Coords</div>
				<div class="font-mono text-xs">
					{point.x.toFixed(2)}, {point.y.toFixed(2)}
				</div>
			</div>
		</div>
		<div class="flex items-start gap-2">
			<Map class="text-primary mt-0.5 h-3.5 w-3.5 shrink-0" />
			<div class="min-w-0 flex-1">
				<div class="text-muted-foreground text-xs font-medium">Map Coords</div>
				<div class="font-mono text-xs">
					{mapCoords.x}, {mapCoords.y * -1}
				</div>
			</div>
		</div>
	</div>
</div>

<style>
	.popup-content {
		background-color: var(--color-surface-900);
		color: white;
		padding: 8px;
		border-radius: 4px;
		min-width: 150px;
	}

	.popup-content h3 {
		margin-top: 0;
		margin-bottom: 8px;
	}
</style>
