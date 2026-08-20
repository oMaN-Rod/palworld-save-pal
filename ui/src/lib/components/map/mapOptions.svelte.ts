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
	hideCollectedRelics: boolean;
	hideUnlockedFastTravel: boolean;
	relicTypes: Record<string, boolean>;
	structureTypes: Record<string, boolean>;
	showDungeons: boolean;
	showBosses: boolean;
	showAlphaPals: boolean;
	showPredatorPals: boolean;
	showBounty: boolean;
	showLabels: boolean;
	enable3d: boolean;
	structureRenderMode: 'detailed' | 'flat';
	structureTextured: boolean;
	panelOpen: boolean;
	palSize: number;
	palAutoFollow: boolean;
	palHeight: number;
	mapOpacity: number;
	fastTravelSize: number;
	watchtowerSize: number;
	relicSize: number;
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
