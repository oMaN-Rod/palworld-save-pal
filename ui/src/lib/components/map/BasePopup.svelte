<script lang="ts">
	import { Card } from '$components/ui';
	import type { Base } from '$types';
	import { worldToMap } from './utils';
	import { Home, Globe, Map } from 'lucide-svelte';
	import { itemsData } from '$lib/data';
	import { assetLoader } from '$utils';
	import { ASSET_DATA_PATH } from '$types/icons';

	let {
		base,
		guildName
	}: {
		base: Base;
		guildName?: string;
	} = $props();

	const mapCoords = $derived(worldToMap(base.location.x, base.location.y));
	const palCount = $derived(base.pals ? Object.keys(base.pals).length : 0);
	const containerCount = $derived(Object.keys(base.storage_containers || {}).length);
	const baseValue = $derived.by(() => {
		if (!base.storage_containers) return '0';
		const containers = Object.values(base.storage_containers);
		const slots = containers.flatMap((container) => container.slots);
		const total = slots.reduce((sum, slot) => {
			const itemData = itemsData.getByKey(slot.static_id);
			return sum + (itemData ? itemData.details.price * slot.count : 0);
		}, 0);
		return total.toLocaleString();
	});

	const goldCoinIcon = $derived.by(() => {
		const goldCoinData = itemsData.getByKey('money');
		return assetLoader.loadImage(`${ASSET_DATA_PATH}/img/${goldCoinData?.details.icon}.webp`);
	});
</script>

<Card class="min-w-70 shadow-lg">
	<div class="pointer-events-auto space-y-3">
		<div class="border-b pb-3">
			<div class="flex items-start gap-2">
				<Home class="text-primary mt-1 h-5 w-5 shrink-0" />
				<div class="min-w-0 flex-1">
					<h3 class="text-foreground truncate text-lg font-bold">{base.name}</h3>
					<span class="text-muted-foreground truncate text-xs font-light">Guild: {guildName}</span>
					<span class="text-muted-foreground truncate text-xs font-light">ID: {base.id}</span>
				</div>
			</div>
		</div>

		<div class="grid grid-cols-2">
			<div class="bg-muted/50 rounded-md p-2">
				<div class="text-muted-foreground text-xs">Pals at Base</div>
				<div class="text-sm font-semibold">{palCount}</div>
			</div>

			<div class="bg-muted/50 rounded-md p-2">
				<div class="text-muted-foreground text-xs">Storage Containers</div>
				<div class="text-sm font-semibold">{containerCount}</div>
				<div class="flex items-center gap-0.5">
					<img src={goldCoinIcon} alt="Gold" class="h-5 w-5" />
					<div>{baseValue}</div>
				</div>
			</div>
		</div>

		<div class="space-y-2 border-t pt-2">
			<div class="flex items-start gap-2">
				<Globe class="text-primary mt-0.5 h-4 w-4 shrink-0" />
				<div class="min-w-0 flex-1">
					<div class="text-muted-foreground mb-1 text-xs font-medium">World Coordinates</div>
					<div class="text-foreground font-mono text-xs">
						{base.location.x.toFixed(2)}, {base.location.y.toFixed(2)}
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
