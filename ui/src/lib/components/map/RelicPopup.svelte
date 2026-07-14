<script lang="ts">
	import { Card } from '$components/ui';
	import type { RelicPoint } from '$types';
	import { worldToMap } from './utils';
	import { Globe, Map, Check, X } from 'lucide-svelte';
	import { relicTypeIcon } from './styles';
	import * as m from '$i18n/messages';

	let {
		point
	}: {
		point: RelicPoint;
	} = $props();

	const mapCoords = $derived(worldToMap(point.x, point.y));
</script>

<Card class="min-w-70 shadow-lg">
	<div class="pointer-events-auto space-y-3">
		<div class="border-b pb-3">
			<div class="flex items-start gap-2">
				<img
					src={relicTypeIcon(point.relic_type)}
					alt={point.localized_name}
					class="mt-1 h-5 w-5 shrink-0"
				/>
				<div class="min-w-0 flex-1">
					<h3 class="text-foreground truncate text-lg font-bold">{point.localized_name}</h3>
				</div>
			</div>
		</div>

		<div class="space-y-2">
			{#if point.unlocked !== undefined}
				<div class="flex items-center gap-2">
					{#if point.unlocked}
						<Check class="h-4 w-4 shrink-0 text-green-400" />
						<span class="text-sm text-green-400">{m.collected()}</span>
					{:else}
						<X class="h-4 w-4 shrink-0 text-red-400" />
						<span class="text-sm text-red-400">{m.not_collected()}</span>
					{/if}
				</div>
			{/if}
			<div class="flex items-start gap-2">
				<Globe class="text-primary mt-0.5 h-4 w-4 shrink-0" />
				<div class="min-w-0 flex-1">
					<div class="text-muted-foreground mb-1 text-xs font-medium">World Coordinates</div>
					<div class="text-foreground font-mono text-xs">
						{point.x.toFixed(2)}, {point.y.toFixed(2)}
					</div>
				</div>
			</div>
			<div class="flex items-start gap-2">
				<Map class="text-primary mt-0.5 h-4 w-4 shrink-0" />
				<div class="min-w-0 flex-1">
					<div class="text-muted-foreground mb-1 text-xs font-medium">Map Coordinates</div>
					<div class="text-foreground font-mono text-xs">
						{mapCoords.x}, {mapCoords.y * -1}
					</div>
				</div>
			</div>
		</div>
	</div>
</Card>
