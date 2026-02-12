<script lang="ts">
	import type { Base } from '$types';
	import { LandPlot } from '@lucide/svelte';
	import { worldToMap } from './utils';
	import { Globe, Map } from 'lucide-svelte';

	let {
		base,
		guildName
	}: {
		base: Base;
		guildName?: string;
	} = $props();

	const mapCoords = $derived(worldToMap(base.location.x, base.location.y));
</script>

<div class="popup-content">
	<h3 class="text-lg font-bold">{base.name}</h3>
	<h4 class="text-xs font-bold">Guild: {guildName}</h4>
	<h4 class="text-xs font-light">ID: {base.id}</h4>
	<div class="mt-2 space-y-1">
		<div class="flex items-start gap-2">
			<LandPlot class="text-primary mt-0.5 h-3.5 w-3.5 shrink-0" />
			<div class="min-w-0 flex-1">
				<div class="text-muted-foreground text-xs font-medium">Area</div>
				<div class="font-mono text-xs">
					{base.area_range}
				</div>
			</div>
		</div>
		<div class="flex items-start gap-2">
			<Globe class="text-primary mt-0.5 h-3.5 w-3.5 shrink-0" />
			<div class="min-w-0 flex-1">
				<div class="text-muted-foreground text-xs font-medium">World Coords</div>
				<div class="font-mono text-xs">
					{base.location.x.toFixed(2)}, {base.location.y.toFixed(2)}
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
