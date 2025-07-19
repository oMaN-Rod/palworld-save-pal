<script lang="ts">
	import { Map } from '$components';
	import { getAppState, getModalState } from '$states';
	import { worldToLeaflet, worldToMap } from '$components/map/utils';
	import { Accordion } from '@skeletonlabs/skeleton-svelte';
	import { mapImg } from '$components/map/mapImages';
	import { Target } from 'lucide-svelte';
	import { mapObjects } from '$lib/data';
	import L from 'leaflet';
	import type { Base, Player } from '$types';
	import { assetLoader } from '$utils';
	import { TextInputModal } from '$components/modals';
	import { EntryState } from '$types';
	import { staticIcons } from '$types/icons';
	import { persistedState } from 'svelte-persisted-state';

	const appState = getAppState();
	const modal = getModalState();

	type MapOptions = {
		showOrigin: boolean;
		showPlayers: boolean;
		showBases: boolean;
		showFastTravel: boolean;
		showDungeons: boolean;
		showAlphaPals: boolean;
		showPredatorPals: boolean;
	};

	const mapOptionsState = persistedState<MapOptions>('mapOptions', {
		showOrigin: false,
		showPlayers: true,
		showBases: true,
		showFastTravel: true,
		showDungeons: true,
		showAlphaPals: true,
		showPredatorPals: true
	});
	const mapOptions = $derived(mapOptionsState.current);
	// let showOrigin = persistedState('showOrigin', false);
	// let showPlayers = persistedState('showPlayers', true);
	// let showBases = persistedState('showBases', true);
	// let showFastTravel = persistedState('showFastTravel', true);
	// let showDungeons = persistedState('showDungeons', true);
	// let showAlphaPals = persistedState('showAlphaPals', true);
	// let showPredatorPals = persistedState('showPredatorPals', true);
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
	const alphaPalCount = $derived.by(() => {
		return (
			Object.values(mapObjects.points).filter((point) => point.type === 'alpha_pal').length || 0
		);
	});
	const predatorPalCount = $derived.by(() => {
		return (
			Object.values(mapObjects.points).filter((point) => point.type === 'predator_pal').length || 0
		);
	});
	const anubisImg = $derived(assetLoader.loadMenuImage('anubis'));
	const starryonImg = $derived(assetLoader.loadMenuImage('nightbluehorse'));

	function handlePlayerFocus(player: Player) {
		const coords = worldToLeaflet(player.location.x, player.location.y);
		map?.flyTo(coords, 3);
	}

	function handleBaseFocus(base: Base) {
		const coords = worldToLeaflet(base.location.x, base.location.y);
		map?.flyTo(coords, 3);
	}

	async function handleEditBaseName(base: Base) {
		// @ts-ignore
		const result = await modal.showModal<string>(TextInputModal, {
			title: 'Edit Base Name',
			value: base.name || ''
		});
		if (!result) return;

		// Find the guild that contains this base
		const guild = Object.values(appState.guilds || {}).find(
			(g) => g.bases && Object.values(g.bases).some((b) => b.id === base.id)
		);

		if (guild && guild.bases) {
			const baseInGuild = Object.values(guild.bases).find((b) => b.id === base.id);
			if (baseInGuild) {
				baseInGuild.name = result;
				guild.state = EntryState.MODIFIED;
			}
		}
	}
</script>

