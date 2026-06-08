<script lang="ts">
	import { Map } from '$components';
	import { PlayerList } from '$components/player';
	import { Button, Combobox } from '$components/ui';
	import { getAppState, getModalState, getToastState } from '$states';
	import { worldToPixel, worldToMap } from '$components/map/utils';
	import { Accordion } from '@skeletonlabs/skeleton-svelte';
	import { mapImg } from '$components/map/styles';
	import { Target, Unlock, Users, Building, Loader2, Map as MapIcon } from 'lucide-svelte';
	import { mapObjects } from '$lib/data';
	import type { Map as OLMap } from 'ol';
	import type { Base, GuildSummary, Player } from '$types';
	import { assetLoader } from '$utils';
	import { EditBaseModal } from '$components/modals';
	import { EntryState, MessageType } from '$types';
	import { staticIcons } from '$types/icons';
	import { persistedState } from 'svelte-persisted-state';
	import type { ValueChangeDetails } from '@zag-js/accordion';
	import { sendAndWait } from '$utils/websocketUtils';
	import { SectionHeader } from '$components/ui';
	import * as m from '$i18n/messages';
	import { c, p } from '$lib/utils/commonTranslations';

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

	const mapLoader = import('$components/map/Map.svelte');

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
					? `\u25A0 ${summary.name} (${summary.base_count} bases)`
					: `\u25A1 ${summary.name} (${summary.base_count} bases)`
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

	async function handleEditBase(base: Base) {
		// @ts-ignore
		const result = await modal.showModal<{ name: string; area_range: number }>(EditBaseModal, {
			title: m.edit_entity({ entity: m.base({ count: 1 }) }),
			name: base.name || '',
			areaRange: base.area_range || 3500
		});
		if (!result) return;

		// Find the guild that contains this base
		const guild = Object.values(appState.guilds || {}).find(
			(g) => g.bases && Object.values(g.bases).some((b) => b.id === base.id)
		);

		if (guild && guild.bases) {
			const baseInGuild = Object.values(guild.bases).find((b) => b.id === base.id);
			if (baseInGuild) {
				baseInGuild.name = result.name;
				baseInGuild.area_range = result.area_range;
				guild.state = EntryState.MODIFIED;
			}
		}
	}

	async function handleUnlockMap() {
		// @ts-ignore
		const confirmed = await modal.showConfirmModal({
			title: m.unlock_full_map(),
			message: m.unlock_map_confirm(),
			confirmText: m.select_entity({ entity: m.file({ count: 1 }) }),
			cancelText: m.cancel()
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

	let loadingComplete = $state(false);
	let dismissLoading = $state(false);
	let MapComponent: typeof import('$components/map/Map.svelte').default | undefined = $state();

	const LOADING_MIN_MS = 1200;

	$effect(() => {
		if (loadingComplete && appState.selectedPlayer && mapOptionsState.current.showPlayers) {
			handlePlayerLoaded(appState.selectedPlayer);
		}
	});

	$effect(() => {
		if (loadingComplete) return;
		let cancelled = false;
		const start = performance.now();

		mapLoader.then((module) => {
			MapComponent = module.default;
			const elapsed = performance.now() - start;
			const remaining = Math.max(0, LOADING_MIN_MS - elapsed);
			setTimeout(() => {
				if (!cancelled) {
					loadingComplete = true;
					dismissLoading = true;
					setTimeout(() => {
						dismissLoading = false;
					}, 1000);
				}
			}, remaining);
		});

		return () => {
			cancelled = true;
		};
	});
</script>

<div class="relative h-full overflow-hidden">
	{#if dismissLoading || !loadingComplete}
		<div class="loading-overlay" class:loading-dismiss={loadingComplete}>
			<div class="loading-content">
				<div class="relative">
					<Loader2
						size={64}
						class="text-secondary-400 animate-spin"
						style="filter: drop-shadow(0 0 20px color-mix(in srgb, var(--color-secondary-400) 50%, transparent));"
					/>
					<MapIcon size={24} class="text-secondary-300 absolute inset-0 m-auto" />
				</div>
				<p class="loading-text">INITIALIZING MAP</p>
				<div class="loading-bar-track">
					<div class="loading-bar-fill" class:loading-bar-done={loadingComplete}></div>
				</div>
			</div>
		</div>
	{/if}

	<div class="grid h-full grid-cols-[20%_1fr] gap-2" class:page-blurred={!loadingComplete}>
		<div class="flex flex-col gap-4 p-4">
			<div class="flex flex-col gap-4">
				<div class="flex flex-col gap-2">
					<div class="flex items-center">
						<SectionHeader text={m.map_options()}>
							{#snippet action()}
								<Button
									variant="ghost"
									size="sm"
									class="flex items-center gap-2"
									onclick={handleUnlockMap}
								>
									<Unlock class="h-4 w-4" />
									<span>{m.unlock_map()}</span>
								</Button>
							{/snippet}
						</SectionHeader>
					</div>
					<div class="grid grid-cols-2 gap-2">
						<button
							class="flex items-center space-x-2 {mapOptions.showOrigin ? '' : 'opacity-25'}"
							onclick={() => (mapOptions.showOrigin = !mapOptions.showOrigin)}
						>
							<Target class="mr-2 h-6 w-6" />
							<span>{m.origin()}</span>
						</button>
						<button
							class="flex items-center space-x-2 {mapOptions.showFastTravel ? '' : 'opacity-25'} "
							onclick={() => (mapOptions.showFastTravel = !mapOptions.showFastTravel)}
						>
							<img src={mapImg.fastTravel} alt={m.fast_travel()} class="mr-2 h-6 w-6" />
							<span>{m.fast_travel()}</span>
							<span class="text-surface-500 text-xs">{fastTravelCount}</span>
						</button>
						{#if appState.saveFile}
							<button
								class="flex items-center space-x-2 {mapOptions.showPlayers ? '' : 'opacity-25'}"
								onclick={() => (mapOptions.showPlayers = !mapOptions.showPlayers)}
							>
								<img src={mapImg.player} alt={m.player({ count: 2 })} class="mr-2 h-6 w-6" />
								<span>{m.player({ count: 1 })}</span>
								<span class="text-surface-500 text-xs">{loadedPlayerCount}/{totalPlayerCount}</span>
							</button>
							<button
								class="flex items-center space-x-2 {mapOptions.showBases ? '' : 'opacity-25'}"
								onclick={() => (mapOptions.showBases = !mapOptions.showBases)}
							>
								<img src={mapImg.baseCamp} alt={m.base({ count: 2 })} class="mr-2 h-6 w-6" />
								<span>{m.base({ count: 2 })}</span>
								<span class="text-surface-500 text-xs">{loadedBaseCount}/{totalBaseCount}</span>
							</button>
						{/if}

						<button
							class="flex items-center space-x-2 {mapOptions.showDungeons ? '' : 'opacity-25'}"
							onclick={() => (mapOptions.showDungeons = !mapOptions.showDungeons)}
						>
							<img src={mapImg.dungeon} alt={m.dungeons()} class="mr-2 h-6 w-6" />
							<span>{m.dungeons()}</span>
							<span class="text-surface-500 text-xs">{dungeonCount}</span>
						</button>
						<button
							class="flex items-center space-x-2 {mapOptions.showAlphaPals ? '' : 'opacity-25'}"
							onclick={() => (mapOptions.showAlphaPals = !mapOptions.showAlphaPals)}
						>
							<img src={anubisImg} alt={m.alpha_pal(p.pals)} class="mr-2 h-6 w-6" />
							<span>{m.alpha_pal(p.pals)}</span>
							<span class="text-surface-500 text-xs">{alphaPalCount}</span>
						</button>
						<button
							class="flex items-center space-x-2 {mapOptions.showPredatorPals ? '' : 'opacity-25'}"
							onclick={() => (mapOptions.showPredatorPals = !mapOptions.showPredatorPals)}
						>
							<img src={starryonImg} alt={m.predator_pals(p.pals)} class="mr-2 h-6 w-6" />
							<span>{m.predator_pals(p.pals)}</span>
							<span class="text-surface-500 text-xs">{predatorPalCount}</span>
						</button>
					</div>
				</div>
				{#if appState.saveFile}
					<div class="flex flex-col gap-2">
						<div class="flex items-center gap-2">
							<Users class="h-4 w-4" />
							<span class="text-sm font-medium">{m.load_player()}</span>
						</div>
						<PlayerList
							selected={selectedPlayerUid}
							onselect={handlePlayerLoaded}
							redirect={false}
						/>
					</div>
					<div class="flex flex-col gap-2">
						<div class="flex items-center gap-2">
							<Building class="h-4 w-4" />
							<span class="text-sm font-medium">{m.load_guild_bases()}</span>
						</div>
						{#if appState.loadingGuild}
							<div class="text-surface-400 my-2 flex items-center gap-2 px-3 py-2 text-sm">
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
								{m.loading_entity({ entity: m.guild({ count: 1 }) })}
							</div>
						{:else}
							<Combobox
								value={selectedGuildId}
								options={guildSelectOptions}
								placeholder={m.select_entity({ entity: m.guild({ count: 1 }) })}
								onChange={(value) => handleGuildSelect(value as string)}
								selectClass="w-full"
							/>
						{/if}
						<p class="text-surface-500 text-xs">{m.select_guild_to_load_bases()}</p>
					</div>

					<Accordion
						value={section}
						onValueChange={(e: ValueChangeDetails) => (section = e.value)}
						collapsible
					>
						<Accordion.Item value="players" controlHover="hover:bg-secondary-500/25">
							{#snippet control()}
								<h2 class="text-lg font-bold">
									{m.loaded_entity({ entity: m.player({ count: 2 }) })}
								</h2>
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
													<div class="truncate font-bold">{player.nickname}</div>
													<div class="text-xs">
														{m.level()}: {player.level} | {m.hp()}: {player.hp}
													</div>
													<div class="text-surface-400 text-xs">
														{m.location()}: {Math.round(mapCoords.x)}, {Math.round(mapCoords.y)}
													</div>
													<div class="text-surface-400 text-xs">
														{m.last_online()}: {new Date(player.last_online_time).toLocaleString()}
													</div>
												</button>
											{/if}
										{/each}
									</div>
								{:else}
									<p class="text-surface-500 text-sm">
										{m.no_players_loaded()}
									</p>
								{/if}
							{/snippet}
						</Accordion.Item>
						<Accordion.Item value="bases" controlHover="hover:bg-secondary-500/25">
							{#snippet control()}
								<h2 class="text-lg font-bold">
									{m.loaded_entity({ entity: m.base({ count: 2 }) })}
								</h2>
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
														handleEditBase(base);
													}}
												>
													<div class="truncate font-bold">{base.name}</div>
													<div class="text-surface-400 text-xs">
														{m.id()}: {base.id}
													</div>
													<div class="text-surface-400 text-xs">
														{m.location()}: {worldToMap(base.location.x, base.location.y).x}, {worldToMap(
															base.location.x,
															base.location.y
														).y}
													</div>
												</button>
											{/if}
										{/each}
									</div>
								{:else}
									<p class="text-surface-500 text-sm">
										{m.no_bases_loaded()}
									</p>
								{/if}
							{/snippet}
						</Accordion.Item>
					</Accordion>
				{/if}

				<div class="mt-auto flex flex-col gap-2">
					<p class="text-surface-500 text-sm">{m.click_map_coordinates()}</p>
					<div class="flex flex-col">
						<div class="flex items-center gap-2">
							<img src={staticIcons.leftClickIcon} alt="Left Click" class=" h-6 w-6" />
							<span class="text-surface-500 text-xs">{m.left_click_focus()}</span>
						</div>
						<div class="flex items-center gap-2">
							<img src={staticIcons.rightClickIcon} alt="Right Click" class=" h-6 w-6" />
							<span class="text-surface-500 text-xs">{m.right_click_edit_base()}</span>
						</div>
					</div>
				</div>
			</div>
		</div>
		<div class="relative h-full w-full overflow-hidden">
			{#if MapComponent}
				<MapComponent
					bind:map
					showOrigin={mapOptions.showOrigin}
					showPlayers={mapOptions.showPlayers}
					showBases={mapOptions.showBases}
					showFastTravel={mapOptions.showFastTravel}
					showDungeons={mapOptions.showDungeons}
					showAlphaPals={mapOptions.showAlphaPals}
					showPredatorPals={mapOptions.showPredatorPals}
					onEditBase={handleEditBase}
				/>
			{/if}
		</div>
	</div>
</div>

<style>
	.loading-overlay {
		position: absolute;
		inset: 0;
		z-index: 100;
		display: flex;
		align-items: center;
		justify-content: center;
		background: color-mix(in srgb, var(--color-surface-950) 95%, transparent);
		backdrop-filter: blur(4px);
		transition:
			opacity 0.5s ease-out,
			transform 0.5s ease-out,
			filter 0.3s ease-out;
	}

	.loading-overlay.loading-dismiss {
		opacity: 0;
		transform: scale(1.05);
		filter: blur(2px);
		pointer-events: none;
	}

	.loading-content {
		display: flex;
		flex-direction: column;
		align-items: center;
		gap: 1.5rem;
		animation: loading-float 2s ease-in-out infinite;
	}

	@keyframes loading-float {
		0%,
		100% {
			transform: translateY(0);
		}
		50% {
			transform: translateY(-8px);
		}
	}

	.loading-text {
		color: color-mix(in srgb, var(--color-secondary-400) 70%, transparent);
		font-size: 0.75rem;
		letter-spacing: 0.2em;
		text-transform: uppercase;
		animation: loading-pulse 1.5s ease-in-out infinite;
		position: relative;
	}

	.loading-text::after {
		content: '';
		position: absolute;
		bottom: -4px;
		left: 0;
		width: 100%;
		height: 1px;
		background: linear-gradient(90deg, transparent, var(--color-secondary-400), transparent);
		animation: loading-scan 2s ease-in-out infinite;
	}

	@keyframes loading-pulse {
		0%,
		100% {
			opacity: 0.6;
		}
		50% {
			opacity: 1;
		}
	}

	@keyframes loading-scan {
		0% {
			transform: scaleX(0.3);
			opacity: 0;
		}
		50% {
			transform: scaleX(1);
			opacity: 1;
		}
		100% {
			transform: scaleX(0.3);
			opacity: 0;
		}
	}

	.loading-bar-track {
		width: 200px;
		height: 2px;
		background: color-mix(in srgb, var(--color-secondary-400) 15%, transparent);
		border-radius: 1px;
		overflow: hidden;
	}

	.loading-bar-fill {
		height: 100%;
		width: 0%;
		background: var(--color-secondary-400);
		border-radius: 1px;
		transition: width 0.3s ease-out;
	}

	.loading-bar-fill.loading-bar-done {
		width: 100%;
	}

	.page-blurred {
		filter: blur(2px);
		transition: filter 0.5s ease-out;
		pointer-events: none;
	}
</style>
