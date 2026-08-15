<script lang="ts">
	import { getAppState } from '$states';
	import { goto } from '$app/navigation';
	import { worldToPixel, mapOf, DEFAULT_MAP_AREA, type MapArea } from '$components/map/utils';
	import { pixelToLngLat } from '$components/map/mercator';
	import { isWatchtower } from '$components/map/fastTravel';
	import { mapImg } from '$components/map/styles';
	import { PAL_SCALE_DEFAULT } from '$components/map/palSize';
	import {
		MAP_OBJECT_SCALE_DEFAULT,
		MAP_OBJECT_WATCHTOWER_SCALE_DEFAULT
	} from '$components/map/mapObjectSize';
	import { clampMapOpacity } from '$components/map/mapOpacity';
	import RelicFilterControl from '$components/map/RelicFilterControl.svelte';
	import MapHints from '$components/map/MapHints.svelte';
	import { Loading, SectionHeader } from '$components/ui';
	import { dungeons, fastTravelPoints, relics, relicData, bosses } from '$lib/data';
	import { partitionSpawns } from '$components/map/spawns';
	import { assetLoader } from '$utils';
	import { persistedState } from 'svelte-persisted-state';
	import { fly } from 'svelte/transition';
	import { cubicOut } from 'svelte/easing';
	import type maplibregl from 'maplibre-gl';
	import Target from '@lucide/svelte/icons/target';
	import PanelLeft from '@lucide/svelte/icons/panel-left';
	import PanelLeftClose from '@lucide/svelte/icons/panel-left-close';
	import { Eye, EyeOff } from '@lucide/svelte';
	import * as m from '$i18n/messages';
	import { p } from '$lib/utils/commonTranslations';

	type PublicMapOptions = {
		area: MapArea;
		showOrigin: boolean;
		showFastTravel: boolean;
		showWatchtower: boolean;
		showRelics: boolean;
		/** Per-relic-type visibility; a missing key means visible. */
		relicTypes: Record<string, boolean>;
		showDungeons: boolean;
		showBosses: boolean;
		showAlphaPals: boolean;
		showPredatorPals: boolean;
		showLabels: boolean;
		enable3d: boolean;
		structureRenderMode: 'detailed' | 'flat';
		panelOpen: boolean;
		/** Pal render scale as a multiple of true size. */
		palSize: number;
		/** Whether Pals turn to face the camera; north-facing when off. */
		palAutoFollow: boolean;
		/** Vertical offset above ground, in world centimetres. */
		palHeight: number;
		/** Raster opacity, cross-fading toward the hillshade relief beneath it. */
		mapOpacity: number;
		/** Fast travel statue render scale as a multiple of true size. */
		fastTravelSize: number;
		/** Watchtower render scale as a multiple of true size. */
		watchtowerSize: number;
		/** Relic render scale as a multiple of true size. */
		relicSize: number;
	};

	const PANEL_W = 420;
	const appState = getAppState();

	// Separate key from /worldmap: persistedState does not merge new defaults
	// into an existing stored object.
	const optionsState = persistedState<PublicMapOptions>('psp-public-map-options', {
		area: DEFAULT_MAP_AREA,
		showOrigin: false,
		showFastTravel: true,
		showWatchtower: true,
		showRelics: true,
		relicTypes: {},
		showDungeons: true,
		showBosses: true,
		showAlphaPals: true,
		showPredatorPals: true,
		showLabels: true,
		enable3d: false,
		structureRenderMode: 'detailed',
		panelOpen: true,
		palSize: PAL_SCALE_DEFAULT,
		palAutoFollow: true,
		palHeight: 0,
		mapOpacity: 1,
		fastTravelSize: MAP_OBJECT_SCALE_DEFAULT,
		watchtowerSize: MAP_OBJECT_WATCHTOWER_SCALE_DEFAULT,
		relicSize: MAP_OBJECT_SCALE_DEFAULT
	});

	const options = $derived(optionsState.current);
	const activeArea = $derived(options.area ?? DEFAULT_MAP_AREA);
	const panelOpen = $derived(options.panelOpen ?? true);
	const mapOpacity = $derived(clampMapOpacity(options.mapOpacity));

	let map: maplibregl.Map | undefined = $state(undefined);

	const mapLoader = import('$components/map/Map.svelte');
	let MapComponent = $state<typeof import('$components/map/Map.svelte').default | undefined>(
		undefined
	);

	mapLoader.then((mod) => (MapComponent = mod.default));

	$effect(() => {
		if (appState.saveFile) goto('/worldmap');
	});

	// Every count below is scoped to the active map area, matching what Map.svelte draws.
	const fastTravelCount = $derived(
		Object.values(fastTravelPoints.points).filter(
			(point) => !isWatchtower(point) && mapOf(point.x, point.y) === activeArea
		).length
	);
	const watchtowerCount = $derived(
		Object.values(fastTravelPoints.points).filter(
			(point) => isWatchtower(point) && mapOf(point.x, point.y) === activeArea
		).length
	);

	const relicTypeTotals = $derived.by(() => {
		const totals: Record<string, number> = {};
		for (const relic of Object.values(relics.points)) {
			if (mapOf(relic.x, relic.y) !== activeArea) continue;
			totals[relic.relic_type] = (totals[relic.relic_type] ?? 0) + 1;
		}
		return totals;
	});

	// Game order (relic_data.json), restricted to types that exist on this map.
	const relicTypeList = $derived.by(() => {
		const present = Object.keys(relicTypeTotals);
		const ordered = Object.keys(relicData.relicData).filter((type) => present.includes(type));
		return [...ordered, ...present.filter((type) => !ordered.includes(type))];
	});

	const relicTypeStats = $derived(
		Object.fromEntries(Object.entries(relicTypeTotals).map(([type, total]) => [type, { total }]))
	);

	const relicCount = $derived(
		Object.values(relicTypeTotals).reduce((acc, total) => acc + total, 0)
	);
	const isRelicTypeVisible = (type: string) => options.relicTypes?.[type] !== false;

	const areaSpawnPartition = $derived.by(() => {
		const partition = partitionSpawns(bosses.points);
		const inArea = (p: { x: number; y: number }) => mapOf(p.x, p.y) === activeArea;
		return {
			alpha: partition.alpha.filter(inArea),
			boss: partition.boss.filter(inArea),
			predator: partition.predator.filter(inArea)
		};
	});
	const dungeonCount = $derived(
		Object.values(dungeons.points).filter((p) => mapOf(p.x, p.y) === activeArea).length
	);
	const alphaPalCount = $derived(areaSpawnPartition.alpha.length);
	const predatorPalCount = $derived(areaSpawnPartition.predator.length);
	const bossCount = $derived(areaSpawnPartition.boss.length);

	const anubisImg = $derived(assetLoader.loadMenuImage('anubis'));
	const starryonImg = $derived(assetLoader.loadMenuImage('nightbluehorse'));

	function handleToggleAll(show: boolean) {
		const skip = ['enable3d', 'panelOpen', 'structureRenderMode'];
		for (const key in options) {
			if (skip.includes(key)) continue;
			const value = (options as Record<string, unknown>)[key];
			if (typeof value === 'boolean') {
				(options as Record<string, unknown>)[key] = show;
			}
		}
		options.relicTypes = Object.fromEntries(relicTypeList.map((type) => [type, show]));
	}
