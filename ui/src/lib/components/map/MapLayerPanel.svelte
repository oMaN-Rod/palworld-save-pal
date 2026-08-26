<script lang="ts">
	import { mapLayers } from '$lib/data/mapLayerStore.svelte';
	import { Eye, EyeOff, LoaderCircle } from '@lucide/svelte';
	import {
		buildPanelGroups,
		hideAllLabel,
		loadingLabel,
		showAllLabel,
		type MapLayerVisibility,
		type PanelOptionId
	} from './layerPanelModel';
	import { isMapLayerId } from './layerRegistry';
	import { Accordion } from '@skeletonlabs/skeleton-svelte';

	let {
		layers,
		onVisibilityChange,
		onShowAll,
		count = markerCount,
		loading = layerLoading,
		available
	}: {
		layers: MapLayerVisibility;
		onVisibilityChange: (patch: MapLayerVisibility) => void;
		onShowAll?: (visible: boolean) => void;
		count?: (id: PanelOptionId) => string | undefined;
		loading?: (id: PanelOptionId) => boolean;
		available?: (id: PanelOptionId) => boolean;
	} = $props();

	let value = $state(['general']);

	// peek, never getLayer: the panel reports what is already cached and must not
	// pull an artifact over the wire just by being rendered.
	function markerCount(id: PanelOptionId): string | undefined {
		if (!isMapLayerId(id)) return undefined;
		const points = mapLayers.peek(id)?.points.length;
		return points === undefined ? undefined : String(points);
	}

	function layerLoading(id: PanelOptionId): boolean {
		return isMapLayerId(id) && mapLayers.isLoading(id);
	}

	const groups = $derived(buildPanelGroups(layers, { count, loading, available }));
</script>

{#if onShowAll}
	<div class="border-b-surface-800 grid grid-cols-2 border-b-2 pb-2">
		<button type="button" class="flex items-center space-x-2" onclick={() => onShowAll(true)}>
			<Eye class="mr-2 h-4 w-4" />
			<span class="text-sm">{showAllLabel()}</span>
		</button>
		<button type="button" class="flex items-center space-x-2" onclick={() => onShowAll(false)}>
			<EyeOff class="mr-2 h-4 w-4" />
			<span class="text-sm">{hideAllLabel()}</span>
		</button>
	</div>
{/if}

<Accordion {value} onValueChange={(e: { value: string[] }) => (value = e.value)} multiple>
	{#each groups as group (group.group)}
		<Accordion.Item
			value={group.group}
			controlHover="hover:bg-secondary-500/25"
			classes="border-b-surface-800 border-b"
		>
			{#snippet control()}{group.label}{/snippet}
			{#snippet panel()}
				<div class="grid grid-cols-2 gap-2">
					{#each group.rows as row (row.id)}
						<button
							type="button"
							data-option={row.id}
							class="flex items-center space-x-2 {row.visible ? '' : 'opacity-25'}"
							onclick={() => onVisibilityChange({ [row.id]: !row.visible })}
						>
							<img src={row.icon} alt={row.label} class="mr-2 h-6 w-6" />
							<span>{row.label}</span>
							{#if row.loading}
								<LoaderCircle
									data-loading={row.id}
									class="text-surface-500 h-3 w-3 animate-spin"
									aria-label={loadingLabel()}
								/>
							{:else if row.count !== undefined}
								<span class="text-surface-500 text-xs">{row.count}</span>
							{/if}
						</button>
					{/each}
				</div>
			{/snippet}
		</Accordion.Item>
	{/each}
</Accordion>
