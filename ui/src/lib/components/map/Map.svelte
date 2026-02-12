<script lang="ts">
	import { View, Map, Layer, Feature, Overlay } from 'svelte-openlayers';
	import { createIconStyle, createStyle } from 'svelte-openlayers/utils';
	import { Projection } from 'ol/proj.js';
	import type { Map as OLMap, MapBrowserEvent } from 'ol';
	import { getAppState } from '$states';
	import {
		pixelToWorld,
		pixelToGameCoords,
		MAP_SIZE,
		mapToWorld,
		ORIGIN_GAME_X,
		ORIGIN_GAME_Y,
		worldToPixel,
		SCALE,
		TRANSFORM_A
	} from './utils';
	import { createPalIconStyle, mapImg } from './styles';
	import { mapObjects } from '$lib/data';
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
	import DungeonHover from './DungeonHover.svelte';
	import DungeonPopup from './DungeonPopup.svelte';
	import PalHover from './PalHover.svelte';
	import PalPopup from './PalPopup.svelte';
	import compass from '$lib/assets/img/compass.webp';
	import { onMount } from 'svelte';
	import ContextMenu from 'ol-contextmenu';
	import { Fill, Stroke, Style } from 'ol/style';
	import CircleStyle from 'ol/style/Circle';
	import type { FeatureLike } from 'ol/Feature';
	import type { Base } from '$types';

	// Props to control which markers to display
	let {
		map = $bindable(),
		showOrigin = false,
		showPlayers = true,
		showBases = true,
		showFastTravel = true,
		showDungeons = true,
		showAlphaPals = true,
		showPredatorPals = true,
		onEditBase
	}: {
		map?: OLMap | null;
		showOrigin?: boolean;
		showPlayers?: boolean;
		showBases?: boolean;
		showFastTravel?: boolean;
		showDungeons?: boolean;
		showAlphaPals?: boolean;
		showPredatorPals?: boolean;
		onEditBase?: (base: any) => void;
	} = $props();

	const appState = getAppState();

	// Map extent and projection setup
	const extent: [number, number, number, number] = [0, 0, MAP_SIZE, MAP_SIZE];
	const projection = new Projection({
		code: 'palworld-map',
		units: 'pixels',
		extent
	});
	const offset = [20, 0] as [number, number];
	const positioning = 'center-left';
	const hoverClass = 'bg-transparent! p-8';

	const defaultCenter = () => {
		const worldCoords = mapToWorld(ORIGIN_GAME_X, ORIGIN_GAME_Y);
		return worldToPixel(worldCoords.x, worldCoords.y);
	};

	// Icon styles
	const playerIconStyle = createIconStyle({
		src: mapImg.player,
		scale: 1,
		anchor: [0.5, 0.5],
		anchorXUnits: 'fraction',
		anchorYUnits: 'fraction'
	});

	const baseIconStyle = (feature: FeatureLike, resolution: number) => {
		const props = feature.getProperties();
		const base = props.data as Base;
		const areaRange = base.area_range || 3500;
		const mapPixelRadius = (areaRange / SCALE) * Math.abs(TRANSFORM_A);
		const screenRadius = mapPixelRadius / resolution;
		return [
			createIconStyle({
				src: mapImg.baseCamp,
				scale: 0.83,
				anchor: [0.5, 0.5],
				anchorXUnits: 'fraction',
				anchorYUnits: 'fraction'
			}),
			new Style({
				image: new CircleStyle({
					radius: screenRadius,
					stroke: new Stroke({ color: 'rgba(0, 0, 255, 1)', width: 2, lineDash: [4, 8] }),
					fill: new Fill({ color: 'rgba(0, 0, 255, 0.1)' })
				})
			})
		];
	};

	const fastTravelIconStyle = createIconStyle({
		src: mapImg.fastTravel,
		scale: 1,
		anchor: [0.5, 0.5],
		anchorXUnits: 'fraction',
		anchorYUnits: 'fraction'
	});

	const dungeonIconStyle = createIconStyle({
		src: mapImg.dungeon,
		scale: 1,
		anchor: [0.5, 0.5],
		anchorXUnits: 'fraction',
		anchorYUnits: 'fraction'
	});

	const originIconStyle = createStyle({
		image: {
			src: compass,
			scale: 1,
			anchor: [0.5, 0.5],
			anchorXUnits: 'fraction',
			anchorYUnits: 'fraction'
		}
	});
	const originLineStyle = createStyle({
		stroke: { color: '#ffffff', width: 0.5, lineDash: [4, 8] }
	});
	const originPixelCoords = $derived.by(() => {
		const worldCoords = mapToWorld(ORIGIN_GAME_X, ORIGIN_GAME_Y);
		return worldToPixel(worldCoords.x, worldCoords.y);
	});

	// Derived data
	const players = $derived(Object.values(appState.players || {}));
	const bases = $derived.by(() => {
		const guilds = Object.values(appState.guilds || {});
		return guilds.reduce((acc, guild) => {
			if (guild.bases) {
				Object.values(guild.bases).forEach((base) => {
					acc.push({ base, guildName: guild.name });
				});
			}
			return acc;
		}, [] as any[]);
	});

	const fastTravelPoints = $derived.by(() => {
		if (!mapObjects) return [];
		return mapObjects.points.filter((p) => p.type === 'fast_travel');
	});

	const dungeonPoints = $derived.by(() => {
		if (!mapObjects) return [];
		return mapObjects.points.filter((p) => p.type === 'dungeon');
	});

	const alphaPalPoints = $derived.by(() => {
		if (!mapObjects) return [];
		return mapObjects.points.filter((p) => p.type === 'alpha_pal');
	});

	const predatorPalPoints = $derived.by(() => {
		if (!mapObjects) return [];
		return mapObjects.points.filter((p) => p.type === 'predator_pal');
	});

	// Origin coordinates
	const originCoords = $derived.by(() => {
		const worldCoords = mapToWorld(ORIGIN_GAME_X, ORIGIN_GAME_Y);
		return worldToPixel(worldCoords.x, worldCoords.y);
	});

	// Coordinate display state
	let coordDisplayElement: HTMLDivElement | null = $state(null);
	let coordDisplayText = $state('Coordinates: 0, 0');

	function handlePointerMove(evt: MapBrowserEvent<PointerEvent | KeyboardEvent | WheelEvent>) {
		const [pixelX, pixelY] = evt.coordinate;
		const { worldX, worldY } = pixelToWorld(pixelX, pixelY);
		const { gameX, gameY } = pixelToGameCoords(pixelX, pixelY);
		coordDisplayText = `World: ${Math.round(worldX)}, ${Math.round(worldY)}<br>Map: ${gameX}, ${gameY}`;
	}

	function handleMapClick(evt: MapBrowserEvent<PointerEvent | KeyboardEvent | WheelEvent>) {
		const [pixelX, pixelY] = evt.coordinate;
		const { worldX, worldY } = pixelToWorld(pixelX, pixelY);
		const { gameX, gameY } = pixelToGameCoords(pixelX, pixelY);
		const zoom = map?.getView().getZoom();

		console.log(`Zoom level: ${zoom}`);
		console.log(`Pixel coords: [${pixelX.toFixed(2)}, ${pixelY.toFixed(2)}]`);
		console.log(`World coords: [${worldX.toFixed(2)}, ${worldY.toFixed(2)}]`);
		console.log(`Game Map coords: [${gameX}, ${gameY}]`);
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
			<!-- World map background -->
			<Layer.Static url={mapImg.worldMap} {extent} />

			<!-- Origin marker layer -->
			{#if showOrigin}
				<Layer.Vector>
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
				<Layer.Vector>
					{#each players as player}
						{#if player.location}
							<Feature.Point
								coordinates={worldToPixel(player.location.x, player.location.y)}
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
				<Layer.Vector>
					{#each bases as { base, guildName }}
						<Feature.Point
							coordinates={worldToPixel(base.location.x, base.location.y)}
							style={baseIconStyle}
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

			<!-- Fast travel markers layer -->
			{#if showFastTravel}
				<Layer.Vector>
					{#each fastTravelPoints as point}
						<Feature.Point
							coordinates={worldToPixel(point.x, point.y)}
							style={fastTravelIconStyle}
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

			<!-- Dungeon markers layer -->
			{#if showDungeons}
				<Layer.Vector>
					{#each dungeonPoints as point}
						<Feature.Point
							coordinates={worldToPixel(point.x, point.y)}
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

			<!-- Alpha Pal markers layer -->
			{#if showAlphaPals}
				<Layer.Vector>
					{#each alphaPalPoints as point}
						{@const palImage = assetLoader.loadMenuImage(point.pal)}
						{@const palStyle = createPalIconStyle(palImage, '#ffffff', map)}
						<Feature.Point
							coordinates={worldToPixel(point.x, point.y)}
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
				<Layer.Vector>
					{#each predatorPalPoints as point}
						{@const palImage = assetLoader.loadMenuImage(point.pal)}
						{@const palStyle = createPalIconStyle(palImage, '#ef4444', map)}
						<Feature.Point
							coordinates={worldToPixel(point.x, point.y)}
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
		background-color: var(--color-surface-900) !important;
		color: white !important;
		border-radius: 4px;
	}

	:global(.click-popup) {
		z-index: 100;
	}

	.coordinate-display {
		position: absolute;
		bottom: 8px;
		right: 8px;
		background-color: rgba(0, 0, 0, 0.7);
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
</style>
