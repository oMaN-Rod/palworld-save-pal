<script lang="ts">
	import { Card } from '$components/ui';
	import { goto } from '$app/navigation';
	import { getAppState } from '$states';
	import type { Player } from '$types';
	import { worldToMap } from './utils';
	import { Pencil, Heart, Trophy, Clock, Users, Gamepad2, Swords, Globe, Map } from 'lucide-svelte';

	let {
		player
	}: {
		player: Player;
	} = $props();

	const appState = getAppState();

	function handleEdit(event: MouseEvent) {
		console.log('Edit button clicked for player:', player);
		event.stopPropagation();
		event.preventDefault();
		if (appState.selectedPlayerUid !== player.uid) {
			appState.selectedPlayer = player;
			appState.selectedPlayerUid = player.uid;
		}
		goto('/edit');
	}

	const mapCoords = $derived(worldToMap(player.location.x, player.location.y));

	const guildName = $derived.by(() => {
		if (!player.guild_id) return null;
		const guild = appState.guilds?.[player.guild_id];
		if (guild) return guild.name;
		const guildSummary = appState.guildSummaries?.[player.guild_id];
		return guildSummary?.name || null;
	});

	const palCount = $derived(player.pals ? Object.keys(player.pals).length : 0);
	const dpsCount = $derived(player.dps ? Object.keys(player.dps).length : 0);
</script>

<Card class="min-w-70 shadow-lg">
	<div class="pointer-events-auto space-y-3">
		<!-- Header -->
		<div class="flex items-start justify-between gap-3 border-b pb-3">
			<div class="min-w-0 flex-1">
				<h3 class="text-foreground truncate text-lg font-bold">{player.nickname}</h3>
				<div class="text-muted-foreground mt-1 flex items-center gap-1.5 text-xs">
					<Clock class="h-3 w-3 shrink-0" />
					<span class="truncate">{new Date(player.last_online_time).toLocaleString()}</span>
				</div>
			</div>
			<button
				onclick={handleEdit}
				type="button"
				class="chip preset-outlined-primary-500 hover:preset-outlined-secondary-500 inline-flex shrink-0 items-center gap-1.5 rounded-md px-3 py-1.5 text-xs font-medium transition-colors"
			>
				<Pencil class="h-3 w-3" />
				Edit
			</button>
		</div>

		<!-- Guild Info -->
		{#if guildName}
			<div class="border-b pb-3">
				<div class="bg-muted/50 flex items-center gap-2 rounded-md p-2">
					<Users class="text-primary h-4 w-4 shrink-0" />
					<div class="min-w-0 flex-1">
						<div class="text-muted-foreground text-xs">Guild</div>
						<div class="truncate text-sm font-semibold">{guildName}</div>
					</div>
				</div>
			</div>
		{/if}

		<!-- Stats Grid -->
		<div class="grid grid-cols-2 gap-3">
			<div class="bg-muted/50 flex items-center gap-2 rounded-md p-2">
				<Trophy class="text-primary h-4 w-4 shrink-0" />
				<div class="min-w-0">
					<div class="text-muted-foreground text-xs">Level</div>
					<div class="text-sm font-semibold">{player.level}</div>
				</div>
			</div>
			<div class="bg-muted/50 flex items-center gap-2 rounded-md p-2">
				<Heart class="h-4 w-4 shrink-0 text-red-500" />
				<div class="min-w-0">
					<div class="text-muted-foreground text-xs">HP</div>
					<div class="text-sm font-semibold">{player.hp}</div>
				</div>
			</div>
			<div class="bg-muted/50 flex items-center gap-2 rounded-md p-2">
				<Gamepad2 class="text-primary h-4 w-4 shrink-0" />
				<div class="min-w-0">
					<div class="text-muted-foreground text-xs">Pals</div>
					<div class="text-sm font-semibold">{palCount}</div>
				</div>
			</div>
			{#if dpsCount > 0}
				<div class="bg-muted/50 flex items-center gap-2 rounded-md p-2">
					<Swords class="h-4 w-4 shrink-0 text-orange-500" />
					<div class="min-w-0">
						<div class="text-muted-foreground text-xs">DPS</div>
						<div class="text-sm font-semibold">{dpsCount}</div>
					</div>
				</div>
			{/if}
		</div>

		<!-- Coordinates -->
		<div class="space-y-2 border-t pt-2">
			<div class="flex items-start gap-2">
				<Globe class="text-primary mt-0.5 h-4 w-4 shrink-0" />
				<div class="min-w-0 flex-1">
					<div class="text-muted-foreground mb-1 text-xs font-medium">World Coordinates</div>
					<div class="text-foreground font-mono text-xs">
						{player.location.x.toFixed(2)}, {player.location.y.toFixed(2)}, {player.location.z.toFixed(
							2
						)}
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
