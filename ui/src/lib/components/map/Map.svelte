<script lang="ts">
	import { View, Map, Layer, Feature, Overlay } from 'svelte-openlayers';
	import { Projection } from 'ol/proj.js';
	import type { Map as OLMap, MapBrowserEvent } from 'ol';
	import { getAppState } from '$states';
	import {
		pixelToWorld,
		pixelToGameCoords,
		mapToWorld,
		worldToPixel,
		mapOf,
		MAP_SIZE,
		DEFAULT_MAP_AREA,
		MAP_AREA_ORDER,
		type MapArea
	} from './utils';
	import { relicsByType } from './relics';
	import {
		createPalIconStyle,
		mapImg,
		baseIconStyle,
		fastTravelStyle,
		relicStyle,
		dungeonIconStyle,
		bossStyle,
		originIconStyle,
		originLineStyle,
		playerIconStyle
	} from './styles';
	import { mapObjects, fastTravelPoints, relics, relicData, bosses } from '$lib/data';
	import { assetLoader } from '$utils';
	import 'svelte-openlayers/styles.css';
	import PlayerPopup from './PlayerPopup.svelte';
	import PlayerHover from './PlayerHover.svelte';
	import OriginHover from './OriginHover.svelte';
	import OriginPopup from './OriginPopup.svelte';
	import BaseHover from './BaseHover.svelte';
	import BasePopup from './BasePopup.svelte';
	import FastTravelHover from './FastTravelHover.svelte';
	import FastTravelPopup from './FastTravelPopup.svelte';
	import RelicHover from './RelicHover.svelte';
	import RelicPopup from './RelicPopup.svelte';
	import DungeonHover from './DungeonHover.svelte';
	import DungeonPopup from './DungeonPopup.svelte';
	import BossHover from './BossHover.svelte';
	import BossPopup from './BossPopup.svelte';
	import PalHover from './PalHover.svelte';
	import PalPopup from './PalPopup.svelte';
	import { onMount } from 'svelte';
	import ContextMenu from 'ol-contextmenu';
	import type { MapUnlockPoint, RelicPoint } from '$types';
	import * as m from '$i18n/messages';
	import { isWatchtower } from './fastTravel';

	// Props to control which markers to display
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
		map?: OLMap | null;
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

	// Map extent and projection setup
	const extent: [number, number, number, number] = [0, 0, MAP_SIZE, MAP_SIZE];
	const projection = new Projection({
		code: 'palworld-map',
		units: 'pixels',
		extent
	});
	const offset = [10, 0] as [number, number];
	const positioning = 'center-left';
	const hoverClass = 'bg-transparent! p-0 shadow-none!';

	const defaultCenter = () => {
		const worldCoords = mapToWorld(0, 0);
		return worldToPixel(worldCoords.x, worldCoords.y, area);
	};

	const originPixelCoords = $derived.by(() => {
		const worldCoords = mapToWorld(0, 0);
		return worldToPixel(worldCoords.x, worldCoords.y, area);
	});

	// Derived data
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

	const dungeonPoints = $derived.by(() => {
		if (!mapObjects) return [];
		return mapObjects.points
			.filter((p) => p.type === 'dungeon')
			.filter((p) => mapOf(p.x, p.y) === area);
	});

	const alphaPalPoints = $derived.by(() => {
		if (!mapObjects) return [];
		return mapObjects.points
			.filter((p) => p.type === 'alpha_pal')
			.filter((p) => mapOf(p.x, p.y) === area);
	});

	const bossPoints = $derived.by(() => {
		const defeated = new Set(selectedPlayer?.defeated_bosses ?? []);
		return Object.entries(bosses.points)
			.map(([rowKey, boss]) => ({ ...boss, rowKey, defeated: defeated.has(boss.spawner_id) }))
			.filter((boss) => mapOf(boss.x, boss.y) === area);
	});

	const predatorPalPoints = $derived.by(() => {
		if (!mapObjects) return [];
		return mapObjects.points
			.filter((p) => p.type === 'predator_pal')
			.filter((p) => mapOf(p.x, p.y) === area);
	});

	// Origin coordinates
	const originCoords = $derived.by(() => {
		const worldCoords = mapToWorld(0, 0);
		return worldToPixel(worldCoords.x, worldCoords.y, area);
	});

	// Overlay reveal state
	let overlaysReady = $state(false);

	// Coordinate display state
	let coordDisplayElement: HTMLDivElement | null = $state(null);
	let coordDisplayText = $state('Coordinates: 0, 0');

	function handlePointerMove(evt: MapBrowserEvent<PointerEvent | KeyboardEvent | WheelEvent>) {
		const [pixelX, pixelY] = evt.coordinate;
		const { worldX, worldY } = pixelToWorld(pixelX, pixelY, area);
		const { gameX, gameY } = pixelToGameCoords(pixelX, pixelY, area);
		coordDisplayText = `World: ${Math.round(worldX)}, ${Math.round(worldY)}<br>Map: ${gameX}, ${gameY}`;
	}

	function handleMapClick(evt: MapBrowserEvent<PointerEvent | KeyboardEvent | WheelEvent>) {
		const feature = map?.forEachFeatureAtPixel(evt.pixel, (ft) => ft);
		if (feature && selectedPlayer) {
			const featureType = feature.get('type');
			if (featureType === 'fast_travel') {
				onToggleFastTravel?.(feature.get('data') as MapUnlockPoint);
				return;
			}
			if (featureType === 'relic') {
				onToggleRelic?.(feature.get('data') as RelicPoint);
				return;
			}
		}
	}

	function getHorizontalOriginLineStrings(): number[][] {
		return [
			[0, originPixelCoords[1]],
			[MAP_SIZE, originPixelCoords[1]]
		];
	}

	function getVerticalOriginLineStrings(): number[][] {
		return [
			[originPixelCoords[0], 0],
			[originPixelCoords[0], MAP_SIZE]
		];
	}

	onMount(() => {
		for (const player of Object.values(appState.playerSummaries)) {
			if (!appState.players[player.uid] && player.loaded) {
				appState.selectPlayerLazy(player.uid);
			}
		}
		setTimeout(() => {
			if (map) {
				const baseContextMenu = new ContextMenu({
					width: 180,
					defaultItems: false,
					items: []
				});
				baseContextMenu.on('open', (evt: any) => {
					const feature = map?.forEachFeatureAtPixel(evt.pixel, (ft) => ft);
					if (feature && feature.get('type') === 'base') {
						onEditBase?.(feature.get('data'));
					}
					baseContextMenu.closeMenu();
				});
				map.addControl(baseContextMenu);
			}
		}, 1000);

		const readyTimer = setTimeout(() => {
			overlaysReady = true;
		}, 400);
		return () => clearTimeout(readyTimer);
	});
