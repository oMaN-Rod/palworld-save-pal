<script lang="ts">
	import Icon from '$lib/components/ui/icons/Icon.svelte';
	import { Seo } from '$lib/components/seo';
	import { Loading } from '$components/ui';
	import { getAppState, getModalState, getToastState } from '$states';
	import { worldToPixel, mapOf, DEFAULT_MAP_AREA, type MapArea } from '$components/map/utils';
	import { pixelToLngLat } from '$components/map/mercator';
	import { isWatchtower } from '$components/map/fastTravel';
	import { PAL_SCALE_DEFAULT } from '$components/map/palSize';
	import { MAP_OBJECT_SCALE_DEFAULT } from '$components/map/mapObjectSize';
	import { clampMapOpacity } from '$components/map/mapOpacity';
	import RelicFilterControl from '$components/map/RelicFilterControl.svelte';
	import MapOptionsPanel from '$components/map/MapOptionsPanel.svelte';
	import MapOptionsSheet from '$components/map/MapOptionsSheet.svelte';
	import type { SheetSnap } from '$components/map/mapSheet';
	import PlacementPanel from '$components/map/PlacementPanel.svelte';
	import { mapOptionsState } from '$components/map/mapOptions.svelte';
	import {
		areaFastTravelGuids,
		unlockedInArea,
		relicTypeStats as computeRelicTypeStats,
		orderedRelicTypes,
		pointsInArea
	} from '$components/map/mapCounts';
	import {
		toggleRelicPoint,
		toggleFastTravelPoint,
		unlockFastTravelGuids,
		collectAllRelics
	} from '$components/map/saveMapActions';
	import {
		allVisibilityPatch,
		defaultPanelVisibility,
		type MapLayerVisibility,
		type PanelOptionId
	} from '$components/map/layerPanelModel';
	import { MAP_LAYERS, isMapLayerId, type MapLayerId } from '$components/map/layerRegistry';
	import { mapLayerMarkerCount } from '$components/map/mapLayerFeatures';
	import { mapLayers } from '$lib/data/mapLayerStore.svelte';
	import { dungeons, fastTravelPoints, relics, relicData, bosses } from '$lib/data';
	import { partitionSpawns } from '$components/map/spawns';
	import { placementState } from '$lib/data/placement.svelte';
	import { blueprintsData } from '$lib/data/blueprints.svelte';
	import { baseStructuresData } from '$lib/data/baseStructures.svelte';
	import { browser } from '$app/environment';
	import { isPublicShell } from '$lib/utils/shellRoutes';
	import { isCoarsePointer, isMobileViewport } from '$lib/utils/viewport.svelte';
	import { isWebBuild } from '$lib/utils/platform';
	import { debounce } from '$utils';
	import { sendAndWait } from '$utils/websocketUtils';
	import { EntryState, MessageType } from '$types';
	import type {
		Base,
		FastTravelPoint,
		GuildSummary,
		MapUnlockPoint,
		Player,
		RelicPoint
	} from '$types';
	import { fly } from 'svelte/transition';
	import { cubicOut } from 'svelte/easing';
	import type maplibregl from 'maplibre-gl';
	import * as m from '$i18n/messages';

	const PANEL_W = 420;

	const appState = getAppState();
	const modal = getModalState();
	const toast = getToastState();

	const saveLoaded = $derived(!!appState.saveFile);
	const publicShell = $derived(isPublicShell(isWebBuild, appState.saveFile));

	const mapOptions = $derived(mapOptionsState.current);
	const activeArea = $derived(mapOptions.area ?? DEFAULT_MAP_AREA);
	const panelOpen = $derived(mapOptions.panelOpen ?? true);
	const mapOpacity = $derived(clampMapOpacity(mapOptions.mapOpacity));

	// The sheet's open state is deliberately local: `mapOptions.panelOpen` is
	// persisted and defaults to open, which on a phone would bury the map behind
	// the options on every visit — and a phone toggling it would follow the user
	// back to their desktop.
	const mobile = $derived(isMobileViewport.current);
	const touch = $derived(isCoarsePointer.current);
	let sheetOpen = $state(false);
	let sheetSnap = $state<SheetSnap>('peek');

	const optionsOpen = $derived(mobile ? sheetOpen : panelOpen);

	function toggleOptions() {
		if (mobile) sheetOpen = !sheetOpen;
		else mapOptions.panelOpen = !panelOpen;
	}

	let selectedPlayerUid = $state('');
	let map: maplibregl.Map | undefined = $state(undefined);

	let MapComponent = $state<typeof import('$components/map/Map.svelte').default | undefined>(
		undefined
	);

	// Browser-only, and the rejection is handled. The map needs WebGL, so the
	// server always renders the Loading fallback below no matter what this
	// resolves to — pulling the component in on the server only costs prerender
	// time. It also cannot be left unhandled: this promise is never awaited by
	// the render, so a rejection lands after the response is already sent, where
	// Node treats it as fatal and takes the whole dev server down with it.
	if (browser) {
		import('$components/map/Map.svelte')
			.then((mod) => (MapComponent = mod.default))
			.catch((e) => console.error('Failed to load the map component', e));
	}

	// The editing surfaces stay out of the prerendered public entry: this route is
	// rendered in Node at build time and shipped to visitors with no save at all.
	let saveUi = $state<
		| {
				Panel: typeof import('$components/map/SaveMapPanel.svelte').default;
				Controls: typeof import('$components/map/SaveMapControls.svelte').default;
		  }
		| undefined
	>(undefined);
	let saveUiRequested = false;

	$effect(() => {
		if (!appState.saveFile || saveUiRequested) return;
		saveUiRequested = true;
		Promise.all([
			import('$components/map/SaveMapPanel.svelte'),
			import('$components/map/SaveMapControls.svelte')
		]).then(([panel, controls]) => {
			saveUi = { Panel: panel.default, Controls: controls.default };
		});
	});

	type LayerOptionKey =
		| 'showFastTravel'
		| 'showWatchtower'
		| 'showRelics'
		| 'showDungeons'
		| 'showBosses'
		| 'showAlphaPals'
		| 'showPredatorPals'
		| 'showBounty'
		| 'showOrigin'
		| 'showPlayers'
		| 'showBases'
		| 'showLabels';

	const LEGACY_LAYER_OPTION: Partial<Record<PanelOptionId, LayerOptionKey>> = {
		fast_travel: 'showFastTravel',
		watchtower: 'showWatchtower',
		relics: 'showRelics',
		dungeons: 'showDungeons',
		boss_pals: 'showBosses',
		alpha_pals: 'showAlphaPals',
		predator_pals: 'showPredatorPals',
		bounty: 'showBounty',
		origin: 'showOrigin',
		players: 'showPlayers',
		bases: 'showBases',
		labels: 'showLabels'
	};

	const PANEL_DEFAULTS = defaultPanelVisibility();

	const layerVisibility = $derived.by(() => {
		const record: MapLayerVisibility = { ...(mapOptions.mapLayerVisibility ?? {}) };
		for (const [id, key] of Object.entries(LEGACY_LAYER_OPTION)) {
			record[id as PanelOptionId] = mapOptions[key] ?? PANEL_DEFAULTS[id as PanelOptionId];
		}
		return record;
	});

	function layerAvailable(id: PanelOptionId): boolean {
		return (id !== 'players' && id !== 'bases') || saveLoaded;
	}

	function layerCount(id: PanelOptionId): string | undefined {
		switch (id) {
			case 'fast_travel':
				return fastTravelUnlockedCount !== undefined
					? `${fastTravelUnlockedCount}/${fastTravelCount}`
					: String(fastTravelCount);
			case 'watchtower':
				return watchtowerUnlockedCount !== undefined
					? `${watchtowerUnlockedCount}/${watchtowerCount}`
					: String(watchtowerCount);
			case 'relics':
				return appState.selectedPlayer
					? `${relicCollectedCount}/${relicCount}`
					: String(relicCount);
			case 'players':
				return `${loadedPlayerCount}/${totalPlayerCount}`;
			case 'bases':
				return `${loadedBaseCount}/${totalBaseCount}`;
			case 'dungeons':
				return String(dungeonCount);
			case 'boss_pals':
				return String(bossCount);
			case 'alpha_pals':
				return String(alphaPalCount);
			case 'predator_pals':
				return String(predatorPalCount);
			case 'bounty':
				return String(bountyCount);
			case 'labels':
			case 'origin':
				return undefined;
			default: {
				if (!isMapLayerId(id)) return undefined;
				const selection = mapLayers.peek(id);
				return selection ? mapLayerMarkerCount(id, selection, activeArea).toString() : undefined;
			}
		}
	}

	function handleLayerVisibility(patch: MapLayerVisibility) {
		const registry: MapLayerVisibility = { ...(mapOptions.mapLayerVisibility ?? {}) };
		const enabled: MapLayerId[] = [];
		for (const [key, visible] of Object.entries(patch)) {
			const id = key as PanelOptionId;
			const legacy = LEGACY_LAYER_OPTION[id];
			if (legacy) {
				mapOptions[legacy] = visible;
				continue;
			}
			registry[id] = visible;
			if (visible && isMapLayerId(id)) enabled.push(id);
		}
		mapOptions.mapLayerVisibility = registry;
		if (enabled.length > 0) void mapLayers.getLayers(enabled);
	}

	function handleShowAll(visible: boolean) {
		handleLayerVisibility(allVisibilityPatch(visible));
	}

	// A layer switched on in an earlier session comes back enabled with its
	// artifact never requested, so this reconciles persisted visibility against
	// what's actually cached. Settles after one round: a fetched artifact makes
	// peek() truthy and drops the layer out of `wanted`.
	$effect(() => {
		const wanted = MAP_LAYERS.map((layer) => layer.id).filter(
			(id) => layerVisibility[id] && !mapLayers.peek(id) && !mapLayers.isLoading(id)
		);
		if (wanted.length > 0) void mapLayers.getLayers(wanted);
	});

	const players = $derived(Object.values(appState.players || {}));
	const loadedPlayerCount = $derived(players.length);
	const totalPlayerCount = $derived(Object.keys(appState.playerSummaries || {}).length);
	const guilds = $derived(Object.values(appState.guilds || {}));

	const bases = $derived.by(() =>
		guilds.reduce(
			(acc, guild) => {
				if (guild.bases) {
					Object.values(guild.bases).forEach((base) => {
						acc[base.id] = base;
					});
				}
				return acc;
			},
			{} as Record<string, Base>
		)
	);
	const loadedBaseCount = $derived(Object.keys(bases).length);
	const totalBaseCount = $derived(
		Object.values(appState.guildSummaries || {}).reduce(
			(acc, summary) => acc + (summary as GuildSummary).base_count,
			0
		)
	);

	const areaFtGuids = $derived(areaFastTravelGuids(fastTravelPoints.points, activeArea, false));
	const areaWtGuids = $derived(areaFastTravelGuids(fastTravelPoints.points, activeArea, true));
	const fastTravelCount = $derived(areaFtGuids.size);
	const watchtowerCount = $derived(areaWtGuids.size);
	const fastTravelUnlockedCount = $derived(
		unlockedInArea(areaFtGuids, appState.selectedPlayer?.unlocked_fast_travel_points)
	);
	const watchtowerUnlockedCount = $derived(
		unlockedInArea(areaWtGuids, appState.selectedPlayer?.unlocked_fast_travel_points)
	);

	const relicTypeStats = $derived(
		computeRelicTypeStats(relics.points, activeArea, appState.selectedPlayer ?? undefined)
	);
	const relicTypeList = $derived(
		orderedRelicTypes(relicTypeStats, Object.keys(relicData.relics))
	);
	const relicCount = $derived(
		Object.values(relicTypeStats).reduce((acc, entry) => acc + entry.total, 0)
	);
	const relicCollectedCount = $derived(
		Object.values(relicTypeStats).reduce((acc, entry) => acc + entry.collected, 0)
	);
	const isRelicTypeVisible = (type: string) => mapOptions.relicTypes?.[type] !== false;

	const areaSpawnPartition = $derived.by(() => {
		const partition = partitionSpawns(bosses.points);
		const inArea = (p: { x: number; y: number }) => mapOf(p.x, p.y) === activeArea;
		return {
			alpha: partition.alpha.filter(inArea),
			boss: partition.boss.filter(inArea),
			predator: partition.predator.filter(inArea),
			bounty: partition.bounty.filter(inArea)
		};
	});
	const dungeonCount = $derived(pointsInArea(dungeons.points, activeArea).length);
	const alphaPalCount = $derived(areaSpawnPartition.alpha.length);
	const predatorPalCount = $derived(areaSpawnPartition.predator.length);
	const bossCount = $derived(areaSpawnPartition.boss.length);
	const bountyCount = $derived(areaSpawnPartition.bounty.length);

	function panTo(x: number, y: number, zoom = 4) {
		const area = mapOf(x, y);
		if (!area) return;
		mapOptions.area = area;
		const [px, py] = worldToPixel(x, y, area);
		map?.flyTo({ center: pixelToLngLat(px, py), zoom, duration: 500 });
	}

	function handlePlayerFocus(player: Player) {
		if (!player.location) return;
		panTo(player.location.x, player.location.y);
	}

	// The options panel remounts on each open and re-fires onselect, so this guards
	// the automatic pan to once per player; the per-player focus button always re-centres.
	let autoPannedUid: string | undefined;

	function handlePlayerLoaded(player: Player) {
		selectedPlayerUid = player.uid;
		if (player.location && autoPannedUid !== player.uid) {
			autoPannedUid = player.uid;
			handlePlayerFocus(player);
		}
	}

	$effect(() => {
		if (MapComponent && appState.selectedPlayer && mapOptions.showPlayers) {
			handlePlayerLoaded(appState.selectedPlayer);
		}
	});

	function handleBaseFocus(base: Base) {
		if (!base.location) return;
		panTo(base.location.x, base.location.y);
	}

	async function handleEditBase(base: Base) {
		const { default: EditBaseModal } = await import(
			'$components/modals/edit-base/EditBaseModal.svelte'
		);
		// @ts-expect-error Component typing
		const result = await modal.showModal<{ name: string; area_range: number }>(EditBaseModal, {
			title: m.edit_entity({ entity: m.base({ count: 1 }) }),
			name: base.name || '',
			areaRange: base.area_range || 3500
		});
		if (!result) return;

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

	async function handleExportBlueprint(base: Base) {
		const { default: ExportBlueprintModal } = await import(
			'$components/modals/export-blueprint/ExportBlueprintModal.svelte'
		);
		// @ts-expect-error Component typing
		await modal.showModal<boolean>(ExportBlueprintModal, {
			baseId: base.id,
			baseName: base.name || ''
		});
	}

	async function handleDeleteBase(base: Base) {
		const baseName = base.name || 'Unnamed Base';
		const confirmed = await modal.showConfirmModal({
			title: `Delete base "${baseName}"?`,
			message: 'This removes its structures and pals.',
			confirmText: 'Delete',
			cancelText: 'Cancel'
		});
		if (!confirmed) return;

		const guildEntry = Object.entries(appState.guilds || {}).find(
			([, guild]) => guild.bases && Object.values(guild.bases).some((b) => b.id === base.id)
		);
		if (!guildEntry) return;
		const [guildId] = guildEntry;

		try {
			await sendAndWait(MessageType.DELETE_BASE, { base_id: base.id });
		} catch (e) {
			toast.add(String(e instanceof Error ? e.message : e), 'Failed to delete base', 'error');
			return;
		}

		delete appState.guilds[guildId];
		await appState.loadGuildLazy(guildId);
		baseStructuresData.reset();
		toast.add(`Deleted base "${baseName}".`, 'Base deleted', 'success');
	}

	function handleToggleFastTravel(point: MapUnlockPoint) {
		const player = appState.selectedPlayer;
		if (!player) return;
		toggleFastTravelPoint(player, point.guid);
		player.state = EntryState.MODIFIED;
	}

	function handleToggleRelic(point: RelicPoint) {
		const player = appState.selectedPlayer;
		if (!player) return;
		toggleRelicPoint(player, point);
		player.state = EntryState.MODIFIED;
	}

	function handleToggleStructureType(type: string) {
		const visible = mapOptions.structureTypes?.[type] !== false;
		mapOptions.structureTypes = { ...(mapOptions.structureTypes ?? {}), [type]: !visible };
	}

	function unlockAllWhere(predicate: (point: FastTravelPoint) => boolean) {
		const player = appState.selectedPlayer;
		if (!player) return;
		const guids = Object.entries(fastTravelPoints.points)
			.filter(([, point]) => predicate(point))
			.map(([guid]) => guid);
		if (unlockFastTravelGuids(player, guids) > 0) player.state = EntryState.MODIFIED;
	}

	function handleUnlockAllFastTravel() {
		unlockAllWhere((point) => !isWatchtower(point));
	}

	function handleUnlockAllWatchtowers() {
		unlockAllWhere(isWatchtower);
	}

	function handleCollectAllRelics() {
		const player = appState.selectedPlayer;
		if (!player || !mapOptions.showRelics) return;
		const visible = Object.entries(relics.points)
			.filter(([, relic]) => mapOf(relic.x, relic.y) === activeArea)
			.filter(([, relic]) => isRelicTypeVisible(relic.relic_type))
			.map(([guid, relic]) => ({ guid, relic_type: relic.relic_type }));
		if (collectAllRelics(player, visible) > 0) player.state = EntryState.MODIFIED;
	}

	async function handleUnlockMap() {
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

	const guildOptions = $derived(
		Object.entries(appState.guildSummaries ?? {}).map(([id, summary]) => ({
			value: id,
			label: (summary as GuildSummary).name
		}))
	);
	const playerOptions = $derived(
		Object.values(appState.players ?? {}).map((player) => ({
			value: player.uid,
			label: player.nickname
		}))
	);

	$effect(() => {
		let panToLoc: { x: number; y: number; radius: number } = { x: 0, y: 0, radius: 3500 };
		if (placementState.active && placementState.handle && placementState.geometry.length === 0) {
			blueprintsData
				.requestGeometry(placementState.handle)
				.then((res) => {
					placementState.geometry = res.structures;
					placementState.setAnchor(res.origin);
					panToLoc = {
						x: res.origin.x,
						y: res.origin.y,
						radius: placementState.header?.footprint_radius ?? 3500
					};
				})
				.finally(() => {
					setTimeout(() => {
						panTo(panToLoc.x, panToLoc.y, 7);
					}, 100);
				});
		}
	});

	const debouncedValidate = debounce(() => placementState.runValidate(), 200);

	$effect(() => {
		if (!placementState.active) return;
		void placementState.targetGuild;
		void placementState.anchor;
		debouncedValidate();
	});

	async function handlePlace() {
		let res: Awaited<ReturnType<typeof placementState.commit>>;
		try {
			res = await placementState.commit();
		} catch (e) {
			toast.add(String(e instanceof Error ? e.message : e), 'Placement failed', 'error');
			return;
		}

		const guild = placementState.targetGuild;
		placementState.exit();
		delete appState.guilds[guild];
		await appState.loadGuildLazy(guild);
		baseStructuresData.reset();
		toast.add(`Placed ${res.structures_placed} structures.`, 'Blueprint placed', 'success');

		try {
			await appState.writeSave();
		} catch (e) {
			toast.add(
				String(e instanceof Error ? e.message : e),
				'Placed, but saving to disk failed - use Save to write it.',
				'error'
			);
		}
	}

	function handleCancel() {
		placementState.exit();
	}
</script>

<Seo pathname="/map" title={m.map_meta_title()} description={m.map_meta_description()} />

<div class="relative h-full overflow-hidden">
	<!-- The map has no visible heading; this gives screen readers and crawlers one. -->
	<h1 class="sr-only">{m.map_meta_title()}</h1>

	{#snippet optionsBody()}
		<MapOptionsPanel
			{saveLoaded}
			{touch}
			showHeader={!mobile}
			layers={layerVisibility}
			onVisibilityChange={handleLayerVisibility}
			onShowAll={handleShowAll}
			count={layerCount}
			available={layerAvailable}
			onUnlockMap={handleUnlockMap}
			savePanel={saveUi ? savePanelSnippet : undefined}
		/>
	{/snippet}

	{#snippet savePanelSnippet()}
		{#if saveUi}
			<saveUi.Panel
				hideUnlockedFastTravel={mapOptions.hideUnlockedFastTravel}
				hideCollectedRelics={mapOptions.hideCollectedRelics}
				showPlayers={mapOptions.showPlayers}
				showBases={mapOptions.showBases}
				{selectedPlayerUid}
				onToggleHideUnlocked={() =>
					(mapOptions.hideUnlockedFastTravel = !mapOptions.hideUnlockedFastTravel)}
				onToggleHideCollected={() =>
					(mapOptions.hideCollectedRelics = !mapOptions.hideCollectedRelics)}
				onPlayerLoaded={handlePlayerLoaded}
				onPlayerFocus={handlePlayerFocus}
				onBaseFocus={handleBaseFocus}
				onEditBase={handleEditBase}
			/>
		{/if}
	{/snippet}

	{#if !mobile && panelOpen}
		<aside
			class="bg-surface-900/95 absolute top-2 bottom-2 left-2 z-10 flex w-[420px] flex-col rounded-lg shadow-lg"
			transition:fly={{ x: -(PANEL_W + 16), duration: 300, easing: cubicOut }}
		>
			{@render optionsBody()}
		</aside>
	{/if}

	{#if mobile && sheetOpen}
		<MapOptionsSheet
			bind:snap={sheetSnap}
			title={m.map_options()}
			onClose={() => (sheetOpen = false)}
		>
			{@render optionsBody()}
		</MapOptionsSheet>
	{/if}

	<!-- Cleared of the public shell's nav, which is a full-width bar on phones and a
	     centred pill on desktop — at ~1440px that pill sits directly over this column
	     once the panel pushes it right. The desktop app has a left sidebar and no top
	     nav, so it keeps the original 8px inset. -->
	<div
		class="absolute top-16 z-20 flex flex-col items-start gap-2 md:transition-[left] md:duration-300 md:ease-out {publicShell
			? ''
			: 'md:top-2'}"
		style:left="{!mobile && panelOpen ? PANEL_W + 16 : 8}px"
	>
		<button
			type="button"
			class="bg-surface-900/95 hover:bg-surface-800 rounded-lg p-2 shadow-lg {touch
				? 'min-h-11 min-w-11 flex items-center justify-center'
				: ''}"
			title={m.map_options()}
			aria-label={m.map_options()}
			aria-expanded={optionsOpen}
			onclick={toggleOptions}
		>
			{#if optionsOpen}
				<Icon icon="tabler:layout-sidebar-left-collapse" class="h-5 w-5" />
			{:else}
				<Icon icon="tabler:layout-sidebar" class="h-5 w-5" />
			{/if}
		</button>

		{#if mapOptions.showRelics && relicTypeList.length > 0}
			<RelicFilterControl
				types={relicTypeList}
				stats={relicTypeStats}
				enabled={mapOptions.relicTypes ?? {}}
				showCollected={saveLoaded && !!appState.selectedPlayer}
				{touch}
				ontoggle={(relicType) =>
					(mapOptions.relicTypes = {
						...(mapOptions.relicTypes ?? {}),
						[relicType]: !isRelicTypeVisible(relicType)
					})}
			/>
		{/if}

		{#if saveUi && appState.selectedPlayer}
			<saveUi.Controls
				showRelics={mapOptions.showRelics}
				onUnlockAllFastTravel={handleUnlockAllFastTravel}
				onUnlockAllWatchtowers={handleUnlockAllWatchtowers}
				onCollectAllRelics={handleCollectAllRelics}
			/>
		{/if}
	</div>

	<div class="absolute inset-0">
		{#if MapComponent}
			<MapComponent
				bind:map
				area={activeArea}
				onAreaChange={(next: MapArea) => (mapOptions.area = next)}
				showOrigin={mapOptions.showOrigin}
				showPlayers={saveLoaded && mapOptions.showPlayers}
				showBases={saveLoaded && mapOptions.showBases}
				showFastTravel={mapOptions.showFastTravel}
				showWatchtower={mapOptions.showWatchtower}
				showRelics={mapOptions.showRelics}
				hideCollectedRelics={mapOptions.hideCollectedRelics}
				hideUnlockedFastTravel={mapOptions.hideUnlockedFastTravel}
				relicTypes={mapOptions.relicTypes ?? {}}
				showDungeons={mapOptions.showDungeons}
				showBosses={mapOptions.showBosses}
				showAlphaPals={mapOptions.showAlphaPals}
				showPredatorPals={mapOptions.showPredatorPals}
				showBounty={mapOptions.showBounty}
				mapLayerVisibility={layerVisibility}
				showLabels={mapOptions.showLabels}
				show3d={mapOptions.enable3d}
				showStructureControls={saveLoaded}
				areaSwitchAlign={publicShell ? 'right' : 'center'}
				palSize={mapOptions.palSize ?? PAL_SCALE_DEFAULT}
				palAutoFollow={mapOptions.palAutoFollow}
				palHeight={mapOptions.palHeight ?? 0}
				{mapOpacity}
				fastTravelSize={mapOptions.fastTravelSize ?? MAP_OBJECT_SCALE_DEFAULT}
				watchtowerSize={mapOptions.watchtowerSize ?? MAP_OBJECT_SCALE_DEFAULT}
				relicSize={mapOptions.relicSize ?? MAP_OBJECT_SCALE_DEFAULT}
				structureTypes={mapOptions.structureTypes ?? {}}
				renderMode={mapOptions.structureRenderMode ?? 'detailed'}
				structureTextured={mapOptions.structureTextured}
				onToggle3d={() => (mapOptions.enable3d = !mapOptions.enable3d)}
				onToggleStructureType={handleToggleStructureType}
				onToggleRenderMode={() =>
					(mapOptions.structureRenderMode =
						(mapOptions.structureRenderMode ?? 'detailed') === 'detailed' ? 'flat' : 'detailed')}
				onToggleStructureTextured={() =>
					(mapOptions.structureTextured = !mapOptions.structureTextured)}
				onEditBase={saveLoaded ? handleEditBase : undefined}
				onExportBase={saveLoaded ? handleExportBlueprint : undefined}
				onDeleteBase={saveLoaded ? handleDeleteBase : undefined}
				onToggleFastTravel={saveLoaded ? handleToggleFastTravel : undefined}
				onToggleRelic={saveLoaded ? handleToggleRelic : undefined}
				onTogglePalAutoFollow={() => (mapOptions.palAutoFollow = !mapOptions.palAutoFollow)}
				onPalSizeChange={(scale: number) => (mapOptions.palSize = scale)}
				onFastTravelSizeChange={(scale: number) => (mapOptions.fastTravelSize = scale)}
				onWatchtowerSizeChange={(scale: number) => (mapOptions.watchtowerSize = scale)}
				onRelicSizeChange={(scale: number) => (mapOptions.relicSize = scale)}
				onPalHeightChange={(height: number) => (mapOptions.palHeight = height)}
				onMapOpacityChange={(opacity: number) => (mapOptions.mapOpacity = opacity)}
				placement={placementState.active}
				placementGeometry={placementState.geometry}
				placementAnchor={placementState.anchor}
				onPlacementAnchorChange={(a) => {
					placementState.setAnchor(a);
					debouncedValidate();
				}}
			/>
		{:else}
			<Loading label={m.initializing_entity({ entity: m.map() })} icon="tabler:map" iconSize={24} />
		{/if}

		{#if saveLoaded && placementState.active}
			<PlacementPanel
				{guildOptions}
				{playerOptions}
				onPlace={handlePlace}
				onCancel={handleCancel}
			/>
		{/if}
	</div>
</div>
