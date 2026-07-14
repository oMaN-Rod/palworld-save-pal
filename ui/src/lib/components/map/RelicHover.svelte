<script lang="ts">
	import type { RelicPoint } from '$types';
	import { worldToMap } from './utils';
	import { relicTypeIcon } from './styles';
	import { Globe, Map, Check, X } from 'lucide-svelte';
	import * as m from '$i18n/messages';

	let {
		point
	}: {
		point: RelicPoint;
	} = $props();

	const mapCoords = $derived(worldToMap(point.x, point.y));
</script>

<div class="popup-content">
	<div class="flex items-center gap-2">
		<img
			src={relicTypeIcon(point.relic_type)}
			alt={point.localized_name}
			class="h-5 w-5 shrink-0"
		/>
		<h3 class="text-lg font-bold">{point.localized_name}</h3>
	</div>
	<div class="mt-2 space-y-1">
		{#if point.unlocked !== undefined}
			<div class="flex items-center gap-2">
				{#if point.unlocked}
					<Check class="mt-0.5 h-3.5 w-3.5 shrink-0 text-green-400" />
					<span class="text-xs text-green-400">{m.collected()}</span>
				{:else}
					<X class="mt-0.5 h-3.5 w-3.5 shrink-0 text-red-400" />
					<span class="text-xs text-red-400">{m.not_collected()}</span>
				{/if}
			</div>
		{/if}
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
	}
</style>
