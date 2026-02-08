<script lang="ts">
	import { Card } from '$components/ui';
	import type { MapObject } from '$types';
	import { worldToMap } from './utils';
	import { Castle, Globe, Map } from 'lucide-svelte';

	let {
		point
	}: {
		point: MapObject;
	} = $props();

	const mapCoords = $derived(worldToMap(point.x, point.y));
</script>

<Card class="min-w-70 shadow-lg">
	<div class="pointer-events-auto space-y-3">
		<div class="border-b pb-3">
			<div class="flex items-start gap-2">
				<Castle class="text-primary mt-1 h-5 w-5 shrink-0" />
				<div class="min-w-0 flex-1">
					<h3 class="text-foreground truncate text-lg font-bold">Dungeon</h3>
					<p class="text-muted-foreground text-sm">Dungeon Location</p>
				</div>
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
