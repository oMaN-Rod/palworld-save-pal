<script lang="ts">
	import type { Boss } from '$types';
	import { worldToMap } from './utils';
	import { Globe, Map } from 'lucide-svelte';
	import * as m from '$i18n/messages';

	let {
		point
	}: {
		point: Boss & { defeated?: boolean };
	} = $props();

	const mapCoords = $derived(worldToMap(point.x, point.y));
</script>

<h3 class="mt-0 mb-2 text-lg font-bold">{point.character_id}</h3>
<p class="text-muted-foreground mb-2 text-xs">{m.level()}: {point.level}</p>
<div class="space-y-1">
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
