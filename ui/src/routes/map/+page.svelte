<script lang="ts">
	import { getAppState } from '$states';
	import { goto } from '$app/navigation';
	import {
		worldToPixel,
		mapOf,
		DEFAULT_MAP_AREA,
		type MapArea
	} from '$components/map/utils';
	import { pixelToLngLat } from '$components/map/mercator';
	import { isWatchtower } from '$components/map/fastTravel';
	import { mapImg, relicTypeIcon } from '$components/map/styles';
	import { Loading, SectionHeader } from '$components/ui';
	import { mapObjects, fastTravelPoints, relics, relicData, bosses } from '$lib/data';
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
		panelOpen: true
	});

	const options = $derived(optionsState.current);
	const activeArea = $derived(options.area ?? DEFAULT_MAP_AREA);
	const panelOpen = $derived(options.panelOpen ?? true);

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

	const relicCount = $derived(
		Object.values(relicTypeTotals).reduce((acc, total) => acc + total, 0)
	);
	const isRelicTypeVisible = (type: string) => options.relicTypes?.[type] !== false;

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
						class="flex items-center space-x-2 {(options.showDungeons ?? true)
							? ''
							: 'opacity-25'}"
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

				{#if (options.showRelics ?? true) && relicTypeList.length > 0}
					<div class="border-surface-700 grid grid-cols-2 gap-2 rounded-sm border p-2">
						{#each relicTypeList as relicType (relicType)}
							<button
								class="flex items-center space-x-2 {isRelicTypeVisible(relicType)
									? ''
									: 'opacity-25'}"
								onclick={() =>
									(options.relicTypes = {
										...(options.relicTypes ?? {}),
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
								<span class="text-surface-500 text-xs">{relicTypeTotals[relicType]}</span>
							</button>
						{/each}
					</div>
				{/if}
			</div>
		</aside>
	{/if}

	<button
		type="button"
		class="bg-surface-900/95 hover:bg-surface-800 absolute top-2 z-20 rounded-lg p-2 shadow-lg transition-[left] duration-300 ease-out"
		style:left="{panelOpen ? PANEL_W + 16 : 8}px"
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
				onToggle3d={() => (options.enable3d = !(options.enable3d ?? false))}
				onToggleRenderMode={() =>
					(options.structureRenderMode =
						(options.structureRenderMode ?? 'detailed') === 'detailed' ? 'flat' : 'detailed')}
			/>
		{:else}
			<Loading label={m.initializing_entity({ entity: m.map() })} />
		{/if}
	</div>
</div>
