<script lang="ts">
	import Icon from '$lib/components/ui/icons/Icon.svelte';
	import type { Player } from '$types';
	import { worldToMap } from './utils';

	let {
		player
	}: {
		player: Player;
	} = $props();

	const mapCoords = $derived(worldToMap(player.location.x, player.location.y));
</script>

<div class="popup-content">
	<div class="border-b-surface-700 mb-2 flex gap-4 border-b pb-2">
		<div class="bg-muted/50 flex items-center gap-2 rounded-md">
			<Icon icon="tabler:trophy" class="text-primary h-4 w-4 shrink-0" />
			<div class="min-w-0">
				<div class="text-muted-foreground text-xs">Level</div>
				<div class="text-sm font-semibold">{player.level}</div>
			</div>
		</div>
		<h3 class="text-lg font-bold">{player.nickname}</h3>
	</div>
	<div>
		<div class="flex gap-4">
			<div class="bg-muted/50 flex items-center gap-2 rounded-md">
				<Icon icon="tabler:heart" class="h-4 w-4 shrink-0 text-red-500" />
				<div class="min-w-0">
					<div class="text-muted-foreground text-xs">HP</div>
					<div class="text-sm font-semibold">{player.hp}</div>
				</div>
			</div>
		</div>
		<div class="bg-muted/50 flex items-center gap-2 rounded-md">
			<Icon icon="tabler:clock" class="h-4 w-4 shrink-0" />
			<div class="min-w-0">
				<div class="text-muted-foreground text-xs">Last Online</div>
				<span class="truncate">{new Date(player.last_online_time).toLocaleString()}</span>
			</div>
		</div>
	</div>
	<div class="mt-2 space-y-1">
		<div class="flex items-start gap-2">
			<Icon icon="tabler:world" class="text-primary mt-0.5 h-3.5 w-3.5 shrink-0" />
			<div class="min-w-0 flex-1">
				<div class="text-muted-foreground text-xs font-medium">World Coords</div>
				<div class="font-mono text-xs">
					{player.location.x.toFixed(2)}, {player.location.y.toFixed(2)}, {player.location.z.toFixed(
						2
					)}
				</div>
			</div>
		</div>
		<div class="flex items-start gap-2">
			<Icon icon="tabler:map" class="text-primary mt-0.5 h-3.5 w-3.5 shrink-0" />
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