</script>

<div class="relative h-full overflow-hidden">
	{#if panelOpen}
		<aside
			class="bg-surface-900/95 absolute top-2 bottom-2 left-2 z-10 flex w-[420px] flex-col gap-4 overflow-y-auto rounded-lg p-4 shadow-lg"
			transition:fly={{ x: -(PANEL_W + 16), duration: 300, easing: cubicOut }}
		>
			<div class="flex flex-col gap-2">
				<SectionHeader text={m.map_options()} />

				<div class="border-b-surface-800 grid grid-cols-2 border-b-2 pb-2">
					<button class="flex items-center space-x-2" onclick={() => handleToggleAll(true)}>
						<Eye class="mr-2 h-4 w-4" />
						<span class="text-sm">Show All</span>
					</button>
					<button class="flex items-center space-x-2" onclick={() => handleToggleAll(false)}>
						<EyeOff class="mr-2 h-4 w-4" />
						<span class="text-sm">Hide All</span>
					</button>
				</div>

				<div class="grid grid-cols-2 gap-2">
					<button
						class="flex items-center space-x-2 {options.showOrigin ? '' : 'opacity-25'}"
						onclick={() => (options.showOrigin = !options.showOrigin)}
					>
						<Target class="mr-2 h-6 w-6" />
						<span>{m.origin()}</span>
					</button>
					<button
						class="flex items-center space-x-2 {(options.showFastTravel ?? true)
							? ''
							: 'opacity-25'}"
						onclick={() => (options.showFastTravel = !(options.showFastTravel ?? true))}
					>
						<img src={mapImg.fastTravel} alt={m.fast_travel()} class="mr-2 h-6 w-6" />
						<span>{m.fast_travel()}</span>
						<span class="text-surface-500 text-xs">{fastTravelCount}</span>
					</button>
					<button
						class="flex items-center space-x-2 {(options.showWatchtower ?? true)
							? ''
							: 'opacity-25'}"
						onclick={() => (options.showWatchtower = !(options.showWatchtower ?? true))}
					>
						<img src={mapImg.watchTower} alt={m.watchtower()} class="mr-2 h-6 w-6" />
						<span>{m.watchtower()}</span>
						<span class="text-surface-500 text-xs">{watchtowerCount}</span>
					</button>
					<button
						class="flex items-center space-x-2 {(options.showRelics ?? true) ? '' : 'opacity-25'}"
						onclick={() => (options.showRelics = !(options.showRelics ?? true))}
					>
						<img src={mapImg.effigy} alt={m.relics()} class="mr-2 h-6 w-6" />
						<span>{m.relics()}</span>
						<span class="text-surface-500 text-xs">{relicCount}</span>
					</button>
					<button
						class="flex items-center space-x-2 {(options.showDungeons ?? true) ? '' : 'opacity-25'}"
						onclick={() => (options.showDungeons = !(options.showDungeons ?? true))}
					>
						<img src={mapImg.dungeon} alt={m.dungeons()} class="mr-2 h-6 w-6" />
						<span>{m.dungeons()}</span>
						<span class="text-surface-500 text-xs">{dungeonCount}</span>
					</button>
					<button
						class="flex items-center space-x-2 {(options.showBosses ?? true) ? '' : 'opacity-25'}"
						onclick={() => (options.showBosses = !(options.showBosses ?? true))}
					>
						<img src={mapImg.boss} alt={m.bosses()} class="mr-2 h-6 w-6" />
						<span>{m.bosses()}</span>
						<span class="text-surface-500 text-xs">{bossCount}</span>
					</button>
					<button
						class="flex items-center space-x-2 {(options.showAlphaPals ?? true)
							? ''
							: 'opacity-25'}"
						onclick={() => (options.showAlphaPals = !(options.showAlphaPals ?? true))}
					>
						<img src={anubisImg} alt={m.alpha_pal(p.pals)} class="mr-2 h-6 w-6" />
						<span>{m.alpha_pal(p.pals)}</span>
						<span class="text-surface-500 text-xs">{alphaPalCount}</span>
					</button>
					<button
						class="flex items-center space-x-2 {(options.showPredatorPals ?? true)
							? ''
							: 'opacity-25'}"
						onclick={() => (options.showPredatorPals = !(options.showPredatorPals ?? true))}
					>
						<img src={starryonImg} alt={m.predator_pals(p.pals)} class="mr-2 h-6 w-6" />
						<span>{m.predator_pals(p.pals)}</span>
						<span class="text-surface-500 text-xs">{predatorPalCount}</span>
					</button>
					<button
						class="flex items-center space-x-2 {(options.showLabels ?? true) ? '' : 'opacity-25'}"
						onclick={() => (options.showLabels = !(options.showLabels ?? true))}
					>
						<img src={mapImg.fastTravel} alt={m.map_labels()} class="mr-2 h-6 w-6" />
						<span>{m.map_labels()}</span>
					</button>
				</div>
			</div>

			<MapHints />
		</aside>
	{/if}

	<div
		class="absolute top-2 z-20 flex flex-col items-start gap-2 transition-[left] duration-300 ease-out"
		style:left="{panelOpen ? PANEL_W + 16 : 8}px"
	>
		<button
			type="button"
			class="bg-surface-900/95 hover:bg-surface-800 rounded-lg p-2 shadow-lg"
			title={m.map_options()}
			aria-label={m.map_options()}
			aria-expanded={panelOpen}
			onclick={() => (options.panelOpen = !panelOpen)}
		>
			{#if panelOpen}
				<PanelLeftClose class="h-5 w-5" />
			{:else}
				<PanelLeft class="h-5 w-5" />
			{/if}
		</button>

		{#if (options.showRelics ?? true) && relicTypeList.length > 0}
			<RelicFilterControl
				types={relicTypeList}
				stats={relicTypeStats}
				enabled={options.relicTypes ?? {}}
				ontoggle={(relicType) =>
					(options.relicTypes = {
						...(options.relicTypes ?? {}),
						[relicType]: !isRelicTypeVisible(relicType)
					})}
			/>
		{/if}
	</div>

	<div class="absolute inset-0">
		{#if MapComponent}
			<MapComponent
				bind:map
				area={activeArea}
				onAreaChange={(next: MapArea) => (options.area = next)}
				showOrigin={options.showOrigin ?? false}
				showPlayers={false}
				showBases={false}
				showFastTravel={options.showFastTravel ?? true}
				showWatchtower={options.showWatchtower ?? true}
				showRelics={options.showRelics ?? true}
				relicTypes={options.relicTypes ?? {}}
				showDungeons={options.showDungeons ?? true}
				showBosses={options.showBosses ?? true}
				showAlphaPals={options.showAlphaPals ?? true}
				showPredatorPals={options.showPredatorPals ?? true}
				showLabels={options.showLabels ?? true}
				show3d={options.enable3d ?? false}
				showStructureControls={false}
				areaSwitchAlign="right"
				renderMode={options.structureRenderMode ?? 'detailed'}
				palSize={options.palSize ?? PAL_SCALE_DEFAULT}
				palAutoFollow={options.palAutoFollow ?? true}
				palHeight={options.palHeight ?? 0}
				{mapOpacity}
				fastTravelSize={options.fastTravelSize ?? MAP_OBJECT_SCALE_DEFAULT}
				watchtowerSize={options.watchtowerSize ?? MAP_OBJECT_SCALE_DEFAULT}
				relicSize={options.relicSize ?? MAP_OBJECT_SCALE_DEFAULT}
				onToggle3d={() => (options.enable3d = !(options.enable3d ?? false))}
				onToggleRenderMode={() =>
					(options.structureRenderMode =
						(options.structureRenderMode ?? 'detailed') === 'detailed' ? 'flat' : 'detailed')}
				onTogglePalAutoFollow={() => (options.palAutoFollow = !(options.palAutoFollow ?? true))}
				onPalSizeChange={(scale: number) => (options.palSize = scale)}
				onFastTravelSizeChange={(scale: number) => (options.fastTravelSize = scale)}
				onWatchtowerSizeChange={(scale: number) => (options.watchtowerSize = scale)}
				onRelicSizeChange={(scale: number) => (options.relicSize = scale)}
				onPalHeightChange={(height: number) => (options.palHeight = height)}
				onMapOpacityChange={(opacity: number) => (options.mapOpacity = opacity)}
			/>
		{:else}
			<Loading label={m.initializing_entity({ entity: m.map() })} />
		{/if}
	</div>
</div>
