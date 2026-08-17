import { persistedState } from 'svelte-persisted-state';
import type { MapLayerVisibility } from './layerPanelModel';
import { STRUCTURE_TYPE_ORDER } from './mapColors.svelte';
import { MAP_OBJECT_SCALE_DEFAULT, MAP_OBJECT_WATCHTOWER_SCALE_DEFAULT } from './mapObjectSize';
import { PAL_SCALE_DEFAULT } from './palSize';
import { DEFAULT_MAP_AREA, type MapArea } from './utils';

export type MapOptions = {
	area: MapArea;
	showOrigin: boolean;
	showPlayers: boolean;
	showBases: boolean;
	showFastTravel: boolean;
	showWatchtower: boolean;
	showRelics: boolean;
	/** Hide relics the selected player has already collected. */
	hideCollectedRelics: boolean;
	/** Hide fast travel points and watchtowers the selected player has already unlocked. */
	hideUnlockedFastTravel: boolean;
	/** Per-relic-type visibility; a missing key means visible. */
	relicTypes: Record<string, boolean>;
	/** Per-structure-type visibility; a missing key means visible. */
	structureTypes: Record<string, boolean>;
	showDungeons: boolean;
	showBosses: boolean;
	showAlphaPals: boolean;
	showPredatorPals: boolean;
	showBounty: boolean;
	showLabels: boolean;
	enable3d: boolean;
	structureRenderMode: 'detailed' | 'flat';
	/** Renders detailed structures with their glb's own texture instead of the
	 *  per-type flat colour. */
	structureTextured: boolean;
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
	/** Visibility for the registry-driven layers, keyed by layer id. */
	mapLayerVisibility: MapLayerVisibility;
};

// A key of its own, never written by an earlier build: persistedState does not
// merge new defaults into a stored object, so a shared key would hand returning
// users a half-populated blob.
export const mapOptionsState = persistedState<MapOptions>('psp-map-options', {
	area: DEFAULT_MAP_AREA,
	showOrigin: false,
	showPlayers: true,
	showBases: true,
	showFastTravel: true,
	showWatchtower: true,
	showRelics: true,
	hideCollectedRelics: false,
	hideUnlockedFastTravel: false,
	relicTypes: {},
	structureTypes: Object.fromEntries(STRUCTURE_TYPE_ORDER.map((key) => [key, true])),
	showDungeons: true,
	showBosses: true,
	showAlphaPals: true,
	showPredatorPals: true,
	showBounty: false,
	showLabels: true,
	enable3d: false,
	structureRenderMode: 'detailed',
	structureTextured: false,
	panelOpen: true,
	palSize: PAL_SCALE_DEFAULT,
	palAutoFollow: true,
	palHeight: 0,
	mapOpacity: 1,
	fastTravelSize: MAP_OBJECT_SCALE_DEFAULT,
	watchtowerSize: MAP_OBJECT_WATCHTOWER_SCALE_DEFAULT,
	relicSize: MAP_OBJECT_SCALE_DEFAULT,
	mapLayerVisibility: {}
});