</script>

<div class="relative h-full w-full">
	<View center={defaultCenter()} zoom={3} maxZoom={8} {projection} {extent}>
		<Map
			bind:map
			class="h-full w-full"
			pointermove={handlePointerMove}
			click={handleMapClick}
			controls={{ fullscreen: true }}
		>
			{#each MAP_AREA_ORDER as candidate}
				<Layer.Static url={mapImg.maps[candidate]} {extent} visible={area === candidate} />
			{/each}

			<!-- Origin marker layer -->
			{#if showOrigin && area === 'MainMap'}
				<Layer.Vector opacity={overlaysReady ? 1 : 0}>
					<Feature.Point coordinates={originCoords} style={originIconStyle}>
						<Overlay.Hover {positioning} {offset} class={hoverClass}>
							<OriginHover />
						</Overlay.Hover>
						<Overlay.Popup {positioning} {offset}>
							<OriginPopup />
						</Overlay.Popup>
					</Feature.Point>
					<Feature.LineString
						coordinates={getHorizontalOriginLineStrings()}
						style={originLineStyle}
					/>

					<Feature.LineString
						coordinates={getVerticalOriginLineStrings()}
						style={originLineStyle}
					/>
				</Layer.Vector>
			{/if}

			<!-- Player markers layer -->
			{#if showPlayers}
				<Layer.Vector opacity={overlaysReady ? 1 : 0}>
					{#each players as player}
						{#if player.location}
							<Feature.Point
								coordinates={worldToPixel(player.location.x, player.location.y, area)}
								style={playerIconStyle}
								properties={{ type: 'player', data: player }}
							>
								<Overlay.Hover {positioning} {offset} class={hoverClass}>
									<PlayerHover {player} />
								</Overlay.Hover>
								<Overlay.Popup {positioning} {offset}>
									<PlayerPopup {player} />
								</Overlay.Popup>
							</Feature.Point>
						{/if}
					{/each}
				</Layer.Vector>
			{/if}

			<!-- Base markers layer -->
			{#if showBases}
				<Layer.Vector opacity={overlaysReady ? 1 : 0}>
					{#each bases as { base, guildName }}
						<Feature.Point
							coordinates={worldToPixel(base.location.x, base.location.y, area)}
							style={baseIconStyle(area)}
							properties={{ type: 'base', data: base }}
						>
							<Overlay.Hover {positioning} {offset} class={hoverClass}>
								<BaseHover {base} {guildName} />
							</Overlay.Hover>
							<Overlay.Popup {positioning} {offset}>
								<BasePopup {base} {guildName} />
							</Overlay.Popup>
						</Feature.Point>
					{/each}
				</Layer.Vector>
			{/if}

			<!-- Fast travel + watchtower markers layer -->
			{#if showFastTravel || showWatchtower}
				<Layer.Vector opacity={overlaysReady ? 1 : 0}>
					{#each visibleFastTravelPoints as point (point.guid)}
						<Feature.Point
							coordinates={worldToPixel(point.x, point.y, area)}
							style={fastTravelStyle}
							properties={{ type: 'fast_travel', data: point }}
						>
							<Overlay.Hover {positioning} {offset} class={hoverClass}>
								<FastTravelHover {point} />
							</Overlay.Hover>
							<Overlay.Popup {positioning} {offset}>
								<FastTravelPopup {point} />
							</Overlay.Popup>
						</Feature.Point>
					{/each}
				</Layer.Vector>
			{/if}

			<!-- Relic markers layer (all EPalRelicType, incl. Lifmunk Effigies) -->
			{#if showRelics}
				<Layer.Vector opacity={overlaysReady ? 1 : 0}>
					{#each relicPointList as point (point.guid)}
						<Feature.Point
							coordinates={worldToPixel(point.x, point.y, area)}
							style={relicStyle}
							properties={{ type: 'relic', data: point }}
						>
							<Overlay.Hover {positioning} {offset} class={hoverClass}>
								<RelicHover {point} />
							</Overlay.Hover>
							<Overlay.Popup {positioning} {offset}>
								<RelicPopup {point} />
							</Overlay.Popup>
						</Feature.Point>
					{/each}
				</Layer.Vector>
			{/if}

			<!-- Dungeon markers layer -->
			{#if showDungeons}
				<Layer.Vector opacity={overlaysReady ? 1 : 0}>
					{#each dungeonPoints as point}
						<Feature.Point
							coordinates={worldToPixel(point.x, point.y, area)}
							style={dungeonIconStyle}
							properties={{ type: 'dungeon', data: point }}
						>
							<Overlay.Hover {positioning} {offset} class={hoverClass}>
								<DungeonHover {point} />
							</Overlay.Hover>
							<Overlay.Popup {positioning} {offset}>
								<DungeonPopup {point} />
							</Overlay.Popup>
						</Feature.Point>
					{/each}
				</Layer.Vector>
			{/if}

			<!-- Boss markers layer -->
			{#if showBosses}
				<Layer.Vector opacity={overlaysReady ? 1 : 0}>
					{#each bossPoints as point (point.rowKey)}
						<Feature.Point
							coordinates={worldToPixel(point.x, point.y, area)}
							style={bossStyle}
							properties={{ type: 'boss', data: point }}
						>
							<Overlay.Hover {positioning} {offset} class={hoverClass}>
								<BossHover {point} />
							</Overlay.Hover>
							<Overlay.Popup {positioning} {offset}>
								<BossPopup {point} />
							</Overlay.Popup>
						</Feature.Point>
					{/each}
				</Layer.Vector>
			{/if}

			<!-- Alpha Pal markers layer -->
			{#if showAlphaPals}
				<Layer.Vector opacity={overlaysReady ? 1 : 0}>
					{#each alphaPalPoints as point}
						{@const palImage = assetLoader.loadMenuImage(point.pal)}
						{@const palStyle = createPalIconStyle(palImage, '#ffffff', map)}
						<Feature.Point
							coordinates={worldToPixel(point.x, point.y, area)}
							style={palStyle}
							properties={{ type: 'alpha_pal', data: point }}
						>
							<Overlay.Hover {positioning} {offset} class={hoverClass}>
								<PalHover {point} isPredator={false} />
							</Overlay.Hover>
							<Overlay.Popup {positioning} {offset}>
								<PalPopup {point} isPredator={false} />
							</Overlay.Popup>
						</Feature.Point>
					{/each}
				</Layer.Vector>
			{/if}

			<!-- Predator Pal markers layer -->
			{#if showPredatorPals}
				<Layer.Vector opacity={overlaysReady ? 1 : 0}>
					{#each predatorPalPoints as point}
						{@const palImage = assetLoader.loadMenuImage(point.pal)}
						{@const palStyle = createPalIconStyle(palImage, '#ef4444', map)}
						<Feature.Point
							coordinates={worldToPixel(point.x, point.y, area)}
							style={palStyle}
							properties={{ type: 'predator_pal', data: point }}
						>
							<Overlay.Hover {positioning} {offset} class={hoverClass}>
								<PalHover {point} isPredator={true} />
							</Overlay.Hover>
							<Overlay.Popup {positioning} {offset}>
								<PalPopup {point} isPredator={true} />
							</Overlay.Popup>
						</Feature.Point>
					{/each}
				</Layer.Vector>
			{/if}
		</Map>
	</View>

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
	<div class="coordinate-display" bind:this={coordDisplayElement}>
		{@html coordDisplayText}
	</div>
</div>

<style>
	:global(.ol-map-root) {
		background-color: #000 !important;
	}

	:global(.ol-tooltip) {
		background-color: color-mix(in srgb, var(--color-surface-900) 90%, transparent) !important;
		color: white !important;
		border-radius: 4px;
		backdrop-filter: blur(4px);
		margin: 0 0 0 12px;
		border: 1px solid color-mix(in srgb, var(--color-surface-700) 40%, transparent);
	}

	:global(.click-popup) {
		z-index: 100;
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
	:global(.ol-ctx-menu-container) {
		background-color: transparent !important;
		background: none;
		box-shadow: none !important;
		filter: none !important;
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
