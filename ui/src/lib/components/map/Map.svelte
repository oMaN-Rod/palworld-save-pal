<script module lang="ts">
	const HOVER_STATE = { hover: true };
	const STRUCTURE_MIN_ZOOM = 5;
	const MAX_PITCH_DETAILED = 80;
	const MAX_PITCH_FLAT = 85;
	const BOUNDS_PITCH_LIMIT = 85;
	const HALO_COLOR = '#f59e0b';
	const RELIC_ART_PX = 42;
	const FAST_TRAVEL_ART_PX = 52;
	const WATCHTOWER_ART_PX = 77;
</script>

<script lang="ts">
	import { onMount, untrack } from 'svelte';
	import type maplibregl from 'maplibre-gl';
	import type { FilterSpecification } from 'maplibre-gl';
	import type { ExpressionSpecification } from '@maplibre/maplibre-gl-style-spec';
	import {
		Map as MLMap,
		Source,
		Layer,
		Control,
		ImageLoader,
		FeatureState,
		Terrain
	} from '$components/maplibre';
	import { getAppState } from '$states';
	import {
		bossPalKey,
		humanizeSpawnerId,
		mapOf,
		mapToWorld,
		pixelToGameCoords,
		pixelToWorld,
		worldToPixel,
		cmPerPx,
		sceneryStreamUrl,
		DEFAULT_MAP_AREA,
		MAP_AREA_ORDER,
		MAP_TILE_DIR,
		type MapArea
	} from './utils';
	import { lngLatToPixel, pixelToLngLat, verticalScaleFactor } from './mercator';
	import { worldFittingConstrain } from './constrain';
	import { haloRadiusPx, zoomScaledIconSize, zoomScaledRadius } from './expressions';
	import { partitionSpawns } from './spawns';
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
		buildStructureFC,
		emptyFC,
		lookupFootprint,
		structureCentroid,
		type MapFeatureType,
		type StructureFC,
		type StructureFeature
	} from './features';
	import { PAL_BORDER_ALPHA, PAL_BORDER_PREDATOR, renderPalIcon, staticIconUrls } from './icons';
	import { ICON_BOUNTY, palIconId } from './iconIds';
	import { mapLayers } from '$lib/data/mapLayerStore.svelte';
	import { genericRenderLayers, getMapLayer, type MapLayerId } from './layerRegistry';
	import type { MapLayerVisibility } from './layerPanelModel';
	import { buildMapLayerFC, mapLayerIconScale } from './mapLayerFeatures';
	import { relicsByType } from './relics';
	import { isWatchtower } from './fastTravel';
	import { portalRingColorExpression, PORTAL_HEX } from './mapObjectPortal';
	import {
		STRUCTURE_TYPE_ORDER,
		materialBlend,
		materialOpacities,
		materialTints,
		structureColors
	} from './mapColors.svelte';
	import {
		dungeons,
		fastTravelPoints,
		relics,
		relicData,
		bosses,
		palsData,
		baseStructuresData,
		buildingsData
	} from '$lib/data';
	import { assetLoader } from '$utils';
	import MapTooltip from './MapTooltip.svelte';
	import MapPopup from './MapPopup.svelte';
	import BenchOverlay from './BenchOverlay.svelte';
	import Toggle3dControl from './Toggle3dControl.svelte';
	import Map3dOptionsControl from './Map3dOptionsControl.svelte';
	import { createStructureLayer, type StructureLayer } from './structureLayer';
	import { createGhostLayer, type GhostLayer } from './ghostLayer';
	import { createSceneryLayer, type SceneryLayer } from './sceneryLayer';
	import { beforeIdFor, LAYER_ORDER_3D } from './layerOrder';
	import {
		createPalLayer,
		predatorPalBosses,
		type PalLayer,
		type PalBoss,
		type PalPredator
	} from './palLayer';
	import { createMapObjectLayer, type MapObjectItem, type MapObjectLayer } from './mapObjectLayer';
	import { buildMapObjectItems, buildFastTravelRingFC, buildRelicRingFC } from './mapObjectItems';
	import { buildPalPortalFC } from './palPortalFC';
	import { PAL_SCALE_DEFAULT } from './palSize';
	import { MAP_OBJECT_SCALE_DEFAULT } from './mapObjectSize';
	import { decodeSceneryStream, type SceneryStream } from './sceneryFormat';
	import { loadTintMosaic, type TintMosaic } from './sceneryTint';
	import { composeWorld } from './ghostTransform';
	import type {
		BaseStructure,
		BlueprintStructureGeometry,
		MapUnlockPoint,
		PlacementAnchor,
		RelicPoint
	} from '$types';
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
		hideCollectedRelics = false,
		hideUnlockedFastTravel = false,
		relicTypes = {},
		showDungeons = true,
		showBosses = true,
		showAlphaPals = true,
		showPredatorPals = true,
		showBounty = false,
		mapLayerVisibility = {},
		showLabels = true,
		show3d = false,
		showStructureControls = true,
		areaSwitchAlign = 'center',
		structureTypes = {},
		renderMode = 'detailed',
		structureTextured = false,
		onToggle3d,
		onToggleStructureType,
		onToggleRenderMode,
		onToggleStructureTextured,
		onEditBase,
		onExportBase,
		onDeleteBase,
		onToggleFastTravel,
		onToggleRelic,
		onTogglePalAutoFollow,
		onPalSizeChange,
		onFastTravelSizeChange,
		onWatchtowerSizeChange,
		onRelicSizeChange,
		onPalHeightChange,
		onMapOpacityChange,
		placement = false,
		placementGeometry,
		placementAnchor,
		onPlacementAnchorChange,
		palSize = PAL_SCALE_DEFAULT,
		palAutoFollow = true,
		palHeight = 0,
		mapOpacity = 1,
		fastTravelSize = MAP_OBJECT_SCALE_DEFAULT,
		watchtowerSize = MAP_OBJECT_SCALE_DEFAULT,
		relicSize = MAP_OBJECT_SCALE_DEFAULT
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
		hideCollectedRelics?: boolean;
		hideUnlockedFastTravel?: boolean;
		relicTypes?: Record<string, boolean>;
		showDungeons?: boolean;
		showBosses?: boolean;
		showAlphaPals?: boolean;
		showPredatorPals?: boolean;
		showBounty?: boolean;
		mapLayerVisibility?: MapLayerVisibility;
		showLabels?: boolean;
		show3d?: boolean;
		showStructureControls?: boolean;
		areaSwitchAlign?: 'center' | 'right';
		structureTypes?: Record<string, boolean>;
		renderMode?: 'detailed' | 'flat';
		structureTextured?: boolean;
		onToggle3d?: () => void;
		onToggleStructureType?: (type: string) => void;
		onToggleRenderMode?: () => void;
		onToggleStructureTextured?: () => void;
		onEditBase?: (base: any) => void;
		onExportBase?: (base: any) => void;
		onDeleteBase?: (base: any) => void;
		onToggleFastTravel?: (point: MapUnlockPoint) => void;
		onToggleRelic?: (point: RelicPoint) => void;
		onTogglePalAutoFollow?: () => void;
		onPalSizeChange?: (scale: number) => void;
		onFastTravelSizeChange?: (scale: number) => void;
		onWatchtowerSizeChange?: (scale: number) => void;
		onRelicSizeChange?: (scale: number) => void;
		onPalHeightChange?: (heightCm: number) => void;
		onMapOpacityChange?: (opacity: number) => void;
		placement?: boolean;
		placementGeometry?: BlueprintStructureGeometry[];
		placementAnchor?: PlacementAnchor;
		onPlacementAnchorChange?: (anchor: PlacementAnchor) => void;
		palSize?: number;
		palAutoFollow?: boolean;
		palHeight?: number;
		mapOpacity?: number;
		fastTravelSize?: number;
		watchtowerSize?: number;
		relicSize?: number;
	} = $props();

	const appState = getAppState();

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

	const fastTravelPointList = $derived.by(() => {
		const unlocked = new Set(
			(selectedPlayer?.unlocked_fast_travel_points ?? []).map((guid) => guid.toUpperCase())
		);
		return Object.entries(fastTravelPoints.points)
			.map(([guid, point]) => ({
				guid,
				x: point.x,
				y: point.y,
				z: point.z,
				class: point.class,
				localized_name: point.localized_name ?? point.id,
				unlocked: selectedPlayer ? unlocked.has(guid.toUpperCase()) : undefined
			}))
			.filter((p) => mapOf(p.x, p.y) === area);
	});

	const visibleFastTravelPoints = $derived(
		fastTravelPointList
			.filter((p) => (isWatchtower(p) ? showWatchtower : showFastTravel))
			.filter((p) => !(hideUnlockedFastTravel && p.unlocked === true))
	);

	const collectedRelicGuids = $derived.by(() => {
		const byType: Record<string, Set<string>> = {};
		for (const [type, guids] of Object.entries(
			selectedPlayer ? relicsByType(selectedPlayer) : {}
		)) {
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
				z: point.z,
				relic_type: point.relic_type,
				localized_name: relicData.relics[point.relic_type]?.localized_name ?? point.relic_type,
				unlocked: selectedPlayer
					? (collectedRelicGuids[point.relic_type]?.has(guid.toUpperCase()) ?? false)
					: undefined
			}))
			.filter((p) => mapOf(p.x, p.y) === area)
			.filter((p) => !(hideCollectedRelics && p.unlocked === true));
	});

	const spawnPartition = $derived(partitionSpawns(bosses.points));
	const defeatedSpawnerIds = $derived(new Set(selectedPlayer?.defeated_bosses ?? []));

	const dungeonPoints = $derived(
		Object.values(dungeons.points).filter((p) => mapOf(p.x, p.y) === area)
	);

	const bossPoints = $derived(
		spawnPartition.boss
			.filter((b) => mapOf(b.x, b.y) === area)
			.map((b) => {
				const palKey = bossPalKey(b.character_id);
				const palData = palKey ? palsData.getByKey(palKey) : undefined;
				return {
					...b,
					palKey,
					defeated: defeatedSpawnerIds.has(b.spawner_id),
					localized_name: palData?.localized_name || humanizeSpawnerId(b.spawner_id)
				};
			})
	);

	const alphaSpawnPoints = $derived(
		spawnPartition.alpha
			.filter((b) => mapOf(b.x, b.y) === area)
			.map((b) => ({
				...b,
				palKey: bossPalKey(b.character_id),
				defeated: defeatedSpawnerIds.has(b.spawner_id)
			}))
	);
	const alphaPalPoints = $derived(
		alphaSpawnPoints.map((b) => ({ x: b.x, y: b.y, z: b.z, pal: b.palKey ?? '' }))
	);

	const bountyPoints = $derived(
		spawnPartition.bounty
			.filter((b) => mapOf(b.x, b.y) === area)
			.map((b) => ({
				...b,
				defeated: defeatedSpawnerIds.has(b.spawner_id),
				localized_name: humanizeSpawnerId(b.spawner_id)
			}))
	);

	const predatorSpawnsInArea = $derived(
		spawnPartition.predator.filter((p) => mapOf(p.x, p.y) === area)
	);
	const predatorPalPoints = $derived(
		predatorSpawnsInArea.map((p) => ({ x: p.x, y: p.y, z: p.z, pal: p.pal }))
	);

	const originFC = $derived(showOrigin && area === 'MainMap' ? buildOriginFC(area) : emptyFC());
	const originLinesFC = $derived(buildOriginCrosshairFC(area));
	const playerFC = $derived(buildPlayerFC(players as never, area));
	const baseFC = $derived(buildBaseFC(bases, area));
	const baseRadiusFC = $derived(buildBaseRadiusFC(bases, area));
	const fastTravelFC = $derived(buildFastTravelFC(visibleFastTravelPoints, area));
	const relicFC = $derived(buildRelicFC(relicPointList, area));
	const dungeonFC = $derived(buildMapObjectFC(dungeonPoints, 'dungeon', area));
	const alphaFC = $derived(buildMapObjectFC(alphaPalPoints, 'alpha_pal', area));
	const predatorFC = $derived(buildMapObjectFC(predatorPalPoints, 'predator_pal', area));
	const bossFC = $derived(buildBossFC(bossPoints as never, area));
	const bountyFC = $derived(
		buildBossFC(bountyPoints as never, area, { type: 'bounty', icon: ICON_BOUNTY })
	);

	// Memoized by point-list identity: rebuilding ~580 ring polygons on every unrelated
	// recompute made MapLibre re-tessellate and re-upload both sources.
	function sameRingPoints<T>(a: T[], b: T[]): boolean {
		if (a.length !== b.length) return false;
		for (let i = 0; i < a.length; i++) if (a[i] !== b[i]) return false;
		return true;
	}

	function memoRingFC<T extends { x: number; y: number }>(
		build: (points: T[], area: MapArea, ...scales: number[]) => GeoJSON.FeatureCollection
	) {
		let lastPoints: T[] = [];
		let lastArea: MapArea | null = null;
		let lastScales: number[] = [];
		let last: GeoJSON.FeatureCollection = { type: 'FeatureCollection', features: [] };
		return (points: T[], nextArea: MapArea, ...scales: number[]): GeoJSON.FeatureCollection => {
			if (
				nextArea === lastArea &&
				sameRingPoints(lastScales, scales) &&
				sameRingPoints(lastPoints, points)
			)
				return last;
			lastPoints = points;
			lastArea = nextArea;
			lastScales = scales;
			last = build(points, nextArea, ...scales);
			return last;
		};
	}

	const fastTravelRing = memoRingFC<MapUnlockPoint>(buildFastTravelRingFC);
	const relicRing = memoRingFC<RelicPoint>(buildRelicRingFC);

	const fastTravelRingColor = portalRingColorExpression('fastTravel');
	const relicRingColor = portalRingColorExpression('relic');
	const palPortalRingColor = portalRingColorExpression('palRing', PORTAL_HEX.palRing.boss);
	const fastTravelRingFC = $derived(
		fastTravelRing(show3d ? visibleFastTravelPoints : [], area, fastTravelSize, watchtowerSize)
	);
	const relicRingFC = $derived(
		relicRing(show3d && showRelics ? relicPointList : [], area, relicSize)
	);

	const mapObjectItems = $derived<MapObjectItem[]>(
		buildMapObjectItems(
			show3d,
			{
				points: visibleFastTravelPoints,
				sources: fastTravelPoints.points,
				size: fastTravelSize,
				watchtowerSize
			},
			{ show: showRelics, points: relicPointList, sources: relics.points, size: relicSize }
		)
	);

	const byKey = $derived.by(() => {
		const table = new Map<string, { data: any; guildName?: string }>();
		for (const p of players) table.set(`player:${p.uid}`, { data: p });
		for (const { base, guildName } of bases)
			table.set(`base:${base.id}`, { data: base, guildName });
		for (const p of visibleFastTravelPoints) table.set(`fast_travel:${p.guid}`, { data: p });
		for (const p of relicPointList) table.set(`relic:${p.guid}`, { data: p });
		for (const p of dungeonPoints) table.set(`dungeon:dungeon:${p.x}:${p.y}`, { data: p });
		for (const p of alphaPalPoints) table.set(`alpha_pal:alpha_pal:${p.x}:${p.y}`, { data: p });
		for (const p of predatorPalPoints)
			table.set(`predator_pal:predator_pal:${p.x}:${p.y}`, { data: p });
		for (const b of bossPoints) table.set(`boss:${b.rowKey}`, { data: b });
		if (show3d) {
			for (const { base } of bases) {
				for (const s of baseStructuresData.for(base.id)) {
					table.set(`structure:${s.instance_id}`, { data: s });
				}
			}
		}
		table.set('origin:origin', { data: null });
		return table;
	});

	function lookup(type: string, key: string) {
		return byKey.get(`${type}:${key}`);
	}

	const staticIcons = staticIconUrls();

	const REGISTRY_LAYERS: MapLayerId[] = genericRenderLayers();

	const mapLayerRenders = $derived(
		REGISTRY_LAYERS.map((id) => ({
			id,
			minZoom: getMapLayer(id).minZoom,
			visible: mapLayerVisibility[id] ?? getMapLayer(id).defaultVisible,
			// Folded into the zoom stops rather than multiplied around them: MapLibre only
			// accepts `zoom` as the input to a top-level interpolate, and wrapping it in `*` fails validation.
			iconSize: zoomScaledIconSize(0.6 * mapLayerIconScale(id), mapLayerIconScale(id)),
			fc: buildMapLayerFC(id, mapLayers.peek(id), area)
		}))
	);

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

	$effect(() => {
		const instance = map;
		if (!instance) return;
		const tf = instance.transform;
		tf.setConstrainOverride(worldFittingConstrain(tf));
		return () => tf.setConstrainOverride(null);
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

	let coordDisplayText = $state('Coords: 0, 0<br>Zoom: 0');
	type ScreenPoint = { x: number; y: number };

	let hovered = $state<{
		type: MapFeatureType;
		key: string;
		source?: string;
		id?: string | number;
		point: ScreenPoint | null;
	} | null>(null);
	let selected = $state<{
		type: MapFeatureType;
		key: string;
		lngLat: [number, number];
	} | null>(null);

	// Reprojected on every map move so the popup stays pinned to its feature across pan/zoom.
	let moveTick = $state(0);
	const selectedPoint = $derived.by(() => {
		void moveTick;
		return selected ? projectLngLat(selected.lngLat) : null;
	});

	function featureLngLat(feature: maplibregl.MapGeoJSONFeature): [number, number] | null {
		const geometry = feature.geometry;
		if (geometry.type === 'Point') return geometry.coordinates as [number, number];
		if (geometry.type === 'Polygon') {
			return structureCentroid(feature as unknown as StructureFeature);
		}
		return null;
	}

	function projectLngLat(lngLat: [number, number]): ScreenPoint | null {
		const projected = map?.project(lngLat);
		return projected ? { x: projected.x, y: projected.y } : null;
	}

	const INTERACTIVE_LAYERS = [
		'origin-icons',
		'player-icons',
		'base-icons',
		'fast-travel-icons',
		'relic-icons',
		'dungeon-icons',
		'boss-icons',
		'alpha-icons',
		'predator-icons',
		'structure-extrusions'
	];

	function topFeatureAt(ev: maplibregl.MapMouseEvent, layerIds: string[]) {
		const instance = map;
		if (!instance) return null;
		const usable = detailed ? layerIds.filter((id) => id !== 'structure-extrusions') : layerIds;
		const layers = usable.filter((id) => instance.getLayer(id));
		if (layers.length === 0) return null;
		return instance.queryRenderedFeatures(ev.point, { layers })[0] ?? null;
	}

	let pickSeq = 0;
	let pickScheduled = false;
	let pendingPoint: { x: number; y: number } | null = null;

	// Coalesced to one pick per frame: the pick pass re-renders the scene, so one per
	// mousemove event would be wasteful.
	function schedulePick(x: number, y: number) {
		pendingPoint = { x, y };
		if (pickScheduled) return;
		pickScheduled = true;
		requestAnimationFrame(() => {
			pickScheduled = false;
			const point = pendingPoint;
			pendingPoint = null;
			if (!point || !structureLayer) return;
			const seq = ++pickSeq;
			structureLayer.requestPick(point.x, point.y, (key) => {
				if (seq !== pickSeq) return; // a newer pick superseded this one
				if (key) {
					if (hovered?.key !== key) hovered = { type: 'structure', key, point };
				} else if (hovered) {
					hovered = null;
				}
				const canvas = map?.getCanvas();
				if (canvas) canvas.style.cursor = key ? 'pointer' : '';
			});
		});
	}

	let ghostDragging = false;

	const STRUCTURE_HIT_CM = 800;

	function clickIsOnGhost(worldX: number, worldY: number): boolean {
		const base = placementAnchor ?? { x: 0, y: 0, z: 0, yaw: 0 };
		return (placementGeometry ?? []).some((s) => {
			const w = composeWorld(base, {
				translation: s.translation,
				rotation: s.rotation,
				scale: s.scale
			});
			return Math.hypot(worldX - w.translation.x, worldY - w.translation.y) <= STRUCTURE_HIT_CM;
		});
	}

	function handleMouseDown(ev: maplibregl.MapMouseEvent) {
		if (!placement) return;
		const oe = ev.originalEvent;
		if (oe.button !== 0) return;
		const [px, py] = lngLatToPixel(ev.lngLat.lng, ev.lngLat.lat);
		const { worldX, worldY } = pixelToWorld(px, py, area);
		const base = placementAnchor ?? { x: 0, y: 0, z: 0, yaw: 0 };
		if (oe.ctrlKey) {
			onPlacementAnchorChange?.({ ...base, x: worldX, y: worldY });
			return;
		}
		if (clickIsOnGhost(worldX, worldY)) {
			ghostDragging = true;
			map?.dragPan.disable();
		}
	}

	// MapLibre's own mouseup only fires over the canvas, so a window-level listener
	// (below) is needed to end a drag released over the PlacementPanel or off-window.
	function endGhostDrag() {
		if (!ghostDragging) return;
		ghostDragging = false;
		map?.dragPan.enable();
	}

	function handleMouseUp() {
		endGhostDrag();
	}

	function handleMouseMove(ev: maplibregl.MapMouseEvent) {
		const [px, py] = lngLatToPixel(ev.lngLat.lng, ev.lngLat.lat);
		const { worldX, worldY } = pixelToWorld(px, py, area);
		const { gameX, gameY } = pixelToGameCoords(px, py, area);
		coordDisplayText = `Coords: ${gameX}, ${gameY}<br>Zoom: ${zoom.toFixed(2)}<br>Pitch: ${pitch.toFixed(1)}	`;

		if (placement) {
			if (ghostDragging) {
				const base = placementAnchor ?? { x: 0, y: 0, z: 0, yaw: 0 };
				onPlacementAnchorChange?.({ ...base, x: worldX, y: worldY });
			} else {
				const canvas = map?.getCanvas();
				if (canvas) canvas.style.cursor = clickIsOnGhost(worldX, worldY) ? 'move' : '';
			}
			return;
		}

		const top = topFeatureAt(ev, INTERACTIVE_LAYERS);
		if (top) {
			// Bumps the sequence to discard any in-flight pick, which resolves a frame
			// later and would otherwise land after this hit and stomp it.
			pickSeq++;
			pendingPoint = null;
			const source = top.source;
			const id = top.id as string | number;
			if (hovered?.source !== source || hovered?.id !== id) {
				const lngLat = featureLngLat(top);
				hovered = {
					type: top.properties.type as MapFeatureType,
					key: String(top.properties.key),
					source,
					id,
					point: lngLat ? projectLngLat(lngLat) : { x: ev.point.x, y: ev.point.y }
				};
			}
		} else if (detailed && structureLayer) {
			schedulePick(ev.point.x, ev.point.y);
		} else if (hovered) {
			hovered = null;
		}
		const canvas = map?.getCanvas();
		if (canvas) canvas.style.cursor = top ? 'pointer' : '';
	}

	function handleMouseOut() {
		pickSeq++;
		pendingPoint = null;
		hovered = null;
		endGhostDrag();
		const canvas = map?.getCanvas();
		if (canvas) canvas.style.cursor = '';
	}

	function handleClick(ev: maplibregl.MapMouseEvent) {
		if (placement) return;
		const top = topFeatureAt(ev, INTERACTIVE_LAYERS);
		if (!top && detailed && hovered?.type === 'structure') {
			const structure = lookup('structure', hovered.key)?.data;
			if (structure) {
				const [px, py] = worldToPixel(structure.x, structure.y, area);
				selected = { type: 'structure', key: hovered.key, lngLat: pixelToLngLat(px, py) };
				return;
			}
		}
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
		selected = { type, key, lngLat: featureLngLat(top) ?? [ev.lngLat.lng, ev.lngLat.lat] };
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

	function areaCenter(target: MapArea): [number, number] {
		const world = mapToWorld(0, 0);
		const [px, py] = worldToPixel(world.x, world.y, target);
		return pixelToLngLat(px, py);
	}

	let center = $state<[number, number]>(untrack(() => areaCenter(area)));
	let zoom = $state(2);
	let pitch = $state(0);
	let bearing = $state(0);

	$effect(() => {
		center = areaCenter(area);
	});

	const verticalScale = $derived(verticalScaleFactor(center[1], cmPerPx(area)));

	let lastGroundBounds: maplibregl.LngLatBounds | null = null;

	$effect(() => {
		if (!show3d || zoom < STRUCTURE_MIN_ZOOM) return;
		const instance = map;
		if (!instance) return;
		void center;
		let bounds = lastGroundBounds;
		if (!bounds || pitch <= BOUNDS_PITCH_LIMIT) {
			bounds = instance.getBounds();
			lastGroundBounds = bounds;
		}
		baseStructuresData.loadFootprints();
		buildingsData.ensureLoaded().catch(() => {});
		for (const { base } of bases) {
			const loc = base.location;
			if (!loc) continue;
			const [px, py] = worldToPixel(loc.x, loc.y, area);
			if (!bounds.contains(pixelToLngLat(px, py))) continue;
			baseStructuresData.load(base.id);
		}
	});

	const structureFC = $derived.by<StructureFC>(() => {
		const features: StructureFC['features'] = [];
		if (!show3d) return { type: 'FeatureCollection', features };
		for (const { base } of bases) {
			const loc = base.location;
			if (!loc) continue;
			const structures = baseStructuresData.for(base.id);
			if (structures.length === 0) continue;
			const fc = buildStructureFC(structures, baseStructuresData.footprints, loc.z, area);
			features.push(...fc.features);
		}
		return { type: 'FeatureCollection', features };
	});

	let structureLayer: StructureLayer | null = null;
	let pendingStyleHandler: (() => void) | null = null;
	const detailed = $derived(show3d && renderMode === 'detailed');

	let ghostLayer: GhostLayer | null = null;
	let pendingGhostStyleHandler: (() => void) | null = null;

	function mount3dLayer(instance: maplibregl.Map, layer: maplibregl.CustomLayerInterface) {
		const mounted = LAYER_ORDER_3D.filter((id) => instance.getLayer(id));
		instance.addLayer(layer, beforeIdFor(layer.id as (typeof LAYER_ORDER_3D)[number], mounted));
	}

	function populateStructureLayer() {
		if (!structureLayer) return;
		const all: BaseStructure[] = [];
		for (const { base } of bases) {
			const loc = base.location;
			if (!loc) continue;
			for (const s of baseStructuresData.for(base.id)) {
				const typeA =
					lookupFootprint(baseStructuresData.footprints, s.map_object_id)?.typeA ?? 'Other';
				if (structureTypes[typeA] === false) continue;
				all.push(s);
			}
		}
		structureLayer.update(
			all,
			baseStructuresData.footprints,
			area,
			verticalScale,
			structureTextured
		);
	}

	$effect(() => {
		const instance = map;
		if (!instance) return;
		if (detailed && !structureLayer) {
			// MapLibre throws if a layer is added before the style finishes loading; only
			// keep the reference once addLayer actually succeeds, retrying on `styledata`.
			const add = () => {
				if (!instance.isStyleLoaded()) return false;
				// isStyleLoaded() goes true before this component's layers exist. Without this
				// anchor check, a 3D layer mounted early lands below the rasters that mount
				// afterwards, which repaint over anything it draws without writing depth (the portal beam).
				if (!instance.getLayer('origin-icons')) return false;
				const layer = createStructureLayer({ id: 'structure-3d' });
				mount3dLayer(instance, layer);
				structureLayer = layer;
				populateStructureLayer();
				return true;
			};
			// `styledata` fires many times and often before isStyleLoaded() flips true, so a
			// one-shot retry can miss every firing; keep listening until add() succeeds.
			if (!add() && !pendingStyleHandler) {
				const onStyle = () => {
					if (!detailed || structureLayer) {
						instance.off('styledata', onStyle);
						pendingStyleHandler = null;
						return;
					}
					if (add()) {
						instance.off('styledata', onStyle);
						pendingStyleHandler = null;
					}
				};
				pendingStyleHandler = onStyle;
				instance.on('styledata', onStyle);
			}
		}
		if (!detailed && structureLayer) {
			if (instance.getLayer('structure-3d')) instance.removeLayer('structure-3d');
			structureLayer.dispose();
			structureLayer = null;
		}
		if (!detailed && pendingStyleHandler) {
			instance.off('styledata', pendingStyleHandler);
			pendingStyleHandler = null;
		}
	});

	let sceneryStreamsByArea: Partial<Record<MapArea, SceneryStream>> = $state({});
	let sceneryStreamAttempted = $state(new Set<MapArea>());
	const sceneryStream = $derived(sceneryStreamsByArea[area] ?? null);

	let sceneryLayer: SceneryLayer | null = null;
	let pendingSceneryStyleHandler: (() => void) | null = null;

	let palLayer: PalLayer | null = null;
	let pendingPalStyleHandler: (() => void) | null = null;

	let mapObjectLayer: MapObjectLayer | null = null;
	let pendingMapObjectStyleHandler: (() => void) | null = null;
	const palBosses = $derived(
		[...(showAlphaPals ? alphaSpawnPoints : []), ...(showBosses ? bossPoints : [])]
			.filter((b) => b.palKey)
			.map(
				(b): PalBoss => ({
					key: b.palKey as string,
					x: b.x,
					y: b.y,
					z: b.z,
					defeated: b.defeated
				})
			)
	);
	const predatorPalModels = $derived<PalPredator[]>(
		showPredatorPals ? predatorPalBosses(predatorSpawnsInArea) : []
	);
	const palPortalFC = $derived(buildPalPortalFC(palBosses, predatorPalModels, area, palSize));

	$effect(() => {
		if (!show3d) return;
		const currentArea = area;
		if (sceneryStreamAttempted.has(currentArea)) return;
		sceneryStreamAttempted = new Set(sceneryStreamAttempted).add(currentArea);
		fetch(sceneryStreamUrl(currentArea))
			.then((r) => (r.ok ? r.arrayBuffer() : Promise.reject(new Error(`${r.status}`))))
			.then((buf) => {
				sceneryStreamsByArea = { ...sceneryStreamsByArea, [currentArea]: decodeSceneryStream(buf) };
			})
			.catch((e) =>
				console.warn(
					`[scenery] stream unavailable for ${currentArea}, layer disabled for this area`,
					e
				)
			);
	});

	let tintMosaicsByArea: Partial<Record<MapArea, TintMosaic>> = $state({});
	let tintMosaicAttempted = $state(new Set<MapArea>());

	$effect(() => {
		if (!show3d) return;
		const currentArea = area;
		if (tintMosaicAttempted.has(currentArea)) return;
		tintMosaicAttempted = new Set(tintMosaicAttempted).add(currentArea);
		// Captured as currentArea, not the live binding, so a mosaic resolving after an
		// area switch still lands in its own slot.
		loadTintMosaic(currentArea).then((mosaic) => {
			tintMosaicsByArea = { ...tintMosaicsByArea, [currentArea]: mosaic };
		});
	});

	$effect(() => {
		const instance = map;
		if (!instance) return;
		const streamToMount = sceneryStream;
		if (show3d && streamToMount && !sceneryLayer) {
			const add = () => {
				if (!instance.isStyleLoaded()) return false;
				if (!instance.getLayer('origin-icons')) return false;
				const layer = createSceneryLayer({ id: 'scenery-3d' });
				mount3dLayer(instance, layer);
				sceneryLayer = layer;
				sceneryLayer.setTint(tintMosaicsByArea[area] ?? null);
				sceneryLayer.setOpacity(mapOpacity);
				sceneryLayer.update(streamToMount, area, verticalScale);
				return true;
			};
			if (!add() && !pendingSceneryStyleHandler) {
				const onStyle = () => {
					if (!show3d || !sceneryStream || sceneryLayer) {
						instance.off('styledata', onStyle);
						pendingSceneryStyleHandler = null;
						return;
					}
					if (add()) {
						instance.off('styledata', onStyle);
						pendingSceneryStyleHandler = null;
					}
				};
				pendingSceneryStyleHandler = onStyle;
				instance.on('styledata', onStyle);
			}
		}
		if ((!show3d || !sceneryStream) && sceneryLayer) {
			if (instance.getLayer('scenery-3d')) instance.removeLayer('scenery-3d');
			sceneryLayer.dispose();
			sceneryLayer = null;
		}
		if (!show3d && pendingSceneryStyleHandler) {
			instance.off('styledata', pendingSceneryStyleHandler);
			pendingSceneryStyleHandler = null;
		}
	});

	// sceneryLayer.update() owns no camera subscription and re-derives its culled
	// bucket set per call, so the caller must re-invoke it on every camera move.
	// zoom, center and pitch are read here purely to make this effect track them.
	$effect(() => {
		const stream = sceneryStream;
		const currentArea = area;
		const vScale = verticalScale;
		const currentTint = tintMosaicsByArea[currentArea] ?? null;
		void zoom;
		void center;
		void pitch;
		if (!stream || !sceneryLayer) return;
		sceneryLayer.setTint(currentTint);
		sceneryLayer.update(stream, currentArea, vScale);
	});

	$effect(() => {
		const instance = map;
		if (!instance) return;
		if (show3d && !palLayer) {
			const add = () => {
				if (!instance.isStyleLoaded()) return false;
				if (!instance.getLayer('origin-icons')) return false;
				const layer = createPalLayer({ id: 'pals-3d' });
				mount3dLayer(instance, layer);
				palLayer = layer;
				palLayer.update(
					palBosses,
					area,
					verticalScale,
					{
						scale: palSize,
						heightCm: palHeight,
						autoFollow: palAutoFollow,
						xray: mapOpacity < 1
					},
					predatorPalModels
				);
				return true;
			};
			if (!add() && !pendingPalStyleHandler) {
				const onStyle = () => {
					if (!show3d || palLayer) {
						instance.off('styledata', onStyle);
						pendingPalStyleHandler = null;
						return;
					}
					if (add()) {
						instance.off('styledata', onStyle);
						pendingPalStyleHandler = null;
					}
				};
				pendingPalStyleHandler = onStyle;
				instance.on('styledata', onStyle);
			}
		}
		if (!show3d && palLayer) {
			if (instance.getLayer('pals-3d')) instance.removeLayer('pals-3d');
			palLayer.dispose();
			palLayer = null;
		}
		if (!show3d && pendingPalStyleHandler) {
			instance.off('styledata', pendingPalStyleHandler);
			pendingPalStyleHandler = null;
		}
	});

	// palLayer.update() owns no camera subscription, so the caller must re-invoke
	// it on every camera move -- bearing included, so Pals turn to face the camera.
	$effect(() => {
		const bosses = palBosses;
		const predators = predatorPalModels;
		const currentArea = area;
		const vScale = verticalScale;
		void zoom;
		void center;
		void pitch;
		void bearing;
		void palSize;
		void palAutoFollow;
		void palHeight;
		void mapOpacity;
		if (!palLayer) return;
		palLayer.update(
			bosses,
			currentArea,
			vScale,
			{
				scale: palSize,
				heightCm: palHeight,
				autoFollow: palAutoFollow,
				xray: mapOpacity < 1
			},
			predators
		);
	});

	$effect(() => {
		const instance = map;
		if (!instance) return;
		if (show3d && !mapObjectLayer) {
			const add = () => {
				if (!instance.isStyleLoaded()) return false;
				if (!instance.getLayer('origin-icons')) return false;
				const layer = createMapObjectLayer('map-objects-3d');
				mount3dLayer(instance, layer);
				mapObjectLayer = layer;
				mapObjectLayer.update(mapObjectItems, area, verticalScale);
				return true;
			};
			if (!add() && !pendingMapObjectStyleHandler) {
				const onStyle = () => {
					if (!show3d || mapObjectLayer) {
						instance.off('styledata', onStyle);
						pendingMapObjectStyleHandler = null;
						return;
					}
					if (add()) {
						instance.off('styledata', onStyle);
						pendingMapObjectStyleHandler = null;
					}
				};
				pendingMapObjectStyleHandler = onStyle;
				instance.on('styledata', onStyle);
			}
		}
		if (!show3d && mapObjectLayer) {
			if (instance.getLayer('map-objects-3d')) instance.removeLayer('map-objects-3d');
			mapObjectLayer.dispose();
			mapObjectLayer = null;
		}
		if (!show3d && pendingMapObjectStyleHandler) {
			instance.off('styledata', pendingMapObjectStyleHandler);
			pendingMapObjectStyleHandler = null;
		}
	});

	// mapObjectLayer.update() owns no camera subscription and re-derives the centre
	// its cull distances are measured from, so re-invoke it on every camera move.
	$effect(() => {
		const items = mapObjectItems;
		const currentArea = area;
		const vScale = verticalScale;
		void zoom;
		void center;
		void pitch;
		if (!mapObjectLayer) return;
		mapObjectLayer.update(items, currentArea, vScale);
	});

	$effect(() => {
		const o = mapOpacity;
		if (!sceneryLayer) return;
		sceneryLayer.setOpacity(o);
	});

	$effect(() => {
		const instance = map;
		if (!instance) return;
		if (placement && !ghostLayer) {
			const add = () => {
				if (!instance.isStyleLoaded()) return false;
				if (!instance.getLayer('origin-icons')) return false;
				const layer = createGhostLayer({ id: 'blueprint-ghost' });
				// Mounted below pals-3d, which clears the depth buffer under x-ray: a ghost drawn
				// after it would float over the terrain and misreport where the blueprint lands.
				mount3dLayer(instance, layer);
				ghostLayer = layer;
				ghostLayer.update(
					placementGeometry ?? [],
					placementAnchor ?? { x: 0, y: 0, z: 0, yaw: 0 },
					area,
					verticalScale
				);
				return true;
			};
			if (!add() && !pendingGhostStyleHandler) {
				const onStyle = () => {
					if (!placement || ghostLayer) {
						instance.off('styledata', onStyle);
						pendingGhostStyleHandler = null;
						return;
					}
					if (add()) {
						instance.off('styledata', onStyle);
						pendingGhostStyleHandler = null;
					}
				};
				pendingGhostStyleHandler = onStyle;
				instance.on('styledata', onStyle);
			}
		}
		if (!placement && ghostLayer) {
			if (instance.getLayer('blueprint-ghost')) instance.removeLayer('blueprint-ghost');
			ghostLayer.dispose();
			ghostLayer = null;
			endGhostDrag();
		}
		if (!placement && pendingGhostStyleHandler) {
			instance.off('styledata', pendingGhostStyleHandler);
			pendingGhostStyleHandler = null;
		}
	});

	$effect(() => {
		if (!placement) return;
		window.addEventListener('mouseup', endGhostDrag);
		return () => window.removeEventListener('mouseup', endGhostDrag);
	});

	$effect(() => {
		const isPlacement = placement;
		const geometry = placementGeometry ?? [];
		const anchor = placementAnchor ?? { x: 0, y: 0, z: 0, yaw: 0 };
		const currentArea = area;
		const vScale = verticalScale;
		if (!isPlacement || !ghostLayer) return;
		ghostLayer.update(geometry, anchor, currentArea, vScale);
	});

	$effect(() => {
		// Read every reactive dependency before any early return: Svelte only tracks
		// sources a run actually reads, and bailing out immediately would leave this
		// effect tracking only `detailed`, silently missing later structure data.
		const isDetailed = detailed;
		const baseList = bases;
		const footprints = baseStructuresData.footprints;
		const types = structureTypes;
		const currentArea = area;
		const colors = structureColors();
		const tints = materialTints();
		const blend = materialBlend();
		const opacities = materialOpacities();
		const isTextured = structureTextured;
		let total = 0;
		for (const { base } of baseList) total += baseStructuresData.for(base.id).length;
		void footprints;
		void types;
		void currentArea;
		void colors;
		void tints;
		void blend;
		void opacities;
		void isTextured;
		void total;
		if (!isDetailed || !structureLayer) return;
		populateStructureLayer();
	});

	// verticalScale is $derived from the map centre latitude and so changes on
	// every camera move. It stays out of the heavy effect above, which rebuilds
	// every group and material; this one only recomposes matrices.
	$effect(() => {
		const vScale = verticalScale;
		if (!detailed || !structureLayer) return;
		structureLayer.setVerticalScale(vScale);
	});

	$effect(() => {
		// Read `hovered` before the layer check: optional-chaining would short-circuit
		// and Svelte would never track it, so the effect would never re-run.
		const current = hovered;
		if (!structureLayer) return;
		structureLayer.setHover(current?.type === 'structure' ? current.key : null);
	});

	// The wrapper reads maxZoom/pitchWithRotate only at construction, and DragRotateHandler
	// captures pitchWithRotate too — enable() can never turn pitch back on. So the map is
	// built pitch-capable and a maxPitch of 0 is what actually keeps 2D flat.
	$effect(() => {
		const instance = map;
		if (!instance) return;
		instance.setMaxZoom(show3d ? 11 : 7);
		instance.setMaxPitch(show3d ? (detailed ? MAX_PITCH_DETAILED : MAX_PITCH_FLAT) : 0);
		if (show3d) {
			instance.dragRotate.enable();
		} else {
			instance.dragRotate.disable();
			// setBearing delegates to jumpTo, which fires `move` even when nothing
			// changes, so calling it unconditionally manufactures map events.
			if (instance.getBearing() !== 0) instance.setBearing(0);
		}
	});

	const structureColor = $derived.by<ExpressionSpecification>(() => {
		const colors = structureColors();
		return [
			'case',
			['boolean', ['feature-state', 'hover'], false],
			'#ffffff',
			[
				'match',
				['get', 'typeA'],
				...(Object.entries(colors).flat() as [string, string, ...string[]]),
				colors.Other
			]
		];
	});

	const structureTypeList = STRUCTURE_TYPE_ORDER;
	let options3dOpen = $state(false);

	const structureFilter = $derived.by<FilterSpecification | undefined>(() => {
		const hidden = Object.entries(structureTypes)
			.filter(([, visible]) => visible === false)
			.map(([type]) => type);
		if (hidden.length === 0) return undefined;
		return ['!', ['in', ['get', 'typeA'], ['literal', hidden]]];
	});

	const relicHaloRadius = zoomScaledRadius(
		haloRadiusPx(RELIC_ART_PX, 0.4),
		haloRadiusPx(RELIC_ART_PX, 0.6)
	);

	// Watchtowers share this layer with fast travel points but differ in both art
	// extent and icon size, so the per-class split lives inside the interpolate
	// stops -- a `case` wrapping two interpolates would nest zoom and fail validation.
	const fastTravelHaloRadius = zoomScaledRadius(
		[
			'case',
			['get', 'watchtower'],
			haloRadiusPx(WATCHTOWER_ART_PX, 0.36),
			haloRadiusPx(FAST_TRAVEL_ART_PX, 0.45)
		],
		[
			'case',
			['get', 'watchtower'],
			haloRadiusPx(WATCHTOWER_ART_PX, 0.6),
			haloRadiusPx(FAST_TRAVEL_ART_PX, 0.75)
		]
	);

	$effect(() => {
		if (!show3d) {
			if (selected?.type === 'structure') selected = null;
			if (hovered?.type === 'structure') hovered = null;
			return;
		}
		const structureType = (key: string) => {
			const data = byKey.get(`structure:${key}`)?.data;
			if (!data) return undefined;
			return lookupFootprint(baseStructuresData.footprints, data.map_object_id)?.typeA ?? 'Other';
		};
		if (selected?.type === 'structure') {
			const type = structureType(selected.key);
			if (!type || structureTypes[type] === false) selected = null;
		}
		if (hovered?.type === 'structure') {
			const type = structureType(hovered.key);
			if (!type || structureTypes[type] === false) hovered = null;
		}
	});

	// Uses `has`, not a truthy `data` check, because the origin entry is legitimately
	// present with `data: null`; a truthy check would wrongly clear `hovered` for it.
	$effect(() => {
		const current = hovered;
		if (!current) return;
		if (!byKey.has(`${current.type}:${current.key}`)) hovered = null;
	});
</script>

<div class="relative h-full w-full">
	<MLMap
		bind:map
		class="h-full w-full"
		style={EMPTY_STYLE}
		bind:center
		bind:zoom
		bind:pitch
		bind:bearing
		minZoom={0}
		maxZoom={7}
		renderWorldCopies={false}
		centerClampedToGround={false}
		dragRotate={show3d}
		pitchWithRotate={true}
		touchZoomRotate={show3d}
		attributionControl={false}
		onmove={() => moveTick++}
		onmousemove={handleMouseMove}
		onmouseout={handleMouseOut}
		onmousedown={handleMouseDown}
		onmouseup={handleMouseUp}
		onclick={handleClick}
		oncontextmenu={handleContextMenu}
	>
		<Control.Navigation position="top-right" visualizePitch />
		<Control.Fullscreen position="top-right" />
		<Toggle3dControl
			position="top-right"
			active={show3d}
			title={showStructureControls ? `3D ${m.structures()}` : '3D'}
			onchange={onToggle3d}
		/>
		<Map3dOptionsControl
			position="top-right"
			types={structureTypeList}
			enabled={structureTypes}
			open={options3dOpen}
			onToggleOpen={() => (options3dOpen = !options3dOpen)}
			ontoggle={(type) => onToggleStructureType?.(type)}
			title={m.map_3d_options()}
			{show3d}
			{showStructureControls}
			{detailed}
			textured={structureTextured}
			{palAutoFollow}
			ontoggledetailed={() => onToggleRenderMode?.()}
			ontoggletextured={() => onToggleStructureTextured?.()}
			ontogglepalautofollow={() => onTogglePalAutoFollow?.()}
			{palSize}
			{fastTravelSize}
			{watchtowerSize}
			{relicSize}
			{palHeight}
			{mapOpacity}
			onPalSizeChange={(scale) => onPalSizeChange?.(scale)}
			onFastTravelSizeChange={(scale) => onFastTravelSizeChange?.(scale)}
			onWatchtowerSizeChange={(scale) => onWatchtowerSizeChange?.(scale)}
			onRelicSizeChange={(scale) => onRelicSizeChange?.(scale)}
			onPalHeightChange={(height) => onPalHeightChange?.(height)}
			onMapOpacityChange={(opacity) => onMapOpacityChange?.(opacity)}
		/>

		<ImageLoader images={staticIcons}>
			<!-- Declared before the DEM block so the hillshade always has a raster to anchor
			     against. Using the first area's rather than the visible one keeps the id
			     independent of which area is selected. -->
			{#each MAP_AREA_ORDER as candidate}
				<Source.Raster
					tiles={[`/maps/${MAP_TILE_DIR[candidate]}/{z}/{x}/{y}.webp`]}
					tileSize={512}
					maxzoom={4}
				>
					<Layer.Raster
						id="raster-{candidate}"
						visible={area === candidate}
						paint={{ 'raster-fade-duration': 300, 'raster-opacity': mapOpacity }}
					/>
				</Source.Raster>
			{/each}

			<!-- Each area is baked against its own world extent, and both share one lng/lat
			     tile namespace, so only one AREA's DEM may be mounted at a time. #key forces a
			     full teardown/rebuild on area change rather than mutating a live source. -->
			{#if show3d || mapOpacity < 1}
				{#key area}
					<Source.RasterDEM
						id="dem-{MAP_TILE_DIR[area]}"
						tiles={[`/maps/dem/${MAP_TILE_DIR[area]}/{z}/{x}/{y}.png`]}
						tileSize={512}
						maxzoom={4}
						encoding="custom"
						redFactor={512}
						greenFactor={2}
						blueFactor={0}
						baseShift={50000}
					>
						<!-- Same cm-to-MapLibre-metre scalar the extrusions below use, so terrain and
						     buildings share one vertical space and sum correctly. -->
						{#if show3d}
							<Terrain source="dem-{MAP_TILE_DIR[area]}" exaggeration={verticalScale} />
						{/if}
					</Source.RasterDEM>

					<!-- Identical tiles under a second id: MapLibre degrades hillshade quality
					     when one source feeds both it and terrain, and the HTTP cache makes
					     the duplicate free. -->
					<Source.RasterDEM
						id="dem-hs-{MAP_TILE_DIR[area]}"
						tiles={[`/maps/dem/${MAP_TILE_DIR[area]}/{z}/{x}/{y}.png`]}
						tileSize={512}
						maxzoom={4}
						encoding="custom"
						redFactor={512}
						greenFactor={2}
						blueFactor={0}
						baseShift={50000}
					>
						<!-- Anchored explicitly, not by declaration order: the {#key} above tears
						     this down on every area switch, and an unanchored re-add lands on top
						     of the style instead of back under the raster. -->
						<Layer.Hillshade
							id="hillshade"
							beforeId="raster-{MAP_AREA_ORDER[0]}"
							visible={mapOpacity < 1}
							paint={{ 'hillshade-exaggeration': 0.5 }}
						/>
					</Source.RasterDEM>
				{/key}
			{/if}

			<Source.GeoJSON data={originLinesFC}>
				<Layer.Line
					visible={showOrigin && area === 'MainMap'}
					paint={{ 'line-color': '#ffffff', 'line-width': 0.5, 'line-dasharray': [4, 8] }}
				/>
			</Source.GeoJSON>

			<Source.GeoJSON id="base-radius-src" data={baseRadiusFC}>
				<Layer.Fill visible={showBases} paint={{ 'fill-color': '#0000ff', 'fill-opacity': 0.2 }} />
				<Layer.Line
					visible={showBases}
					paint={{ 'line-color': '#0000ff', 'line-width': 2, 'line-dasharray': [4, 8] }}
				/>
			</Source.GeoJSON>

			<Source.GeoJSON id="origin-src" data={originFC}>
				<Layer.Symbol
					id="origin-icons"
					visible={showOrigin && area === 'MainMap'}
					layout={{
						'icon-image': ['get', 'icon'],
						'icon-allow-overlap': true,
						'icon-size': zoomScaledIconSize(0.6, 1.0)
					}}
				/>
			</Source.GeoJSON>

			<!-- Declared after `origin-icons` so the anchor exists at mount, and anchored to it so
			     toggling 3D on at runtime inserts beneath the icon layers instead of on top. -->
			{#if show3d}
				<Source.GeoJSON id="structure-src" data={structureFC} promoteId="key">
					<Layer.FillExtrusion
						id="structure-extrusions"
						beforeId="origin-icons"
						visible={!detailed}
						minzoom={STRUCTURE_MIN_ZOOM}
						filter={structureFilter}
						paint={{
							'fill-extrusion-color': structureColor,
							'fill-extrusion-base': ['*', ['get', 'b'], verticalScale],
							'fill-extrusion-height': ['*', ['get', 'h'], verticalScale],
							'fill-extrusion-opacity': 0.9
						}}
					/>
				</Source.GeoJSON>
			{/if}

			<!-- Draped 2D layers, not the three.js portal column: MapLibre's
			     render-to-texture pass conforms these to terrain, unlike the rigid flat
			     disc this replaces. -->
			{#if show3d}
				<Source.GeoJSON id="pal-portal-src" data={palPortalFC}>
					<Layer.Fill
						id="pal-portal-fill"
						beforeId="origin-icons"
						paint={{
							'fill-color': palPortalRingColor,
							'fill-opacity': ['case', ['get', 'defeated'], 0.08, 0.22]
						}}
					/>
					<Layer.Line
						id="pal-portal-line"
						beforeId="origin-icons"
						paint={{
							'line-color': palPortalRingColor,
							'line-width': 2,
							'line-opacity': ['case', ['get', 'defeated'], 0.3, 0.9]
						}}
					/>
				</Source.GeoJSON>
			{/if}

			<Source.GeoJSON id="base-src" data={baseFC}>
				<Layer.Symbol
					id="base-icons"
					visible={showBases}
					layout={{
						'icon-image': ['get', 'icon'],
						'icon-allow-overlap': true,
						'icon-size': zoomScaledIconSize(0.5, 0.83)
					}}
				/>
			</Source.GeoJSON>

			<Source.GeoJSON id="player-src" data={playerFC}>
				<Layer.Symbol
					id="player-icons"
					visible={showPlayers}
					layout={{
						'icon-image': ['get', 'icon'],
						'icon-allow-overlap': true,
						'icon-size': zoomScaledIconSize(0.6, 1.0)
					}}
				/>
			</Source.GeoJSON>

			<!-- Layers below label icons with a `step` expression on `text-field` rather than a
			     layer-level `minzoom`, because MapLibre gates a symbol layer's icon and text
			     together — a `minzoom` here would hide the icon along with the label. -->
			<Source.GeoJSON id="fast-travel-src" data={fastTravelFC}>
				<Layer.Circle
					id="fast-travel-glow"
					beforeId="origin-icons"
					visible={showFastTravel || showWatchtower}
					filter={['==', ['get', 'locked'], true]}
					paint={{
						'circle-color': HALO_COLOR,
						'circle-radius': fastTravelHaloRadius,
						'circle-blur': 0.7,
						'circle-opacity': 0.55,
						'circle-pitch-scale': 'viewport'
					}}
				/>
				<Layer.Circle
					id="fast-travel-ring"
					beforeId="origin-icons"
					visible={showFastTravel || showWatchtower}
					filter={['==', ['get', 'locked'], true]}
					paint={{
						'circle-color': HALO_COLOR,
						'circle-opacity': 0,
						'circle-radius': fastTravelHaloRadius,
						'circle-stroke-color': HALO_COLOR,
						'circle-stroke-width': 1.5,
						'circle-stroke-opacity': 0.95,
						'circle-pitch-scale': 'viewport'
					}}
				/>
				<Layer.Symbol
					id="fast-travel-icons"
					visible={showFastTravel || showWatchtower}
					layout={{
						'icon-image': ['get', 'icon'],
						'icon-allow-overlap': true,
						'symbol-sort-key': ['case', ['get', 'locked'], 1, 2],
						'icon-size': zoomScaledIconSize(
							['case', ['get', 'watchtower'], 0.36, 0.45],
							['case', ['get', 'watchtower'], 0.6, 0.75]
						),
						'text-field': showLabels ? ['step', ['zoom'], '', 4, ['get', 'name']] : '',
						'text-size': 14,
						'text-optional': true,
						'text-anchor': 'top',
						'text-offset': [0, 0.8],
						'text-max-width': 12
					}}
					paint={{
						'icon-opacity': [
							'case',
							['boolean', ['feature-state', 'hover'], false],
							1,
							['get', 'locked'],
							0.6,
							1
						],
						'text-color': '#ffffff',
						'text-halo-color': '#000000',
						'text-halo-width': 1.5
					}}
				/>
			</Source.GeoJSON>

			<Source.GeoJSON id="relic-src" data={relicFC}>
				<Layer.Circle
					id="relic-glow"
					beforeId="origin-icons"
					visible={showRelics}
					filter={['==', ['get', 'collected'], false]}
					paint={{
						'circle-color': HALO_COLOR,
						'circle-radius': relicHaloRadius,
						'circle-blur': 0.7,
						'circle-opacity': 0.55,
						'circle-pitch-scale': 'viewport'
					}}
				/>
				<Layer.Circle
					id="relic-ring"
					beforeId="origin-icons"
					visible={showRelics}
					filter={['==', ['get', 'collected'], false]}
					paint={{
						'circle-color': HALO_COLOR,
						'circle-opacity': 0,
						'circle-radius': relicHaloRadius,
						'circle-stroke-color': HALO_COLOR,
						'circle-stroke-width': 1.5,
						'circle-stroke-opacity': 0.95,
						'circle-pitch-scale': 'viewport'
					}}
				/>
				<Layer.Symbol
					id="relic-icons"
					visible={showRelics}
					layout={{
						'icon-image': ['get', 'icon'],
						'icon-allow-overlap': true,
						'symbol-sort-key': ['case', ['get', 'collected'], 2, 1],
						'icon-size': zoomScaledIconSize(0.4, 0.6)
					}}
					paint={{
						'icon-opacity': [
							'case',
							['boolean', ['feature-state', 'hover'], false],
							1,
							['get', 'collected'],
							1,
							0.6
						]
					}}
				/>
			</Source.GeoJSON>

			{#if show3d}
				<Source.GeoJSON id="fast-travel-ring-src" data={fastTravelRingFC}>
					<Layer.Fill
						id="fast-travel-ring-fill"
						beforeId="origin-icons"
						paint={{
							'fill-color': fastTravelRingColor,
							'fill-opacity': 0.22
						}}
					/>
					<Layer.Line
						id="fast-travel-ring-line"
						beforeId="origin-icons"
						paint={{
							'line-color': fastTravelRingColor,
							'line-width': 2,
							'line-opacity': 0.9
						}}
					/>
				</Source.GeoJSON>
				<Source.GeoJSON id="relic-ring-src" data={relicRingFC}>
					<Layer.Fill
						id="relic-ring-fill"
						beforeId="origin-icons"
						paint={{
							'fill-color': relicRingColor,
							'fill-opacity': 0.22
						}}
					/>
					<Layer.Line
						id="relic-ring-line"
						beforeId="origin-icons"
						paint={{
							'line-color': relicRingColor,
							'line-width': 2,
							'line-opacity': 0.9
						}}
					/>
				</Source.GeoJSON>
			{/if}

			<Source.GeoJSON id="dungeon-src" data={dungeonFC}>
				<Layer.Symbol
					id="dungeon-icons"
					visible={showDungeons}
					layout={{
						'icon-image': ['get', 'icon'],
						'icon-allow-overlap': true,
						'icon-size': zoomScaledIconSize(0.6, 1.0)
					}}
				/>
			</Source.GeoJSON>

			<Source.GeoJSON id="boss-src" data={bossFC}>
				<Layer.Symbol
					id="boss-icons"
					visible={showBosses}
					layout={{
						'icon-image': ['get', 'icon'],
						'icon-allow-overlap': true,
						'symbol-sort-key': ['case', ['get', 'defeated'], 2, 1],
						'icon-size': zoomScaledIconSize(0.6, 1.0),
						'text-field': showLabels ? ['step', ['zoom'], '', 5, ['get', 'name']] : '',
						'text-size': 11,
						'text-optional': true,
						'text-anchor': 'top',
						'text-offset': [0, 0.8],
						'text-max-width': 12
					}}
					paint={{
						'icon-opacity': [
							'case',
							['boolean', ['feature-state', 'hover'], false],
							1,
							['get', 'defeated'],
							0.6,
							1
						],
						'text-color': '#ffffff',
						'text-halo-color': '#000000',
						'text-halo-width': 1.5
					}}
				/>
			</Source.GeoJSON>

			<Source.GeoJSON id="bounty-src" data={bountyFC}>
				<Layer.Symbol
					id="bounty-icons"
					visible={showBounty}
					layout={{
						'icon-image': ['get', 'icon'],
						'icon-allow-overlap': true,
						'symbol-sort-key': ['case', ['get', 'defeated'], 2, 1],
						'icon-size': zoomScaledIconSize(0.5, 0.7),
						'text-field': showLabels ? ['step', ['zoom'], '', 5, ['get', 'name']] : '',
						'text-size': 11,
						'text-optional': true,
						'text-anchor': 'top',
						'text-offset': [0, 0.8],
						'text-max-width': 12
					}}
					paint={{
						'icon-opacity': [
							'case',
							['boolean', ['feature-state', 'hover'], false],
							1,
							['get', 'defeated'],
							0.6,
							1
						],
						'text-color': '#ffffff',
						'text-halo-color': '#000000',
						'text-halo-width': 1.5
					}}
				/>
			</Source.GeoJSON>

			<Source.GeoJSON id="alpha-src" data={alphaFC}>
				<Layer.Symbol
					id="alpha-icons"
					visible={showAlphaPals}
					layout={{
						'icon-image': ['get', 'icon'],
						'icon-allow-overlap': true,
						'icon-size': zoomScaledIconSize(0.6, 1.0)
					}}
				/>
			</Source.GeoJSON>

			<Source.GeoJSON id="predator-src" data={predatorFC}>
				<Layer.Symbol
					id="predator-icons"
					visible={showPredatorPals}
					layout={{
						'icon-image': ['get', 'icon'],
						'icon-allow-overlap': true,
						'icon-size': zoomScaledIconSize(0.6, 1.0)
					}}
				/>
			</Source.GeoJSON>

			{#each mapLayerRenders as render (render.id)}
				<Source.GeoJSON id="{render.id}-src" data={render.fc}>
					<Layer.Symbol
						id="{render.id}-icons"
						visible={render.visible}
						minzoom={render.minZoom}
						layout={{
							'icon-image': ['get', 'icon'],
							'icon-allow-overlap': true,
							'icon-size': render.iconSize
						}}
					/>
				</Source.GeoJSON>
			{/each}
		</ImageLoader>

		{#if hovered}
			{#if hovered?.source !== undefined && hovered?.id !== undefined}
				<FeatureState source={hovered.source} id={hovered.id} state={HOVER_STATE} />
			{/if}
		{/if}
	</MLMap>

	{#if import.meta.env.DEV && typeof window !== 'undefined' && new URLSearchParams(window.location.search).has('bench')}
		<BenchOverlay {map} {area} />
	{/if}

	{#if hovered?.point}
		{@const entry = lookup(hovered.type, hovered.key)}
		<div class="map-anchored-card" style="left: {hovered.point.x}px; top: {hovered.point.y}px;">
			<MapTooltip type={hovered.type} data={entry?.data} guildName={entry?.guildName} />
		</div>
	{/if}

	{#if selected && selectedPoint}
		{@const entry = lookup(selected.type, selected.key)}
		<div
			class="map-anchored-card map-popup-card"
			style="left: {selectedPoint.x}px; top: {selectedPoint.y}px;"
		>
			<MapPopup
				type={selected.type}
				data={entry?.data}
				guildName={entry?.guildName}
				{onExportBase}
				onDeleteBase={(base) => {
					// Closed before the delete: the base is about to stop existing in appState,
					// and `entry` would otherwise resolve to undefined on the next reactive read.
					selected = null;
					onDeleteBase?.(base);
				}}
			/>
			<button type="button" class="map-popup-close" onclick={() => (selected = null)}>×</button>
		</div>
	{/if}

	<div class="map-area-switch" class:align-right={areaSwitchAlign === 'right'}>
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

	<div class="coordinate-display">
		{@html coordDisplayText}
	</div>
</div>

<style>
	:global(.maplibregl-canvas-container) {
		background-color: #000;
	}

	.map-anchored-card {
		position: absolute;
		z-index: 1000;
		transform: translate(12px, -50%);
		max-width: 320px;
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

	.map-area-switch.align-right {
		left: auto;
		right: 52px;
		transform: none;
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
