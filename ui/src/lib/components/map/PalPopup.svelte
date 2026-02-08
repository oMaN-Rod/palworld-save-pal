<script lang="ts">
	import { Card } from '$components/ui';
	import type { MapObject, Pal } from '$types';
	import { worldToMap } from './utils';
	import { palsData } from '$lib/data';
	import { assetLoader } from '$utils';
	import { PalBadge } from '$components';
	import { Globe } from '@lucide/svelte';
	import { Map } from '@lucide/svelte';

	let {
		point,
		isPredator = false
	}: {
		point: MapObject;
		isPredator?: boolean;
	} = $props();

	const mapCoords = $derived(worldToMap(point.x, point.y));
	const palData = $derived(palsData.getByKey(point.pal));

	const pal = $derived({
		instance_id: '',
		character_key: point.pal,
		character_id: point.pal,
		is_sick: false,
		is_predator: isPredator,
		is_boss: !isPredator
	} as Pal);
</script>

<Card class="min-w-70 shadow-lg">
	<div class="pointer-events-auto space-y-3">
		<div class="border-b-surface-700 border-b pb-3">
			<div class="flex items-start gap-2">
				<PalBadge
					{pal}
					onMove={() => {}}
					onAdd={() => {}}
					onClone={() => {}}
					onDelete={() => {}}
					disabled
				/>
				<div class="min-w-0 flex-1">
					<h3 class="text-foreground truncate text-lg font-bold">
						{palData ? palData.localized_name : point.pal}
					</h3>
					<p class="text-muted-foreground text-sm">
						{isPredator ? 'Predator Pal' : 'Alpha Pal'}
					</p>
				</div>
			</div>
		</div>
		<div class="border-b-surface-700 border-b pb-3">
			<div class="flex items-start gap-2">
				<p class="text-muted-foreground text-sm">
					{palData ? palData.description : ''}
				</p>
			</div>
		</div>

		<div class="space-y-2">
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
