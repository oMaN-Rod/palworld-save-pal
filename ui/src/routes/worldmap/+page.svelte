<script lang="ts">
	import { PlayerList } from '$components/player';
	import { Button, Combobox, Loading } from '$components/ui';
	import { getAppState, getModalState, getToastState } from '$states';
	import {
		worldToPixel,
		worldToMap,
		mapOf,
		DEFAULT_MAP_AREA,
		type MapArea
	} from '$components/map/utils';
	import { collectRelics, relicsByType, toggleRelic } from '$components/map/relics';
	import { Accordion } from '@skeletonlabs/skeleton-svelte';
	import { mapImg, relicTypeIcon } from '$components/map/styles';
	import { isWatchtower } from '$components/map/fastTravel';
	import Target from '@lucide/svelte/icons/target';
	import Unlock from '@lucide/svelte/icons/unlock';
	import Users from '@lucide/svelte/icons/users';
	import MapIcon from '@lucide/svelte/icons/map';
	import Building from '@lucide/svelte/icons/building';
	import { mapObjects, fastTravelPoints, relics, relicData, bosses } from '$lib/data';
	import type { Map as OLMap } from 'ol';
	import type { Base, FastTravelPoint, GuildSummary, MapUnlockPoint, Player, RelicPoint } from '$types';
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
		area: MapArea;
		showOrigin: boolean;
		showPlayers: boolean;
		showBases: boolean;
		showFastTravel: boolean;
		showWatchtower: boolean;
		showRelics: boolean;
		/** Per-relic-type visibility; a missing key means visible. */
		relicTypes: Record<string, boolean>;
		showDungeons: boolean;
		showBosses: boolean;
		showAlphaPals: boolean;
		showPredatorPals: boolean;
	};

	const mapOptionsState = persistedState<MapOptions>('mapOptions', {
		area: DEFAULT_MAP_AREA,
		showOrigin: false,
		showPlayers: true,
		showBases: true,
		showFastTravel: true,
		showWatchtower: true,
		showRelics: true,
		relicTypes: {},
		showDungeons: true,
		showBosses: true,
		showAlphaPals: true,
		showPredatorPals: true
	});
	const mapOptions = $derived(mapOptionsState.current);
	const activeArea = $derived(mapOptions.area ?? DEFAULT_MAP_AREA);
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
	// Every count below is scoped to the active map area, matching what Map.svelte draws.
	const areaFastTravelGuids = $derived(
		new Set(
			Object.entries(fastTravelPoints.points)
				.filter(([, point]) => !isWatchtower(point) && mapOf(point.x, point.y) === activeArea)
				.map(([guid]) => guid.toUpperCase())
		)
	);
	const areaWatchtowerGuids = $derived(
		new Set(
			Object.entries(fastTravelPoints.points)
				.filter(([, point]) => isWatchtower(point) && mapOf(point.x, point.y) === activeArea)
				.map(([guid]) => guid.toUpperCase())
		)
	);
	const fastTravelCount = $derived(areaFastTravelGuids.size);
	const fastTravelUnlockedCount = $derived.by(() => {
		const unlocked = appState.selectedPlayer?.unlocked_fast_travel_points;
		if (!unlocked) return undefined;
		return unlocked.filter((guid) => areaFastTravelGuids.has(guid.toUpperCase())).length;
	});
	const watchtowerCount = $derived(areaWatchtowerGuids.size);
	const watchtowerUnlockedCount = $derived.by(() => {
		const unlocked = appState.selectedPlayer?.unlocked_fast_travel_points;
		if (!unlocked) return undefined;
		return unlocked.filter((guid) => areaWatchtowerGuids.has(guid.toUpperCase())).length;
	});
	const relicTypeStats = $derived.by(() => {
		const player = appState.selectedPlayer;
		const collectedSets: Record<string, Set<string>> = {};
		for (const [type, guids] of Object.entries(player ? relicsByType(player) : {})) {
			collectedSets[type] = new Set(guids.map((guid) => guid.toUpperCase()));
		}
		const stats: Record<string, { total: number; collected: number }> = {};
		for (const [guid, relic] of Object.entries(relics.points)) {
			if (mapOf(relic.x, relic.y) !== activeArea) continue;
			const entry = (stats[relic.relic_type] ??= { total: 0, collected: 0 });
			entry.total++;
			if (collectedSets[relic.relic_type]?.has(guid.toUpperCase())) entry.collected++;
		}
		return stats;
	});

	// Game order (relic_data.json), restricted to types that exist on this map.
	const relicTypeList = $derived.by(() => {
		const present = Object.keys(relicTypeStats);
		const ordered = Object.keys(relicData.relicData).filter((type) => present.includes(type));
		return [...ordered, ...present.filter((type) => !ordered.includes(type))];
	});

	const relicCount = $derived(
		Object.values(relicTypeStats).reduce((acc, entry) => acc + entry.total, 0)
	);
	const relicCollectedCount = $derived(
		Object.values(relicTypeStats).reduce((acc, entry) => acc + entry.collected, 0)
	);
	const isRelicTypeVisible = (type: string) => mapOptions.relicTypes?.[type] !== false;
	const areaMapObjectCounts = $derived.by(() => {
		const counts: Record<string, number> = {};
		for (const point of Object.values(mapObjects.points)) {
			if (mapOf(point.x, point.y) !== activeArea) continue;
			counts[point.type] = (counts[point.type] ?? 0) + 1;
		}
		return counts;
	});
	const dungeonCount = $derived(areaMapObjectCounts['dungeon'] ?? 0);
	const alphaPalCount = $derived(areaMapObjectCounts['alpha_pal'] ?? 0);
	const predatorPalCount = $derived(areaMapObjectCounts['predator_pal'] ?? 0);
	const bossCount = $derived(
		Object.values(bosses.points).filter((b) => mapOf(b.x, b.y) === activeArea).length
	);
	const anubisImg = $derived(assetLoader.loadMenuImage('anubis'));
	const starryonImg = $derived(assetLoader.loadMenuImage('nightbluehorse'));

	function panTo(x: number, y: number) {
		const area = mapOf(x, y);
		if (!area) return;
		mapOptions.area = area;
		const coords = worldToPixel(x, y, area);
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

	function updateRelicCount(player: Player, delta: number) {
		const container = player.essential_container;
		if (!container) return;
		const slot = container.slots.find((s) => s.static_id === 'Relic');
		if (slot) {
			slot.count += delta;
			if (slot.count <= 0) {
				slot.static_id = 'None';
				slot.count = 0;
				slot.dynamic_item = undefined;
			}
		} else if (delta > 0) {
			const usedIndexes = new Set(
				container.slots.filter((s) => s.static_id !== 'None').map((s) => s.slot_index)
			);
			let slotIndex = 0;
			while (usedIndexes.has(slotIndex)) slotIndex++;
			const emptySlot = container.slots.find((s) => s.slot_index === slotIndex);
			if (emptySlot) {
				emptySlot.static_id = 'Relic';
				emptySlot.count = delta;
				emptySlot.dynamic_item = undefined;
			} else {
				container.slots.push({
					static_id: 'Relic',
					slot_index: slotIndex,
					count: delta,
					dynamic_item: undefined
				});
			}
		}
	}

	function handleToggleFastTravel(point: MapUnlockPoint) {
		const player = appState.selectedPlayer;
		if (!player) return;
		const unlocks = player.unlocked_fast_travel_points ?? [];
		const index = unlocks.findIndex((guid) => guid.toUpperCase() === point.guid.toUpperCase());
		if (index >= 0) {
			player.unlocked_fast_travel_points.splice(index, 1);
		} else {
			player.unlocked_fast_travel_points.push(point.guid);
		}
		player.state = EntryState.MODIFIED;
	}

	function handleToggleRelic(point: RelicPoint) {
		const player = appState.selectedPlayer;
		if (!player) return;
		const delta = toggleRelic(player, point);
		if (delta !== 0) updateRelicCount(player, delta);
		player.state = EntryState.MODIFIED;
	}

	function unlockAllWhere(predicate: (point: FastTravelPoint) => boolean) {
		const player = appState.selectedPlayer;
		if (!player) return;
		const unlocked = player.unlocked_fast_travel_points ?? [];
		const existing = new Set(unlocked.map((guid) => guid.toUpperCase()));
		const toAdd = Object.entries(fastTravelPoints.points)
			.filter(([guid, point]) => predicate(point) && !existing.has(guid.toUpperCase()))
			.map(([guid]) => guid);
		if (toAdd.length === 0) return;
		player.unlocked_fast_travel_points = [...unlocked, ...toAdd];
		player.state = EntryState.MODIFIED;
	}

	function handleUnlockAllFastTravel() {
		unlockAllWhere((point) => !isWatchtower(point));
	}

	function handleUnlockAllWatchtowers() {
		unlockAllWhere(isWatchtower);
	}

	// Only the active map area and the currently visible types, so this can never
	// write GUIDs the user cannot see.
	function handleCollectAllRelics() {
		const player = appState.selectedPlayer;
		if (!player) return;
		if (!(mapOptions.showRelics ?? true)) return;
		const visible = Object.entries(relics.points)
			.filter(([, relic]) => mapOf(relic.x, relic.y) === activeArea)
			.filter(([, relic]) => isRelicTypeVisible(relic.relic_type))
			.map(([guid, relic]) => ({ guid, relic_type: relic.relic_type }));
		const { added, capturePowerAdded } = collectRelics(player, visible);
		if (added === 0) return;
		if (capturePowerAdded > 0) updateRelicCount(player, capturePowerAdded);
		player.state = EntryState.MODIFIED;
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
		<Loading
			loadingComplete={loadingComplete}
			label={m.initializing_entity({entity: m.map()})}
			icon={MapIcon}
			iconSize={24} />
	{/if}

	<div class="grid h-full grid-cols-[420px_1fr] gap-2" class:page-blurred={!loadingComplete}>
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
							<span class="text-surface-500 text-xs">
								{fastTravelUnlockedCount !== undefined
									? `${fastTravelUnlockedCount}/${fastTravelCount}`
									: fastTravelCount}
							</span>
						</button>
						<button
							class="flex items-center space-x-2 {(mapOptions.showWatchtower ?? true) ? '' : 'opacity-25'} "
							onclick={() => (mapOptions.showWatchtower = !(mapOptions.showWatchtower ?? true))}
						>
							<img src={mapImg.watchTower} alt={m.watchtower()} class="mr-2 h-6 w-6" />
							<span>{m.watchtower()}</span>
							<span class="text-surface-500 text-xs">
								{watchtowerUnlockedCount !== undefined
									? `${watchtowerUnlockedCount}/${watchtowerCount}`
									: watchtowerCount}
							</span>
						</button>
						<button
							class="flex items-center space-x-2 {(mapOptions.showRelics ?? true)
								? ''
								: 'opacity-25'} "
							onclick={() => (mapOptions.showRelics = !(mapOptions.showRelics ?? true))}
						>
							<img src={mapImg.effigy} alt={m.relics()} class="mr-2 h-6 w-6" />
							<span>{m.relics()}</span>
							<span class="text-surface-500 text-xs">
								{appState.selectedPlayer
									? `${relicCollectedCount}/${relicCount}`
									: relicCount}
							</span>
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
							class="flex items-center space-x-2 {(mapOptions.showBosses ?? true) ? '' : 'opacity-25'}"
							onclick={() => (mapOptions.showBosses = !(mapOptions.showBosses ?? true))}
						>
							<img src={mapImg.boss} alt={m.bosses()} class="mr-2 h-6 w-6" />
							<span>{m.bosses()}</span>
							<span class="text-surface-500 text-xs">{bossCount}</span>
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
					{#if (mapOptions.showRelics ?? true) && relicTypeList.length > 0}
						<div class="border-surface-700 grid grid-cols-2 gap-2 rounded-sm border p-2">
							{#each relicTypeList as relicType (relicType)}
								{@const stats = relicTypeStats[relicType]}
								<button
									class="flex items-center space-x-2 {isRelicTypeVisible(relicType)
										? ''
										: 'opacity-25'}"
									onclick={() =>
										(mapOptions.relicTypes = {
											...(mapOptions.relicTypes ?? {}),
											[relicType]: !isRelicTypeVisible(relicType)
										})}
								>
									<img
										src={relicTypeIcon(relicType)}
										alt={relicData.relicData[relicType]?.localized_name ?? relicType}
										class="mr-1 h-5 w-5"
									/>
									<span class="truncate text-xs">
										{relicData.relicData[relicType]?.localized_name ?? relicType}
									</span>
									<span class="text-surface-500 text-xs">
										{appState.selectedPlayer
											? `${stats.collected}/${stats.total}`
											: stats.total}
									</span>
								</button>
							{/each}
						</div>
					{/if}
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
						{#if mapOptions.showPlayers}
							<Accordion.Item value="players" controlHover="hover:bg-secondary-500/25">
								{#snippet control()}
									<h2 class="text-lg font-bold">
										{m.loaded_entity({ entity: m.player({ count: 2 }) })}
									</h2>
								{/snippet}
								{#snippet panel()}
									{#if loadedPlayerCount > 0}
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
															{m.last_online()}: {new Date(
																player.last_online_time
															).toLocaleString()}
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
						{/if}
						{#if mapOptions.showBases}
							<Accordion.Item value="bases" controlHover="hover:bg-secondary-500/25">
								{#snippet control()}
									<h2 class="text-lg font-bold">
										{m.loaded_entity({ entity: m.base({ count: 2 }) })}
									</h2>
								{/snippet}
								{#snippet panel()}
									{#if loadedBaseCount > 0}
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
						{/if}
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
							<img src={staticIcons.leftClickIcon} alt="Left Click" class=" h-6 w-6" />
							<span class="text-surface-500 text-xs">{m.click_toggle_point()}</span>
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
					area={activeArea}
					onAreaChange={(next) => (mapOptions.area = next)}
					showOrigin={mapOptions.showOrigin}
					showPlayers={mapOptions.showPlayers}
					showBases={mapOptions.showBases}
					showFastTravel={mapOptions.showFastTravel}
					showWatchtower={mapOptions.showWatchtower ?? true}
					showRelics={mapOptions.showRelics ?? true}
					relicTypes={mapOptions.relicTypes ?? {}}
					showDungeons={mapOptions.showDungeons}
					showBosses={mapOptions.showBosses ?? true}
					showAlphaPals={mapOptions.showAlphaPals}
					showPredatorPals={mapOptions.showPredatorPals}
					onEditBase={handleEditBase}
					onToggleFastTravel={handleToggleFastTravel}
					onToggleRelic={handleToggleRelic}
					onUnlockAllFastTravel={handleUnlockAllFastTravel}
					onUnlockAllWatchtowers={handleUnlockAllWatchtowers}
					onCollectAllRelics={handleCollectAllRelics}
				/>
			{/if}
		</div>
	</div>
</div>
