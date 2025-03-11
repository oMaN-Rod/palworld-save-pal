<script lang="ts">
	import { Map } from '$components';
	import { getAppState } from '$states';
	import { worldToLeaflet, worldToMap } from '$components/map/utils';
	import { Accordion } from '@skeletonlabs/skeleton-svelte';
	import { mapImg } from '$components/map/mapImages';
	import { Target } from 'lucide-svelte';
	import { mapObjects } from '$lib/data';
	import L from 'leaflet';
	import type { Base, Player } from '$types';

	const appState = getAppState();

	let showOrigin = $state(false);
	let showPlayers = $state(true);
	let showBases = $state(true);
	let showFastTravel = $state(true);
	let showDungeons = $state(true);
	let section = $state(['players']);
	let map: L.Map | undefined = $state();

	const players = $derived(Object.values(appState.players || {}));
	const playerCount = $derived(players.length);
	const guilds = $derived(Object.values(appState.guilds || {}));
	const bases = $derived.by(() => {
		return Object.values(guilds).reduce(
			(acc, guild) => {
				if (guild.bases) {
					Object.values(guild.bases).forEach((base) => {
						acc[base.id] = base;
					});
				}
				return acc;
			},
			{} as Record<string, any>
		);
	});
	const fastTravelCount = $derived.by(() => {
		return (
			Object.values(mapObjects.points).filter((point) => point.type === 'fast_travel').length || 0
		);
	});
	const dungeonCount = $derived.by(() => {
		return Object.values(mapObjects.points).filter((point) => point.type === 'dungeon').length || 0;
	});

	function handlePlayerFocus(player: Player) {
		const coords = worldToLeaflet(player.location.x, player.location.y);
		map?.flyTo(coords, 3);
	}

	function handleBaseFocus(base: Base) {
		const coords = worldToLeaflet(base.location.x, base.location.y);
		map?.flyTo(coords, 3);
	}
</script>

<div class="grid h-full grid-cols-[25%_1fr] gap-2">
	<div class="flex flex-col gap-4 p-4">
		<div class="flex flex-col gap-2">
			<h1 class="text-2xl font-bold">World Map</h1>
			<p class="text-sm text-gray-500">Explore the world of Palworld.</p>
		</div>

		<div class="flex flex-col gap-4">
			{#if appState.saveFile}
				<div class="flex flex-col gap-2">
					<h2 class="text-lg font-bold">Map Controls</h2>
					<div class="grid grid-cols-2 gap-2">
						<button
							class="flex items-center space-x-2 {showOrigin ? '' : 'opacity-25'}"
							onclick={() => (showOrigin = !showOrigin)}
						>
							<Target class="mr-2 h-6 w-6" />
							<span>Origin</span>
						</button>
						<button
							class="flex items-center space-x-2 {showFastTravel ? '' : 'opacity-25'} "
							onclick={() => (showFastTravel = !showFastTravel)}
						>
							<img src={mapImg.fastTravel} alt="Fast Travel" class="mr-2 h-6 w-6" />
							<span>Fast Travel</span>
							<span class="text-surface-500 text-xs">{fastTravelCount}</span>
						</button>
						<button
							class="flex items-center space-x-2 {showPlayers ? '' : 'opacity-25'}"
							onclick={() => (showPlayers = !showPlayers)}
						>
							<img src={mapImg.player} alt="Players" class="mr-2 h-6 w-6" />
							<span>Players</span>
							<span class="text-surface-500 text-xs">{playerCount}</span>
						</button>
						<button
							class="flex items-center space-x-2 {showBases ? '' : 'opacity-25'}"
							onclick={() => (showBases = !showBases)}
						>
							<img src={mapImg.baseCamp} alt="Bases" class="mr-2 h-6 w-6" />
							<span>Bases</span>
							<span class="text-surface-500 text-xs">{Object.keys(bases).length}</span>
						</button>
						<button
							class="flex items-center space-x-2 {showDungeons ? '' : 'opacity-25'}"
							onclick={() => (showDungeons = !showDungeons)}
						>
							<img src={mapImg.dungeon} alt="Dungeons" class="mr-2 h-6 w-6" />
							<span>Dungeons</span>
							<span class="text-surface-500 text-xs">{dungeonCount}</span>
						</button>
					</div>
				</div>
				<Accordion value={section} onValueChange={(e) => (section = e.value)} collapsible>
					<Accordion.Item value="players" controlHover="hover:bg-secondary-500/25">
						{#snippet control()}
							<h2 class="text-lg font-bold">Players</h2>
						{/snippet}
						{#snippet panel()}
							{#if showPlayers && playerCount > 0}
								<div class="flex flex-col gap-2">
									<div class="max-h-64 overflow-y-auto">
										{#each players as player}
											{#if player.location}
												{@const mapCoords = worldToMap(player.location.x, player.location.y)}
												<button
													class="bg-surface-800 hover:bg-secondary-500/25 w-full rounded-sm p-2 text-start"
													onclick={() => handlePlayerFocus(player)}
												>
													<div class="font-bold">{player.nickname}</div>
													<div class="text-xs">Level: {player.level} | HP: {player.hp}</div>
													<div class="text-xs text-gray-400">
														Location: {Math.round(mapCoords.x)}, {Math.round(mapCoords.y)}
													</div>
												</button>
											{/if}
										{/each}
									</div>
								</div>
							{:else}
								<p class="text-sm text-gray-500">No players found.</p>
							{/if}
						{/snippet}
					</Accordion.Item>
					<Accordion.Item value="bases" controlHover="hover:bg-secondary-500/25">
						{#snippet control()}
							<h2 class="text-lg font-bold">Bases</h2>
						{/snippet}
						{#snippet panel()}
							{#if showBases}
								<div class="flex flex-col gap-2">
									<div class="max-h-64 overflow-y-auto">
										{#each Object.values(bases) as base}
											<button
												class="bg-surface-800 hover:bg-secondary-500/25 mb-2 w-full rounded-sm p-2 text-start"
												onclick={() => handleBaseFocus(base)}
											>
												<div class="font-bold">{base.id}</div>
												<div class="text-xs text-gray-400">
													Location: {worldToMap(base.location.x, base.location.y).x}, {worldToMap(
														base.location.x,
														base.location.y
													).y}
												</div>
											</button>
										{/each}
									</div>
								</div>
							{:else}
								<p class="text-sm text-gray-500">No bases found.</p>
							{/if}
						{/snippet}
					</Accordion.Item>
				</Accordion>
			{/if}

			<div class="mt-auto flex flex-col gap-2">
				<p class="text-sm text-gray-500">Click on the map to see detailed coordinates.</p>
			</div>
		</div>
	</div>
	<Map bind:map {showOrigin} {showPlayers} {showBases} {showFastTravel} {showDungeons} />
</div>
