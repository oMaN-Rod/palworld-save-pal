<script lang="ts">
	import { Map, PlayerList } from '$components';
	import { Combobox } from '$components/ui';
	import { getAppState, getModalState, getToastState } from '$states';
	import { worldToPixel, worldToMap } from '$components/map/utils';
	import { Accordion } from '@skeletonlabs/skeleton-svelte';
	import { mapImg } from '$components/map/styles';
	import { Target, Unlock, Users, Building } from 'lucide-svelte';
	import { mapObjects } from '$lib/data';
	import type { Map as OLMap } from 'ol';
	import type { Base, GuildSummary, Player } from '$types';
	import { assetLoader } from '$utils';
	import { TextInputModal } from '$components/modals';
	import { EntryState, MessageType } from '$types';
	import { staticIcons } from '$types/icons';
	import { persistedState } from 'svelte-persisted-state';
	import type { ValueChangeDetails } from '@zag-js/accordion';
	import { sendAndWait } from '$utils/websocketUtils';
	import { SectionHeader } from '$components/ui';

	const appState = getAppState();
	const modal = getModalState();
	let selectedPlayerUid = $state('');
	let selectedGuildId = $state('');

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
	const toast = getToastState();
	let section = $state(['players']);
	let map: OLMap | null = $state(null);

	const players = $derived(Object.values(appState.players || {}));
	const loadedPlayerCount = $derived(players.length);
	const totalPlayerCount = $derived(Object.keys(appState.playerSummaries || {}).length);
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
	const loadedBaseCount = $derived(Object.keys(bases).length);
	const totalBaseCount = $derived(
		Object.values(appState.guildSummaries || {}).reduce(
			(acc, summary) => acc + (summary as GuildSummary).base_count,
			0
		)
	);

	const guildSelectOptions = $derived.by(() => {
		return Object.entries(appState.guildSummaries as Record<string, GuildSummary>).map(
			([id, summary]) => ({
				value: id,
				label: summary.loaded
					? `🟦 ${summary.name} (${summary.base_count} bases)`
					: `🟪 ${summary.name} (${summary.base_count} bases)`
			})
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

	function panTo(x: number, y: number) {
		const coords = worldToPixel(x, y);
		map?.getView().animate({ center: coords, zoom: 5, duration: 500 });
	}
	function handlePlayerFocus(player: Player) {
		if (!player.location) return;
		panTo(player.location.x, player.location.y);
	}

	function handlePlayerLoaded(player: Player) {
		selectedPlayerUid = player.uid;
		if (player.location) {
			handlePlayerFocus(player);
		}
	}

	function handleBaseFocus(base: Base) {
		if (!base.location) return;
		panTo(base.location.x, base.location.y);
	}

	function handleGuildSelect(guildId: string) {
		selectedGuildId = guildId;
		if (appState.guilds?.[guildId]) {
			// Guild already loaded, focus on first base if available
			const guild = appState.guilds[guildId];
			const firstBase = guild.bases ? Object.values(guild.bases)[0] : null;
			if (firstBase?.location) {
				handleBaseFocus(firstBase);
			}
		} else {
			// Load the guild
			appState.loadGuildLazy(guildId);
		}
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

	async function handleUnlockMap() {
		// @ts-ignore
		const confirmed = await modal.showConfirmModal({
			title: 'Unlock Full Map',
			message:
				'This will unlock the entire map by modifying your LocalData.sav file. You will need to select your LocalData.sav file. Continue?',
			confirmText: 'Select File',
			cancelText: 'Cancel'
		});

		if (confirmed) {
			const response: { success: boolean; message: string } = await sendAndWait(
				MessageType.UNLOCK_MAP,
				{}
			);
			const { success, message } = response;
			if (success) {
				toast.add(message, 'Success!', 'success');
			}
		}
	}

	$effect(() => {
		if (appState.selectedPlayer) {
			handlePlayerLoaded(appState.selectedPlayer);
		}
	});
</script>

<div class="grid h-full grid-cols-[20%_1fr] gap-2">
	<div class="flex flex-col gap-4 p-4">
		<div class="flex flex-col gap-4">
			<div class="flex flex-col gap-2">
				<div class="flex items-center">
					<SectionHeader text="Map Options">
						{#snippet action()}
							<button class="btn btn-sm flex items-center gap-2" onclick={handleUnlockMap}>
								<Unlock class="h-4 w-4" />
								<span>Unlock Map</span>
							</button>
						{/snippet}
					</SectionHeader>
				</div>
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
							<span class="text-surface-500 text-xs">{loadedPlayerCount}/{totalPlayerCount}</span>
						</button>
						<button
							class="flex items-center space-x-2 {mapOptions.showBases ? '' : 'opacity-25'}"
							onclick={() => (mapOptions.showBases = !mapOptions.showBases)}
						>
							<img src={mapImg.baseCamp} alt="Bases" class="mr-2 h-6 w-6" />
							<span>Bases</span>
							<span class="text-surface-500 text-xs">{loadedBaseCount}/{totalBaseCount}</span>
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
				<div class="flex flex-col gap-2">
					<div class="flex items-center gap-2">
						<Users class="h-4 w-4" />
						<span class="text-sm font-medium">Load Player</span>
					</div>
					<PlayerList selected={selectedPlayerUid} onselect={handlePlayerLoaded} />
					<p class="text-xs text-gray-500">Select a player to load and show on map</p>
				</div>

				<div class="flex flex-col gap-2">
					<div class="flex items-center gap-2">
						<Building class="h-4 w-4" />
						<span class="text-sm font-medium">Load Guild/Bases</span>
					</div>
					{#if appState.loadingGuild}
						<div class="my-2 flex items-center gap-2 px-3 py-2 text-sm text-gray-400">
							<svg class="h-4 w-4 animate-spin" viewBox="0 0 24 24">
								<circle
									class="opacity-25"
									cx="12"
									cy="12"
									r="10"
									stroke="currentColor"
									stroke-width="4"
									fill="none"
								></circle>
								<path
									class="opacity-75"
									fill="currentColor"
									d="M4 12a8 8 0 018-8V0C5.373 0 0 5.373 0 12h4zm2 5.291A7.962 7.962 0 014 12H0c0 3.042 1.135 5.824 3 7.938l3-2.647z"
								></path>
							</svg>
							Loading guild...
						</div>
					{:else}
						<Combobox
							value={selectedGuildId}
							options={guildSelectOptions}
							placeholder="Select Guild"
							onChange={(value) => handleGuildSelect(value as string)}
							selectClass="w-full"
						/>
					{/if}
					<p class="text-xs text-gray-500">Select a guild to load its bases on the map</p>
				</div>

				<Accordion
					value={section}
					onValueChange={(e: ValueChangeDetails) => (section = e.value)}
					collapsible
				>
					<Accordion.Item value="players" controlHover="hover:bg-secondary-500/25">
						{#snippet control()}
							<h2 class="text-lg font-bold">Loaded Players</h2>
						{/snippet}
						{#snippet panel()}
							{#if mapOptions.showPlayers && loadedPlayerCount > 0}
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
								<p class="text-sm text-gray-500">
									No players loaded yet. Use the selector above to load players.
								</p>
							{/if}
						{/snippet}
					</Accordion.Item>
					<Accordion.Item value="bases" controlHover="hover:bg-secondary-500/25">
						{#snippet control()}
							<h2 class="text-lg font-bold">Loaded Bases</h2>
						{/snippet}
						{#snippet panel()}
							{#if mapOptions.showBases && loadedBaseCount > 0}
								<div class="max-h-64 space-y-2 overflow-y-auto">
									{#each Object.values(bases) as base}
										{#if base.location}
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
										{/if}
									{/each}
								</div>
							{:else}
								<p class="text-sm text-gray-500">
									No bases loaded yet. Use the guild selector above to load bases.
								</p>
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
