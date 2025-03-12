<script lang="ts">
	import { onDestroy } from 'svelte';
	import 'leaflet/dist/leaflet.css';
	import L from 'leaflet';
	import { getAppState } from '$states';
	import {
		leafletToWorld,
		MAP_SIZE,
		mapToWorld,
		ORIGIN_GAME_X,
		ORIGIN_GAME_Y,
		TRANSFORM_A,
		TRANSFORM_B,
		TRANSFORM_C,
		TRANSFORM_D,
		worldToLeaflet,
		worldToMap
	} from './utils';
	import { mapIcons, mapImg } from './mapImages';
	import { mapObjects, palsData } from '$lib/data';
	import { assetLoader } from '$utils';

	// Props to control which markers to display
	let {
		map = $bindable(),
		showOrigin = false,
		showPlayers = true,
		showBases = true,
		showFastTravel = true,
		showDungeons = true,
		showAlphaPals = true,
		showPredatorPals = true
	} = $props();

	const appState = getAppState();

	const initialView = $derived.by(() => {
		const worldCoords = mapToWorld(ORIGIN_GAME_X, ORIGIN_GAME_Y);
		const origin = worldToLeaflet(worldCoords.x, worldCoords.y);
		return [origin.lat, origin.lng] as [number, number];
	});

	// Custom CRS with corrected transformation
	const CustomCRS = L.extend({}, L.CRS.Simple, {
		transformation: new L.Transformation(TRANSFORM_A, TRANSFORM_B, TRANSFORM_C, TRANSFORM_D)
	});

	// Map bounds based on the texture size
	const bounds: L.LatLngBoundsExpression = [
		[0, 0] as L.LatLngTuple,
		[MAP_SIZE, MAP_SIZE] as L.LatLngTuple
	];

	let originMarkers: L.Layer[] = [];
	let playerMarkers: L.Marker[] = [];
	let baseMarkers: L.Marker[] = [];
	let fastTravelMarkers: L.Marker[] = [];
	let dungeonMarkers: L.Marker[] = [];
	let alphaPalMarkers: L.Marker[] = [];
	let predatorPalMarkers: L.Marker[] = [];

	const mapOptions = {
		center: [0, 0] as [number, number],
		crs: CustomCRS,
		minZoom: -4,
		maxZoom: 3,
		maxBounds: bounds,
		maxBoundsViscosity: 1
	};

	// Create icon for the origin
	function createOriginIcon(): L.Icon {
		const iconUrl = `data:image/svg+xml,%3Csvg xmlns='http://www.w3.org/2000/svg' width='24' height='24' viewBox='0 0 24 24' fill='none' stroke='%23FFFFFF' stroke-width='2'%3E%3Ccircle cx='12' cy='12' r='10'/%3E%3Cline x1='12' y1='2' x2='12' y2='22' stroke='%23FFFFFF' stroke-width='2'/%3E%3Cline x1='2' y1='12' x2='22' y2='12' stroke='%23FFFFFF' stroke-width='2'/%3E%3C/svg%3E`;

		return L.icon({
			iconUrl,
			iconSize: [24, 24],
			iconAnchor: [12, 12],
			popupAnchor: [0, -12]
		});
	}

	function addOriginMarker() {
		if (!map) return;

		// Clear any existing origin markers
		originMarkers.forEach((marker) => map!.removeLayer(marker));
		originMarkers = [];

		if (!showOrigin) return;

		// The origin in world coordinates corresponds to map coordinates (0,0)
		const worldCoords = mapToWorld(ORIGIN_GAME_X, ORIGIN_GAME_Y);

		// Convert to Leaflet coordinates
		const latlng = worldToLeaflet(worldCoords.x, worldCoords.y);

		const icon = createOriginIcon();
		const originMarker = L.marker(latlng, { icon }).addTo(map!);

		originMarker.bindPopup(`
            <div>
                <h3 class="text-lg font-bold">Origin (0,0)</h3>
                <p>This is the origin (0,0) in map coordinates</p>
                <p class="text-xs mt-2">World Coords: ${worldCoords.x.toFixed(2)}, ${worldCoords.y.toFixed(2)}</p>
                <p class="text-xs">Map Coords: ${ORIGIN_GAME_X}, ${ORIGIN_GAME_Y}</p>
                <p class="text-xs">Leaflet Coords: ${latlng.lat.toFixed(2)}, ${latlng.lng.toFixed(2)}</p>
            </div>
        `);

		originMarkers.push(originMarker);

		// Add crosshair lines for better visualization of the origin
		const horizontalLine = L.polyline(
			[
				[latlng.lat, 0],
				[latlng.lat, MAP_SIZE]
			],
			{ color: 'rgba(255, 255, 255, 0.5)', weight: 1, dashArray: '5,5' }
		).addTo(map!);

		const verticalLine = L.polyline(
			[
				[0, latlng.lng],
				[MAP_SIZE, latlng.lng]
			],
			{ color: 'rgba(255, 255, 255, 0.5)', weight: 1, dashArray: '5,5' }
		).addTo(map!);

		originMarkers.push(horizontalLine);
		originMarkers.push(verticalLine);
	}

	function addBaseMarkers() {
		if (!map) return;

		// Clear any existing base markers
		baseMarkers.forEach((marker) => map!.removeLayer(marker));
		baseMarkers = [];

		if (!showBases) return;

		// Get all bases from the app state
		const guilds = Object.values(appState.guilds || {});

		const bases = guilds.reduce((acc, guild) => {
			if (guild.bases) {
				Object.values(guild.bases).forEach((base) => {
					acc.push(base);
				});
			}
			return acc;
		}, [] as any[]);

		const icon = mapIcons.baseCamp;
		bases.forEach((base) => {
			if (!base.location) return;

			// Convert base world coordinates to Leaflet coordinates
			const latlng = worldToLeaflet(base.location.x, base.location.y);
			const baseMarker = L.marker(latlng, { icon }).addTo(map!);

			baseMarker.bindPopup(`
            <div>
                <h3 class="text-lg font-bold">${base.id}</h3>
                <p class="text-xs mt-2">World Coords: ${base.location.x.toFixed(2)}, ${base.location.y.toFixed(2)}</p>
                <p class="text-xs">Map Coords: ${worldToMap(base.location.x, base.location.y).x}, ${worldToMap(base.location.x, base.location.y).y}</p>
            </div>
        `);

			// Add the marker to the baseMarkers array so we can remove it later
			baseMarkers.push(baseMarker);
		});
	}

	function addPlayerMarkers() {
		if (!map) return;

		// Clear any existing player markers
		playerMarkers.forEach((marker) => map!.removeLayer(marker));
		playerMarkers = [];

		if (!showPlayers) return;

		// Get all players from the app state
		const players = Object.values(appState.players || {});
		const icon = mapIcons.player;

		players.forEach((player) => {
			if (!player.location) return;

			// Convert player world coordinates to Leaflet coordinates
			const latlng = worldToLeaflet(player.location.x, player.location.y);

			const playerMarker = L.marker(latlng, { icon }).addTo(map!);

			playerMarker.bindPopup(`
                <div>
                    <h3 class="text-lg font-bold">${player.nickname}</h3>
                    <p class="text-xs">Level: ${player.level}</p>
                    <p class="text-xs">HP: ${player.hp}</p>
                    <p class="text-xs mt-2">World Coords: ${player.location.x.toFixed(2)}, ${player.location.y.toFixed(2)}, ${player.location.z.toFixed(2)}</p>
                    <p class="text-xs">Map Coords: ${worldToMap(player.location.x, player.location.y).x}, ${worldToMap(player.location.x, player.location.y).y}</p>
                </div>
            `);

			playerMarkers.push(playerMarker);
		});
	}

	async function addFastTravelMarkers() {
		if (!map) return;

		// Clear any existing fast travel markers
		fastTravelMarkers.forEach((marker) => map!.removeLayer(marker));
		fastTravelMarkers = [];

		if (!showFastTravel) return;
		if (!mapObjects) return;
		const icon = mapIcons.fastTravel;
		mapObjects.points
			.filter((p) => p.type === 'fast_travel')
			.forEach((point) => {
				// Convert point world coordinates to Leaflet coordinates
				const latlng = worldToLeaflet(point.x, point.y);

				const marker = L.marker(latlng, { icon }).addTo(map!);

				marker.bindPopup(`
					<div>
						<h3 class="text-lg font-bold">${point.localized_name}</h3>
						<p class="text-xs mt-2">World Coords: ${point.x.toFixed(2)}, ${point.y.toFixed(2)}</p>
						<p class="text-xs">Map Coords: ${worldToMap(point.x, point.y).x}, ${worldToMap(point.x, point.y).y}</p>
					</div>
				`);

				fastTravelMarkers.push(marker);
			});
	}

	async function addDungeonMarkers() {
		if (!map) return;

		// Clear any existing fast travel markers
		dungeonMarkers.forEach((marker) => map!.removeLayer(marker));
		dungeonMarkers = [];

		if (!showDungeons) return;
		if (!mapObjects) return;
		const icon = mapIcons.dungeon;
		mapObjects.points
			.filter((p) => p.type === 'dungeon')
			.forEach((point) => {
				// Convert point world coordinates to Leaflet coordinates
				const latlng = worldToLeaflet(point.x, point.y);

				const marker = L.marker(latlng, { icon }).addTo(map!);

				marker.bindPopup(`
					<div>
						<p class="text-xs mt-2">World Coords: ${point.x.toFixed(2)}, ${point.y.toFixed(2)}</p>
						<p class="text-xs">Map Coords: ${worldToMap(point.x, point.y).x}, ${worldToMap(point.x, point.y).y}</p>
					</div>
				`);

				dungeonMarkers.push(marker);
			});
	}

	async function addAlphaPalMarkers() {
		if (!map) return;

		// Clear any existing alpha pal markers
		alphaPalMarkers.forEach((marker) => map!.removeLayer(marker));
		alphaPalMarkers = [];

		if (!showAlphaPals) return;
		if (!mapObjects) return;
		mapObjects.points
			.filter((p) => p.type === 'alpha_pal')
			.forEach((point) => {
				// Convert point world coordinates to Leaflet coordinates
				const latlng = worldToLeaflet(point.x, point.y);
				const palImage = assetLoader.loadMenuImage(point.pal);
				const palData = palsData.pals[point.pal];
				const icon = L.icon({
					iconUrl: palImage,
					iconSize: [40, 40],
					iconAnchor: [20, 20],
					popupAnchor: [0, -20],
					className: 'rounded-full border-2 border-white'
				});
				const marker = L.marker(latlng, { icon }).addTo(map!);

				marker.bindPopup(`
					<div>
						<h3 class="text-lg font-bold">${palData ? palData.localized_name : point.pal}</h3>
						<p class="text-xs mt-2">World Coords: ${point.x.toFixed(2)}, ${point.y.toFixed(2)}</p>
						<p class="text-xs">Map Coords: ${worldToMap(point.x, point.y).x}, ${worldToMap(point.x, point.y).y}</p>
					</div>
				`);

				alphaPalMarkers.push(marker);
			});
	}

	async function addPredatorPalMarkers() {
		if (!map) return;

		// Clear any existing predator pal markers
		predatorPalMarkers.forEach((marker) => map!.removeLayer(marker));
		predatorPalMarkers = [];

		if (!showPredatorPals) return;
		if (!mapObjects) return;
		mapObjects.points
			.filter((p) => p.type === 'predator_pal')
			.forEach((point) => {
				// Convert point world coordinates to Leaflet coordinates
				const latlng = worldToLeaflet(point.x, point.y);
				const palImage = assetLoader.loadMenuImage(point.pal);
				const palData = palsData.pals[point.pal];
				const icon = L.icon({
					iconUrl: palImage,
					iconSize: [40, 40],
					iconAnchor: [20, 20],
					popupAnchor: [0, -20],
					className: 'rounded-full border-2 border-red-500'
				});
				const marker = L.marker(latlng, { icon }).addTo(map!);

				marker.bindPopup(`
					<div>
						<h3 class="text-lg font-bold">${palData ? palData.localized_name : point.pal}</h3>
						<p class="text-xs mt-2">World Coords: ${point.x.toFixed(2)}, ${point.y.toFixed(2)}</p>
						<p class="text-xs">Map Coords: ${worldToMap(point.x, point.y).x}, ${worldToMap(point.x, point.y).y}</p>
					</div>
				`);

				predatorPalMarkers.push(marker);
			});
	}

	function initialize(container: HTMLElement) {
		map = L.map(container, mapOptions);

		// Add the world map image
		L.imageOverlay(mapImg.worldMap, bounds).addTo(map);

		// Fit to bounds and set initial view
		map.fitBounds(bounds);

		// Add click handler to log zoom level and coordinates
		map.on('click', (e: L.LeafletMouseEvent) => {
			const zoom = map?.getZoom();
			const leafletCoords = e.latlng;
			const worldCoords = leafletToWorld(leafletCoords);

			// Calculate game map coordinates from Leaflet coordinates
			const gameX = (leafletCoords.lng - TRANSFORM_B) / TRANSFORM_A;
			const gameY = (leafletCoords.lat - TRANSFORM_D) / TRANSFORM_C;

			console.log(`Zoom level: ${zoom}`);
			console.log(
				`Leaflet coords: [${leafletCoords.lat.toFixed(2)}, ${leafletCoords.lng.toFixed(2)}]`
			);
			console.log(
				`World coords: [${worldCoords.worldX.toFixed(2)}, ${worldCoords.worldY.toFixed(2)}]`
			);
			console.log(`Game Map coords: [${Math.round(gameX)}, ${Math.round(gameY)}]`);
		});

		// Log when zoom changes
		map.on('zoomend', () => {
			console.log(`New zoom level: ${map?.getZoom()}`);
		});

		// Set initial view to the center of the map
		map.setView(initialView, -3);

		// Add markers
		addOriginMarker();
		addPlayerMarkers();
		addBaseMarkers();
		addFastTravelMarkers();
		addDungeonMarkers();
		addAlphaPalMarkers();
		addPredatorPalMarkers();

		// Add coordinate display
		addCoordinateDisplay();
	}

	// Add a coordinate display in the corner that shows both map and world coordinates
	function addCoordinateDisplay() {
		if (!map) return;

		const coordControl = L.Control.extend({
			options: {
				position: 'bottomright'
			},

			onAdd: function () {
				const container = L.DomUtil.create('div', 'coordinate-display');
				container.style.backgroundColor = 'rgba(0, 0, 0, 0.7)';
				container.style.color = 'white';
				container.style.padding = '5px 10px';
				container.style.borderRadius = '4px';
				container.style.margin = '0';
				container.style.fontFamily = 'monospace';
				container.style.fontSize = '12px';
				container.style.lineHeight = '1.4';
				container.innerHTML = 'Coordinates: 0, 0';
				return container;
			}
		});

		map.addControl(new coordControl());

		// Update coordinates on mouse move
		map.on('mousemove', function (e: L.LeafletMouseEvent) {
			const display = document.querySelector('.coordinate-display');
			if (display) {
				const worldCoords = leafletToWorld(e.latlng);

				// Calculate game map coordinates from Leaflet coordinates
				const gameX = (e.latlng.lng - TRANSFORM_B) / TRANSFORM_A;
				const gameY = (e.latlng.lat - TRANSFORM_D) / TRANSFORM_C;

				display.innerHTML = `
                    World: ${Math.round(worldCoords.worldX)}, ${Math.round(worldCoords.worldY)}<br>
                    Map: ${Math.round(gameX)}, ${Math.round(gameY)}
                `;
			}
		});
	}

	$effect(() => {
		if (map) {
			addOriginMarker();
		}
	});

	$effect(() => {
		if (map) {
			addPlayerMarkers();
		}
	});

	$effect(() => {
		if (map) {
			addBaseMarkers();
		}
	});

	$effect(() => {
		if (map) {
			addFastTravelMarkers();
		}
	});

	$effect(() => {
		if (map) {
			addDungeonMarkers();
		}
	});

	$effect(() => {
		if (map) {
			addAlphaPalMarkers();
		}
	});

	$effect(() => {
		if (map) {
			addPredatorPalMarkers();
		}
	});

	onDestroy(() => {
		if (map) {
			map.remove();
			map = undefined;
		}
	});
</script>

<div class="h-full w-full" id="map" use:initialize></div>

<style>
	:global(.leaflet-container) {
		background-color: #000 !important;
	}

	:global(.marker-popup) {
		padding: 8px;
	}

	:global(.marker-popup h3) {
		margin-top: 0;
		margin-bottom: 8px;
	}

	:global(.leaflet-popup-content-wrapper) {
		background-color: var(--color-surface-900) !important;
		color: white !important;
		border-radius: 4px;
	}

	:global(.leaflet-popup-tip) {
		background-color: var(--color-surface-900) !important;
	}

	:global(.leaflet-popup-close-button) {
		color: white !important;
	}
</style>
