<script lang="ts">
	import Icon from '$lib/components/ui/icons/Icon.svelte';
	import { Button, SectionHeader } from '$components/ui';
	import MapLayerPanel from './MapLayerPanel.svelte';
	import MapHints from './MapHints.svelte';
	import type { MapLayerVisibility, PanelOptionId } from './layerPanelModel';
	import type { Snippet } from 'svelte';
	import * as m from '$i18n/messages';

	let {
		saveLoaded = false,
		touch = false,
		showHeader = true,
		layers,
		onVisibilityChange,
		onShowAll,
		count,
		available,
		onUnlockMap,
		savePanel
	}: {
		saveLoaded?: boolean;
		touch?: boolean;
		showHeader?: boolean;
		layers: MapLayerVisibility;
		onVisibilityChange: (patch: MapLayerVisibility) => void;
		onShowAll?: (visible: boolean) => void;
		count?: (id: PanelOptionId) => string | undefined;
		available?: (id: PanelOptionId) => boolean;
		onUnlockMap?: () => void;
		savePanel?: Snippet;
	} = $props();
</script>

<div class="flex min-h-0 flex-1 flex-col gap-4 overflow-y-auto p-4">
	<div class="flex flex-col gap-2">
		{#if showHeader}
			<div class="flex items-center">
				<SectionHeader text={m.map_options()}>
					{#snippet action()}
						{#if saveLoaded && onUnlockMap}
							<Button
								variant="ghost"
								size="sm"
								class="flex items-center gap-2"
								onclick={onUnlockMap}
							>
								<Icon icon="tabler:lock-open" class="h-4 w-4" />
								<span>{m.unlock_map()}</span>
							</Button>
						{/if}
					{/snippet}
				</SectionHeader>
			</div>
		{:else if saveLoaded && onUnlockMap}
			<Button variant="ghost" size="sm" class="flex items-center gap-2" onclick={onUnlockMap}>
				<Icon icon="tabler:lock-open" class="h-4 w-4" />
				<span>{m.unlock_map()}</span>
			</Button>
		{/if}

		<MapLayerPanel
			{layers}
			{onVisibilityChange}
			{onShowAll}
			{count}
			{available}
			{touch}
		/>
	</div>

	{@render savePanel?.()}

	<MapHints saveHints={saveLoaded} {touch} />
</div>
