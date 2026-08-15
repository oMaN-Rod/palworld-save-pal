<script lang="ts">
	import type { BaseStructure } from '$types';
	import { Box, Heart } from 'lucide-svelte';
	import { structureInfo } from './structureInfo';
	import { baseStructuresData, buildingsData } from '$lib/data';
	import { getAppState } from '$states';

	let { structure }: { structure: BaseStructure } = $props();

	const appState = getAppState();
	const info = $derived(
		structureInfo(
			structure,
			baseStructuresData.footprints,
			buildingsData.buildings,
			appState.playerSummaries
		)
	);
</script>

<div class="popup-content">
	<h3 class="text-lg font-bold">{info.name}</h3>
	<span class="truncate text-xs font-light">{structure.map_object_id}</span>
	<div class="mt-2 space-y-1">
		<div class="flex items-start gap-2">
			<Box class="text-primary mt-0.5 h-3.5 w-3.5 shrink-0" />
			<div class="min-w-0 flex-1">
				<div class="text-muted-foreground text-xs font-medium">Type</div>
				<div class="font-mono text-xs">{info.typeA}</div>
			</div>
		</div>
		<div class="flex items-start gap-2">
			<Heart class="mt-0.5 h-3.5 w-3.5 shrink-0 text-red-500" />
			<div class="min-w-0 flex-1">
				<div class="text-muted-foreground text-xs font-medium">HP</div>
				<div class="font-mono text-xs">{info.hp} / {info.hpMax}</div>
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
