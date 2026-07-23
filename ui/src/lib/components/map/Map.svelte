<script lang="ts">
	import { onMount } from 'svelte';
	import type maplibregl from 'maplibre-gl';
	import { Map as MLMap, Source, Layer, Control, ImageLoader } from '$components/maplibre';
	import { getAppState } from '$states';
	import {
		mapOf,
		mapToWorld,
		pixelToGameCoords,
		pixelToWorld,
		worldToPixel,
		DEFAULT_MAP_AREA,
		MAP_AREA_ORDER,
		type MapArea
	} from './utils';
	import { MERCATOR_LAT_LIMIT, lngLatToPixel, pixelToLngLat } from './mercator';
	import {
		buildBaseFC,
		buildBaseRadiusFC,
		buildBossFC,
		buildFastTravelFC,
		buildMapObjectFC,
		buildOriginCrosshairFC,
		buildOriginFC,
		buildPlayerFC,
		buildRelicFC,
		emptyFC,
		type MapFeatureType
	} from './features';
	import { PAL_BORDER_ALPHA, PAL_BORDER_PREDATOR, renderPalIcon, staticIconUrls } from './icons';
	import { palIconId } from './iconIds';
	import { relicsByType } from './relics';
	import { isWatchtower } from './fastTravel';
	import { mapImg } from './styles';
	import { mapObjects, fastTravelPoints, relics, relicData, bosses } from '$lib/data';
	import { assetLoader } from '$utils';
	import MapTooltip from './MapTooltip.svelte';
	import MapPopup from './MapPopup.svelte';
	import type { MapUnlockPoint, RelicPoint } from '$types';
	import * as m from '$i18n/messages';
	import 'maplibre-gl/dist/maplibre-gl.css';

	let {
		map = $bindable(),
		area = DEFAULT_MAP_AREA,
		onAreaChange,
		showOrigin = false,
		showPlayers = true,
		showBases = true,
		showFastTravel = true,
		showWatchtower = true,
		showRelics = true,
		relicTypes = {},
		showDungeons = true,
		showBosses = true,
		showAlphaPals = true,
		showPredatorPals = true,
		onEditBase,
		onToggleFastTravel,
		onToggleRelic,
		onUnlockAllFastTravel,
		onUnlockAllWatchtowers,
		onCollectAllRelics
	}: {
		map?: maplibregl.Map;
		area?: MapArea;
		onAreaChange?: (area: MapArea) => void;
		showOrigin?: boolean;
		showPlayers?: boolean;
		showBases?: boolean;
		showFastTravel?: boolean;
		showWatchtower?: boolean;
		showRelics?: boolean;
		/** Per-relic-type visibility; a missing key means visible. */
		relicTypes?: Record<string, boolean>;
		showDungeons?: boolean;
		showBosses?: boolean;
		showAlphaPals?: boolean;
		showPredatorPals?: boolean;
		onEditBase?: (base: any) => void;
		onToggleFastTravel?: (point: MapUnlockPoint) => void;
		onToggleRelic?: (point: RelicPoint) => void;
		onUnlockAllFastTravel?: () => void;
		onUnlockAllWatchtowers?: () => void;
		onCollectAllRelics?: () => void;
	} = $props();

	const appState = getAppState();

	const MAP_TILE_DIR: Record<MapArea, string> = { MainMap: 'mainmap', Tree: 'tree' };

	const EMPTY_STYLE: maplibregl.StyleSpecification = {
		version: 8,
		sources: {},
		layers: [{ id: 'background', type: 'background', paint: { 'background-color': '#000000' } }]
	};

	const players = $derived(
		Object.values(appState.players || {}).filter(
			(player) => player.location && mapOf(player.location.x, player.location.y) === area
		)
	);
	const bases = $derived.by(() => {
		const guilds = Object.values(appState.guilds || {});
		return guilds.reduce((acc, guild) => {
			if (guild.bases) {
				Object.values(guild.bases).forEach((base) => {
					if (base.location && mapOf(base.location.x, base.location.y) === area) {
						acc.push({ base, guildName: guild.name });
					}
				});
			}
			return acc;
		}, [] as any[]);
	});

	const selectedPlayer = $derived(appState.selectedPlayer);

	// `unlocked` is deliberately tri-state: undefined when no player is selected, so
	// downstream `=== false` checks leave those pins at full opacity rather than locked.
	const fastTravelPointList = $derived.by(() => {
		const unlocked = new Set(
			(selectedPlayer?.unlocked_fast_travel_points ?? []).map((guid) => guid.toUpperCase())
		);
		return Object.entries(fastTravelPoints.points)
			.map(([guid, point]) => ({
				guid,
				x: point.x,
				y: point.y,
				class: point.class,
				localized_name: point.localized_name ?? point.id,
				unlocked: selectedPlayer ? unlocked.has(guid.toUpperCase()) : undefined
			}))
			.filter((p) => mapOf(p.x, p.y) === area);
	});

	// One layer, class-aware visibility: regular points follow showFastTravel,
	// watchtowers follow showWatchtower. Both keep type: 'fast_travel' so the
	// click/toggle path is shared.
	const visibleFastTravelPoints = $derived(
		fastTravelPointList.filter((p) => (isWatchtower(p) ? showWatchtower : showFastTravel))
	);

	const collectedRelicGuids = $derived.by(() => {
		const byType: Record<string, Set<string>> = {};
		for (const [type, guids] of Object.entries(selectedPlayer ? relicsByType(selectedPlayer) : {})) {
			byType[type] = new Set(guids.map((guid) => guid.toUpperCase()));
		}
		return byType;
	});

	const relicPointList: RelicPoint[] = $derived.by(() => {
		return Object.entries(relics.points)
			.filter(([, point]) => relicTypes[point.relic_type] !== false)
			.map(([guid, point]) => ({
				guid,
				x: point.x,
				y: point.y,
				relic_type: point.relic_type,
				localized_name: relicData.relicData[point.relic_type]?.localized_name ?? point.relic_type,
				unlocked: selectedPlayer
					? (collectedRelicGuids[point.relic_type]?.has(guid.toUpperCase()) ?? false)
					: undefined
			}))
			.filter((p) => mapOf(p.x, p.y) === area);
	});

	const dungeonPoints = $derived(
		mapObjects.points.filter((p) => p.type === 'dungeon').filter((p) => mapOf(p.x, p.y) === area)
	);
	const alphaPalPoints = $derived(
		mapObjects.points.filter((p) => p.type === 'alpha_pal').filter((p) => mapOf(p.x, p.y) === area)
	);
	const predatorPalPoints = $derived(
		mapObjects.points
			.filter((p) => p.type === 'predator_pal')
			.filter((p) => mapOf(p.x, p.y) === area)
	);

	const bossPoints = $derived.by(() => {
		const defeated = new Set(selectedPlayer?.defeated_bosses ?? []);
		return Object.entries(bosses.points)
			.map(([rowKey, boss]) => ({ ...boss, rowKey, defeated: defeated.has(boss.spawner_id) }))
			.filter((boss) => mapOf(boss.x, boss.y) === area);
	});

	const originFC = $derived(showOrigin && area === 'MainMap' ? buildOriginFC(area) : emptyFC());
	const originLinesFC = $derived(buildOriginCrosshairFC(area));
	const playerFC = $derived(buildPlayerFC(players as never, area));
	const baseFC = $derived(buildBaseFC(bases, area));
	const baseRadiusFC = $derived(buildBaseRadiusFC(bases, area));
	const fastTravelFC = $derived(buildFastTravelFC(visibleFastTravelPoints as never, area));
	const relicFC = $derived(buildRelicFC(relicPointList, area));
	const dungeonFC = $derived(buildMapObjectFC(dungeonPoints, 'dungeon', area));
	const alphaFC = $derived(buildMapObjectFC(alphaPalPoints, 'alpha_pal', area));
	const predatorFC = $derived(buildMapObjectFC(predatorPalPoints, 'predator_pal', area));
	const bossFC = $derived(buildBossFC(bossPoints as never, area));

	const byKey = $derived.by(() => {
		const table = new Map<string, { data: any; guildName?: string }>();
		for (const p of players) table.set(`player:${p.uid}`, { data: p });
		for (const { base, guildName } of bases) table.set(`base:${base.id}`, { data: base, guildName });
		for (const p of visibleFastTravelPoints) table.set(`fast_travel:${p.guid}`, { data: p });
		for (const p of relicPointList) table.set(`relic:${p.guid}`, { data: p });
		for (const p of dungeonPoints) table.set(`dungeon:dungeon:${p.x}:${p.y}`, { data: p });
		for (const p of alphaPalPoints) table.set(`alpha_pal:alpha_pal:${p.x}:${p.y}`, { data: p });
		for (const p of predatorPalPoints)
			table.set(`predator_pal:predator_pal:${p.x}:${p.y}`, { data: p });
		for (const b of bossPoints) table.set(`boss:${b.rowKey}`, { data: b });
		table.set('origin:origin', { data: null });
		return table;
	});

	function lookup(type: string, key: string) {
		return byKey.get(`${type}:${key}`);
	}

	const staticIcons = staticIconUrls();

	const palIcons = $derived.by(() => {
		const wanted = new Map<string, { url: string; border: string }>();
		for (const p of alphaPalPoints) {
			wanted.set(palIconId(p.pal, false), {
				url: assetLoader.loadMenuImage(p.pal),
				border: PAL_BORDER_ALPHA
			});
		}
		for (const p of predatorPalPoints) {
			wanted.set(palIconId(p.pal, true), {
				url: assetLoader.loadMenuImage(p.pal),
				border: PAL_BORDER_PREDATOR
			});
		}
		return wanted;
	});

	const registeredPalIcons = new Set<string>();

	$effect(() => {
		const instance = map;
		if (!instance) return;
		for (const [id, { url, border }] of palIcons) {
			if (registeredPalIcons.has(id)) continue;
			registeredPalIcons.add(id);
			renderPalIcon(url, border)
				.then((image) => {
					if (!instance.hasImage(id)) instance.addImage(id, image);
				})
				.catch(() => registeredPalIcons.delete(id));
		}
	});

	let coordDisplayText = $state('World: 0, 0<br>Map: 0, 0');
	let hovered = $state<{ type: MapFeatureType; key: string } | null>(null);
	let selected = $state<{ type: MapFeatureType; key: string; lngLat: maplibregl.LngLat } | null>(
		null
	);

	const INTERACTIVE_LAYERS = [
		'origin-icons',
		'player-icons',
		'base-icons',
		'fast-travel-icons',
		'relic-icons',
		'dungeon-icons',
		'boss-icons',
		'alpha-icons',
		'predator-icons'
	];

	function topFeatureAt(ev: maplibregl.MapMouseEvent, layerIds: string[]) {
		const instance = map;
		if (!instance) return null;
		const layers = layerIds.filter((id) => instance.getLayer(id));
		if (layers.length === 0) return null;
		return instance.queryRenderedFeatures(ev.point, { layers })[0] ?? null;
	}

	function handleMouseMove(ev: maplibregl.MapMouseEvent) {
		const [px, py] = lngLatToPixel(ev.lngLat.lng, ev.lngLat.lat);
		const { worldX, worldY } = pixelToWorld(px, py, area);
		const { gameX, gameY } = pixelToGameCoords(px, py, area);
		coordDisplayText = `World: ${Math.round(worldX)}, ${Math.round(worldY)}<br>Map: ${gameX}, ${gameY}`;

		const top = topFeatureAt(ev, INTERACTIVE_LAYERS);
		hovered = top
			? { type: top.properties.type as MapFeatureType, key: String(top.properties.key) }
			: null;
		const canvas = map?.getCanvas();
		if (canvas) canvas.style.cursor = top ? 'pointer' : '';
	}

	function handleClick(ev: maplibregl.MapMouseEvent) {
		const top = topFeatureAt(ev, INTERACTIVE_LAYERS);
		if (!top) {
			selected = null;
			return;
		}
		const type = top.properties.type as MapFeatureType;
		const key = String(top.properties.key);

		if (selectedPlayer) {
			if (type === 'fast_travel') {
				onToggleFastTravel?.(lookup(type, key)?.data as MapUnlockPoint);
				return;
			}
			if (type === 'relic') {
				onToggleRelic?.(lookup(type, key)?.data as RelicPoint);
				return;
			}
		}
		selected = { type, key, lngLat: ev.lngLat };
	}

	function handleContextMenu(ev: maplibregl.MapMouseEvent) {
		const top = topFeatureAt(ev, ['base-icons']);
		if (top) onEditBase?.(lookup('base', String(top.properties.key))?.data);
	}

	onMount(() => {
		for (const player of Object.values(appState.playerSummaries)) {
			if (!appState.players[player.uid] && player.loaded) {
				appState.selectPlayerLazy(player.uid);
			}
		}
	});

	const defaultCenter = $derived.by(() => {
		const world = mapToWorld(0, 0);
		const [px, py] = worldToPixel(world.x, world.y, area);
		return pixelToLngLat(px, py);
	});

	const MAX_BOUNDS: [[number, number], [number, number]] = [
		[-180, -MERCATOR_LAT_LIMIT],
		[180, MERCATOR_LAT_LIMIT]
	];
