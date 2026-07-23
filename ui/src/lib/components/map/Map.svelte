<script module lang="ts">
	const HOVER_STATE = { hover: true };
	const STRUCTURE_MIN_ZOOM = 5;
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
		DEFAULT_MAP_AREA,
		MAP_AREA_ORDER,
		type MapArea
	} from './utils';
	import { MAP_MAX_BOUNDS, lngLatToPixel, pixelToLngLat, verticalScaleFactor } from './mercator';
	import { zoomScaledIconSize } from './expressions';
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
	import { palIconId } from './iconIds';
	import { relicsByType } from './relics';
	import { isWatchtower } from './fastTravel';
	import { mapImg, STRUCTURE_COLORS } from './styles';
	import {
		mapObjects,
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
	import Toggle3dControl from './Toggle3dControl.svelte';
	import ToggleDetailedControl from './ToggleDetailedControl.svelte';
	import StructureFilterControl from './StructureFilterControl.svelte';
	import { createStructureLayer, type StructureLayer } from './structureLayer';
	import type { BaseStructure, MapUnlockPoint, RelicPoint } from '$types';
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
		showLabels = true,
		show3d = false,
		structureTypes = {},
		renderMode = 'detailed',
		onToggle3d,
		onToggleStructureType,
		onToggleRenderMode,
		onEditBase,
		onExportBase,
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
		showLabels?: boolean;
		show3d?: boolean;
		/** Per-structure-type visibility; a missing key means visible. */
		structureTypes?: Record<string, boolean>;
		renderMode?: 'detailed' | 'flat';
		onToggle3d?: () => void;
		onToggleStructureType?: (type: string) => void;
		onToggleRenderMode?: () => void;
		onEditBase?: (base: any) => void;
		onExportBase?: (base: any) => void;
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
			.map(([rowKey, boss]) => {
				const palKey = bossPalKey(boss.character_id);
				const palData = palKey ? palsData.getByKey(palKey) : undefined;
				return {
					...boss,
					rowKey,
					defeated: defeated.has(boss.spawner_id),
					localized_name: palData?.localized_name || humanizeSpawnerId(boss.spawner_id)
				};
			})
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

	let coordDisplayText = $state('World: 0, 0<br>Map: 0, 0<br>Zoom: 0');
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

	// The popup stays pinned to its feature across pan/zoom, so its screen position has
	// to be reprojected on every map move rather than captured once at click time.
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

	// Coalesced to one pick per frame: the pass re-renders the scene, so a pick per
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
					// A MapLibre miss plus a pick miss means nothing is under the cursor,
					// regardless of what was hovered before (structure, pal, base icon, ...).
					hovered = null;
				}
				const canvas = map?.getCanvas();
				if (canvas) canvas.style.cursor = key ? 'pointer' : '';
			});
		});
	}

	function handleMouseMove(ev: maplibregl.MapMouseEvent) {
		const [px, py] = lngLatToPixel(ev.lngLat.lng, ev.lngLat.lat);
		const { worldX, worldY } = pixelToWorld(px, py, area);
		const { gameX, gameY } = pixelToGameCoords(px, py, area);
		coordDisplayText = `World: ${Math.round(worldX)}, ${Math.round(worldY)}<br>Map: ${gameX}, ${gameY}<br>Zoom: ${zoom.toFixed(2)}`;

		const top = topFeatureAt(ev, INTERACTIVE_LAYERS);
		if (top) {
			// A pick resolves a frame later, so one already in flight would otherwise
			// land after this hit and stomp it; bumping the sequence discards it.
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
		const canvas = map?.getCanvas();
		if (canvas) canvas.style.cursor = '';
	}

	function handleClick(ev: maplibregl.MapMouseEvent) {
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

	$effect(() => {
		center = areaCenter(area);
	});

	const verticalScale = $derived(verticalScaleFactor(center[1], cmPerPx(area)));

	$effect(() => {
		if (!show3d || zoom < STRUCTURE_MIN_ZOOM) return;
		const instance = map;
		if (!instance) return;
		void center;
		const bounds = instance.getBounds();
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
		structureLayer.update(all, baseStructuresData.footprints, area, verticalScale);
	}

	$effect(() => {
		const instance = map;
		if (!instance) return;
		if (detailed && !structureLayer) {
			// MapLibre throws if a layer is added before the style finishes loading; only
			// keep the reference once addLayer actually succeeds, retrying on `styledata`.
			const add = () => {
				if (!instance.isStyleLoaded()) return false;
				const layer = createStructureLayer({ id: 'structure-3d' });
				instance.addLayer(layer, instance.getLayer('origin-icons') ? 'origin-icons' : undefined);
				structureLayer = layer;
				populateStructureLayer();
				return true;
			};
			// `styledata` fires many times while the style/tiles load and often fires
			// before isStyleLoaded() flips true, so a one-shot retry can miss every
			// firing; keep listening until add() succeeds or 3D is toggled off.
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

	$effect(() => {
		// Read every reactive dependency before any early return: Svelte only tracks
		// sources a run actually reads, and bailing out immediately would leave this
		// effect tracking only `detailed`, silently missing later structure data.
		const isDetailed = detailed;
		const baseList = bases;
		const footprints = baseStructuresData.footprints;
		const types = structureTypes;
		const currentArea = area;
		const vScale = verticalScale;
		let total = 0;
		for (const { base } of baseList) total += baseStructuresData.for(base.id).length;
		void footprints;
		void types;
		void currentArea;
		void vScale;
		void total;
		if (!isDetailed || !structureLayer) return;
		populateStructureLayer();
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
		instance.setMaxPitch(show3d ? 60 : 0);
		if (show3d) {
			instance.dragRotate.enable();
		} else {
			instance.dragRotate.disable();
			// setBearing delegates to jumpTo, which fires `move` even when nothing
			// changes, so calling it unconditionally manufactures map events.
			if (instance.getBearing() !== 0) instance.setBearing(0);
		}
	});

	const structureColor: ExpressionSpecification = [
		'case',
		['boolean', ['feature-state', 'hover'], false],
		'#ffffff',
		[
			'match',
			['get', 'typeA'],
			...(Object.entries(STRUCTURE_COLORS).flat() as [string, string, ...string[]]),
			STRUCTURE_COLORS.Other
		]
	];

	const structureTypeList = Object.keys(STRUCTURE_COLORS);
	let structureFilterOpen = $state(false);

	const structureFilter = $derived.by<FilterSpecification | undefined>(() => {
		const hidden = Object.entries(structureTypes)
			.filter(([, visible]) => visible === false)
			.map(([type]) => type);
		if (hidden.length === 0) return undefined;
		return ['!', ['in', ['get', 'typeA'], ['literal', hidden]]];
	});

	// A selected/hovered structure can stop being rendered three ways - 3D toggled off,
	// its type filtered out, or the underlying data reset by a save load - so both refs
	// are re-validated together whenever any of those can happen.
	$effect(() => {
		if (!show3d) {
			structureFilterOpen = false;
			if (selected?.type === 'structure') selected = null;
			if (hovered?.type === 'structure') hovered = null;
			return;
		}
		const structureType = (key: string) => {
			const data = byKey.get(`structure:${key}`)?.data;
			if (!data) return undefined;
			return (
				lookupFootprint(baseStructuresData.footprints, data.map_object_id)?.typeA ?? 'Other'
			);
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

</script>

<div class="relative h-full w-full">
	<MLMap
		bind:map
		class="h-full w-full"
		style={EMPTY_STYLE}
		bind:center
		bind:zoom
		minZoom={0}
		maxZoom={7}
		maxBounds={MAP_MAX_BOUNDS}
		renderWorldCopies={false}
		dragRotate={show3d}
		pitchWithRotate={true}
		touchZoomRotate={show3d}
		attributionControl={false}
		onmove={() => moveTick++}
		onmousemove={handleMouseMove}
		onmouseout={handleMouseOut}
		onclick={handleClick}
		oncontextmenu={handleContextMenu}
	>
		<Control.Navigation position="top-right" showCompass={false} />
		<Control.Fullscreen position="top-right" />
		<Toggle3dControl
			position="top-right"
			active={show3d}
			title="3D {m.structures()}"
			onchange={onToggle3d}
		/>
		{#if show3d}
			<ToggleDetailedControl
				position="top-right"
				active={renderMode === 'detailed'}
				title="Detailed {m.structures()}"
				onchange={onToggleRenderMode}
			/>
			<StructureFilterControl
				position="top-right"
				types={structureTypeList}
				enabled={structureTypes}
				open={structureFilterOpen}
				onToggleOpen={() => (structureFilterOpen = !structureFilterOpen)}
				ontoggle={(type) => onToggleStructureType?.(type)}
				title={m.structure_types()}
			/>
		{/if}

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

			<!-- Each area is baked against its own world extent, and both share one lng/lat
			     tile namespace, so exactly one DEM may be mounted. #key forces a full
			     teardown/rebuild on area change rather than mutating a live source. -->
			{#if show3d}
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
						<Terrain source="dem-{MAP_TILE_DIR[area]}" exaggeration={verticalScale} />
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
						'icon-size': ['interpolate', ['linear'], ['zoom'], 2, 0.6, 7, 1.0]
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
		</ImageLoader>

		{#if hovered}
			{#if hovered?.source !== undefined && hovered?.id !== undefined}
				<FeatureState source={hovered.source} id={hovered.id} state={HOVER_STATE} />
			{/if}
		{/if}
	</MLMap>

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
			/>
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

	/* left/top are set inline from map.project(); the translate reproduces OpenLayers'
	   center-left positioning with its [10, 0] offset. */
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

	.map-actions {
		position: absolute;
		bottom: 72px;
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