<div class="grid h-full grid-cols-[25%_1fr] gap-2">
	<div class="flex flex-col gap-4 p-4">
		<div class="flex flex-col gap-4">
			<div class="flex flex-col gap-2">
				<h2 class="text-lg font-bold">Map Controls</h2>
				<div class="grid grid-cols-2 gap-2">
					<button
						class="flex items-center space-x-2 {mapOptions.showOrigin ? '' : 'opacity-25'}"
						onclick={() => (mapOptions.showOrigin = !mapOptions.showOrigin)}
					>
						<Target class="mr-2 h-6 w-6" />
						<span>Origin</span>
					</button>
					<button
						class="flex items-center space-x-2 {mapOptions.showFastTravel ? '' : 'opacity-25'} "
						onclick={() => (mapOptions.showFastTravel = !mapOptions.showFastTravel)}
					>
						<img src={mapImg.fastTravel} alt="Fast Travel" class="mr-2 h-6 w-6" />
						<span>Fast Travel</span>
						<span class="text-surface-500 text-xs">{fastTravelCount}</span>
					</button>
					{#if appState.saveFile}
						<button
							class="flex items-center space-x-2 {mapOptions.showPlayers ? '' : 'opacity-25'}"
							onclick={() => (mapOptions.showPlayers = !mapOptions.showPlayers)}
						>
							<img src={mapImg.player} alt="Players" class="mr-2 h-6 w-6" />
							<span>Players</span>
							<span class="text-surface-500 text-xs">{playerCount}</span>
						</button>
						<button
							class="flex items-center space-x-2 {mapOptions.showBases ? '' : 'opacity-25'}"
							onclick={() => (mapOptions.showBases = !mapOptions.showBases)}
						>
							<img src={mapImg.baseCamp} alt="Bases" class="mr-2 h-6 w-6" />
							<span>Bases</span>
							<span class="text-surface-500 text-xs">{Object.keys(bases).length}</span>
						</button>
					{/if}

					<button
						class="flex items-center space-x-2 {mapOptions.showDungeons ? '' : 'opacity-25'}"
						onclick={() => (mapOptions.showDungeons = !mapOptions.showDungeons)}
					>
						<img src={mapImg.dungeon} alt="Dungeons" class="mr-2 h-6 w-6" />
						<span>Dungeons</span>
						<span class="text-surface-500 text-xs">{dungeonCount}</span>
					</button>
					<button
						class="flex items-center space-x-2 {mapOptions.showAlphaPals ? '' : 'opacity-25'}"
						onclick={() => (mapOptions.showAlphaPals = !mapOptions.showAlphaPals)}
					>
						<img src={anubisImg} alt="Alpha Pals" class="mr-2 h-6 w-6" />
						<span>Alpha Pals</span>
						<span class="text-surface-500 text-xs">{alphaPalCount}</span>
					</button>
					<button
						class="flex items-center space-x-2 {mapOptions.showPredatorPals ? '' : 'opacity-25'}"
						onclick={() => (mapOptions.showPredatorPals = !mapOptions.showPredatorPals)}
					>
						<img src={starryonImg} alt="Predator Pals" class="mr-2 h-6 w-6" />
						<span>Predator Pals</span>
						<span class="text-surface-500 text-xs">{predatorPalCount}</span>
					</button>
				</div>
			</div>
			{#if appState.saveFile}
				<Accordion value={section} onValueChange={(e) => (section = e.value)} collapsible>
					<Accordion.Item value="players" controlHover="hover:bg-secondary-500/25">
						{#snippet control()}
							<h2 class="text-lg font-bold">Players</h2>
						{/snippet}
						{#snippet panel()}
							{#if mapOptions.showPlayers && playerCount > 0}
								<div class="max-h-64 space-y-2 overflow-y-auto">
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
												<div class="text-xs text-gray-400">
													Last Online: {new Date(player.last_online_time).toLocaleString()}
												</div>
											</button>
										{/if}
									{/each}
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
							{#if mapOptions.showBases}
								<div class="max-h-64 space-y-2 overflow-y-auto">
									{#each Object.values(bases) as base}
										<button
											class="bg-surface-800 hover:bg-secondary-500/25 mb-2 w-full rounded-sm p-2 text-start"
											onclick={() => handleBaseFocus(base)}
											oncontextmenu={(e) => {
												e.preventDefault();
												handleEditBaseName(base);
											}}
										>
											<div class="font-bold">{base.name}</div>
											<div class="text-xs text-gray-400">
												ID: {base.id}
											</div>
											<div class="text-xs text-gray-400">
												Location: {worldToMap(base.location.x, base.location.y).x}, {worldToMap(
													base.location.x,
													base.location.y
												).y}
											</div>
										</button>
									{/each}
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
				<div class="flex flex-col">
					<div class="flex items-center gap-2">
						<img src={staticIcons.leftClickIcon} alt="Right Click" class=" h-6 w-6" />
						<span class="text-xs text-gray-500">Left-click on a player or base to focus.</span>
					</div>
					<div class="flex items-center gap-2">
						<img src={staticIcons.rightClickIcon} alt="Right Click" class=" h-6 w-6" />
						<span class="text-xs text-gray-500">Right-click on a base to edit its name.</span>
					</div>
				</div>
			</div>
		</div>
	</div>
	<Map
		bind:map
		showOrigin={mapOptions.showOrigin}
		showPlayers={mapOptions.showPlayers}
		showBases={mapOptions.showBases}
		showFastTravel={mapOptions.showFastTravel}
		showDungeons={mapOptions.showDungeons}
		showAlphaPals={mapOptions.showAlphaPals}
		showPredatorPals={mapOptions.showPredatorPals}
		onEditBaseName={handleEditBaseName}
	/>
</div>