</script>

<div class="relative h-full w-full">
	<MLMap
		bind:map
		class="h-full w-full"
		style={EMPTY_STYLE}
		center={defaultCenter}
		zoom={2}
		minZoom={0}
		maxZoom={7}
		maxBounds={MAX_BOUNDS}
		renderWorldCopies={false}
		dragRotate={false}
		pitchWithRotate={false}
		touchZoomRotate={false}
		attributionControl={false}
		onmousemove={handleMouseMove}
		onclick={handleClick}
		oncontextmenu={handleContextMenu}
	>
		<Control.Navigation position="top-right" showCompass={false} />
		<Control.Fullscreen position="top-right" />

		<ImageLoader images={staticIcons}>
			{#each MAP_AREA_ORDER as candidate}
				<Source.Raster
					tiles={[`/maps/${MAP_TILE_DIR[candidate]}/{z}/{x}/{y}.webp`]}
					tileSize={512}
					maxzoom={4}
				>
					<Layer.Raster visible={area === candidate} paint={{ 'raster-fade-duration': 300 }} />
				</Source.Raster>
			{/each}

			<Source.GeoJSON data={originLinesFC}>
				<Layer.Line
					visible={showOrigin && area === 'MainMap'}
					paint={{ 'line-color': '#ffffff', 'line-width': 0.5, 'line-dasharray': [4, 8] }}
				/>
			</Source.GeoJSON>

			<Source.GeoJSON data={baseRadiusFC}>
				<Layer.Fill visible={showBases} paint={{ 'fill-color': '#0000ff', 'fill-opacity': 0.1 }} />
				<Layer.Line
					visible={showBases}
					paint={{ 'line-color': '#0000ff', 'line-width': 2, 'line-dasharray': [4, 8] }}
				/>
			</Source.GeoJSON>

			<Source.GeoJSON data={originFC}>
				<Layer.Symbol
					id="origin-icons"
					visible={showOrigin && area === 'MainMap'}
					layout={{
						'icon-image': ['get', 'icon'],
						'icon-allow-overlap': true,
						'icon-size': ['interpolate', ['linear'], ['zoom'], 2, 0.6, 7, 1.0]
					}}
				/>
			</Source.GeoJSON>

			<Source.GeoJSON data={baseFC}>
				<Layer.Symbol
					id="base-icons"
					visible={showBases}
					layout={{
						'icon-image': ['get', 'icon'],
						'icon-allow-overlap': true,
						'icon-size': ['interpolate', ['linear'], ['zoom'], 2, 0.5, 7, 0.83]
					}}
				/>
			</Source.GeoJSON>

			<Source.GeoJSON data={playerFC}>
				<Layer.Symbol
					id="player-icons"
					visible={showPlayers}
					layout={{
						'icon-image': ['get', 'icon'],
						'icon-allow-overlap': true,
						'icon-size': ['interpolate', ['linear'], ['zoom'], 2, 0.6, 7, 1.0]
					}}
				/>
			</Source.GeoJSON>

			<Source.GeoJSON data={fastTravelFC}>
				<Layer.Symbol
					id="fast-travel-icons"
					visible={showFastTravel || showWatchtower}
					layout={{
						'icon-image': ['get', 'icon'],
						'icon-allow-overlap': false,
						'symbol-sort-key': ['case', ['get', 'locked'], 1, 2],
						'icon-size': [
							'interpolate',
							['linear'],
							['zoom'],
							2,
							['case', ['get', 'watchtower'], 0.36, 0.45],
							7,
							['case', ['get', 'watchtower'], 0.6, 0.75]
						]
					}}
					paint={{ 'icon-opacity': ['case', ['get', 'locked'], 0.6, 1] }}
				/>
			</Source.GeoJSON>

			<Source.GeoJSON data={relicFC}>
				<Layer.Symbol
					id="relic-icons"
					visible={showRelics}
					layout={{
						'icon-image': ['get', 'icon'],
						'icon-allow-overlap': false,
						'symbol-sort-key': ['case', ['get', 'collected'], 2, 1],
						'icon-size': ['interpolate', ['linear'], ['zoom'], 2, 0.3, 7, 0.5]
					}}
					paint={{ 'icon-opacity': ['case', ['get', 'collected'], 1, 0.6] }}
				/>
			</Source.GeoJSON>

			<Source.GeoJSON data={dungeonFC}>
				<Layer.Symbol
					id="dungeon-icons"
					visible={showDungeons}
					layout={{
						'icon-image': ['get', 'icon'],
						'icon-allow-overlap': false,
						'icon-size': ['interpolate', ['linear'], ['zoom'], 2, 0.6, 7, 1.0]
					}}
				/>
			</Source.GeoJSON>

			<Source.GeoJSON data={bossFC}>
				<Layer.Symbol
					id="boss-icons"
					visible={showBosses}
					layout={{
						'icon-image': ['get', 'icon'],
						'icon-allow-overlap': false,
						'symbol-sort-key': ['case', ['get', 'defeated'], 2, 1],
						'icon-size': ['interpolate', ['linear'], ['zoom'], 2, 0.6, 7, 1.0]
					}}
					paint={{ 'icon-opacity': ['case', ['get', 'defeated'], 0.6, 1] }}
				/>
			</Source.GeoJSON>

			<Source.GeoJSON data={alphaFC}>
				<Layer.Symbol
					id="alpha-icons"
					visible={showAlphaPals}
					layout={{
						'icon-image': ['get', 'icon'],
						'icon-allow-overlap': false,
						'icon-size': ['interpolate', ['linear'], ['zoom'], 2, 0.6, 7, 1.0]
					}}
				/>
			</Source.GeoJSON>

			<Source.GeoJSON data={predatorFC}>
				<Layer.Symbol
					id="predator-icons"
					visible={showPredatorPals}
					layout={{
						'icon-image': ['get', 'icon'],
						'icon-allow-overlap': false,
						'icon-size': ['interpolate', ['linear'], ['zoom'], 2, 0.6, 7, 1.0]
					}}
				/>
			</Source.GeoJSON>
		</ImageLoader>
	</MLMap>

	{#if hovered}
		{@const entry = lookup(hovered.type, hovered.key)}
		<div class="map-hover-card">
			<MapTooltip type={hovered.type} data={entry?.data} guildName={entry?.guildName} />
		</div>
	{/if}

	{#if selected}
		{@const entry = lookup(selected.type, selected.key)}
		<div class="map-popup-card">
			<MapPopup type={selected.type} data={entry?.data} guildName={entry?.guildName} />
			<button type="button" class="map-popup-close" onclick={() => (selected = null)}>×</button>
		</div>
	{/if}

	<!-- Player bulk actions -->
	{#if selectedPlayer}
		<div class="map-actions">
			<button
				type="button"
				class="map-action-btn"
				title={m.unlock_all_fast_travel()}
				aria-label={m.unlock_all_fast_travel()}
				onclick={() => onUnlockAllFastTravel?.()}
			>
				<img src={mapImg.fastTravel} alt={m.fast_travel()} />
			</button>
			<button
				type="button"
				class="map-action-btn"
				title={m.unlock_all_watchtowers()}
				aria-label={m.unlock_all_watchtowers()}
				onclick={() => onUnlockAllWatchtowers?.()}
			>
				<img src={mapImg.watchTower} alt={m.watchtower()} />
			</button>
			<!-- Never offer a bulk write for pins the user cannot see. -->
			{#if showRelics}
				<button
					type="button"
					class="map-action-btn"
					title={m.collect_all_relics()}
					aria-label={m.collect_all_relics()}
					onclick={() => onCollectAllRelics?.()}
				>
					<img src={mapImg.effigy} alt={m.relics()} />
				</button>
			{/if}
		</div>
	{/if}

	<div class="map-area-switch">
		{#each MAP_AREA_ORDER as candidate}
			<button
				type="button"
				class="map-area-btn"
				class:active={area === candidate}
				onclick={() => onAreaChange?.(candidate)}
			>
				{candidate === 'MainMap' ? m.map_area_mainmap() : m.map_area_tree()}
			</button>
		{/each}
	</div>

	<!-- Coordinate display overlay -->
	<div class="coordinate-display">
		{@html coordDisplayText}
	</div>
</div>

<style>
	:global(.maplibregl-canvas-container) {
		background-color: #000;
	}

	.map-hover-card,
	.map-popup-card {
		position: absolute;
		top: 48px;
		left: 8px;
		z-index: 1000;
		pointer-events: none;
	}

	.map-popup-card {
		pointer-events: auto;
	}

	.map-popup-close {
		position: absolute;
		top: 4px;
		right: 4px;
		width: 20px;
		height: 20px;
		line-height: 1;
		color: white;
		cursor: pointer;
	}

	.map-actions {
		position: absolute;
		bottom: 56px;
		right: 8px;
		display: flex;
		flex-direction: column;
		gap: 8px;
		z-index: 1000;
	}

	.map-action-btn {
		display: flex;
		align-items: center;
		justify-content: center;
		width: 40px;
		height: 40px;
		padding: 6px;
		background: color-mix(in srgb, var(--color-surface-900) 85%, transparent);
		backdrop-filter: blur(8px);
		border: 1px solid color-mix(in srgb, var(--color-surface-700) 40%, transparent);
		border-radius: 4px;
		cursor: pointer;
		transition:
			background-color 0.15s ease-out,
			border-color 0.15s ease-out;
	}

	.map-action-btn:hover {
		background: color-mix(in srgb, var(--color-secondary-500) 25%, transparent);
		border-color: color-mix(in srgb, var(--color-secondary-400) 50%, transparent);
	}

	.map-action-btn img {
		width: 100%;
		height: 100%;
		object-fit: contain;
	}

	.coordinate-display {
		position: absolute;
		bottom: 8px;
		right: 8px;
		background: color-mix(in srgb, var(--color-surface-900) 85%, transparent);
		backdrop-filter: blur(8px);
		border: 1px solid color-mix(in srgb, var(--color-surface-700) 40%, transparent);
		color: white;
		padding: 5px 10px;
		border-radius: 4px;
		font-family: monospace;
		font-size: 12px;
		line-height: 1.4;
		pointer-events: none;
		z-index: 1000;
	}

	.map-area-switch {
		position: absolute;
		top: 8px;
		left: 50%;
		transform: translateX(-50%);
		display: flex;
		gap: 2px;
		padding: 2px;
		background: color-mix(in srgb, var(--color-surface-900) 85%, transparent);
		backdrop-filter: blur(8px);
		border: 1px solid color-mix(in srgb, var(--color-surface-700) 40%, transparent);
		border-radius: 4px;
		z-index: 1000;
	}

	.map-area-btn {
		padding: 4px 12px;
		border-radius: 3px;
		color: white;
		font-size: 13px;
		cursor: pointer;
		transition: background-color 0.15s ease-out;
	}

	.map-area-btn:hover {
		background: color-mix(in srgb, var(--color-secondary-500) 25%, transparent);
	}

	.map-area-btn.active {
		background: color-mix(in srgb, var(--color-secondary-500) 45%, transparent);
	}
</style>
