import * as m from '$i18n/messages';
import compass from '$lib/assets/img/compass.webp';
import { c } from '$lib/utils/commonTranslations';
import { assetLoader } from '$utils';
import {
	MAP_LAYERS,
	MAP_LAYER_GROUPS,
	getMapLayer,
	mapLayersInGroup,
	type MapLayerGroup,
	type MapLayerId
} from './layerRegistry';
import { mapImg } from './styles';

// Not artifact-backed layers: they read app state rather than a `get_map_layer`
// artifact, so the registry's `artifact` field would have to lie if they were rows.
export const PANEL_EXTRAS = [
	{ id: 'origin', group: 'general', defaultVisible: false },
	{ id: 'players', group: 'general', defaultVisible: true },
	{ id: 'bases', group: 'general', defaultVisible: true },
	{ id: 'labels', group: 'general', defaultVisible: true }
] as const satisfies readonly {
	readonly id: string;
	readonly group: MapLayerGroup;
	readonly defaultVisible: boolean;
}[];

export type PanelExtraId = (typeof PANEL_EXTRAS)[number]['id'];
export type PanelOptionId = MapLayerId | PanelExtraId;

const LABELS: Record<MapLayerId, () => string> = {
	fast_travel: m.fast_travel,
	watchtower: m.watchtower,
	tower_boss: m.tower_boss,
	dungeons: m.dungeons,
	relics: m.relics,
	eggs: m.eggs,
	journals: m.journals,
	skill_fruits: m.skill_fruits,
	kinship_peach: m.kinship_peach,
	ancient_ruins: m.ancient_ruins,
	alpha_pals: () => c.alphaPals,
	boss_pals: m.bosses,
	predator_pals: () => c.predatorPals,
	bounty: m.bounty,
	camps: m.camps
};

const EXTRA_LABELS: Record<PanelExtraId, () => string> = {
	origin: m.origin,
	players: () => c.players,
	bases: () => c.bases,
	labels: m.map_labels
};

const GROUP_LABELS: Record<MapLayerGroup, () => string> = {
	general: m.general,
	locations: m.locations,
	collectibles: m.collectibles,
	poi: m.poi
};

const ICONS: Record<MapLayerId, () => string> = {
	fast_travel: () => mapImg.fastTravel,
	watchtower: () => mapImg.watchTower,
	tower_boss: () => mapImg.tower,
	dungeons: () => mapImg.dungeon,
	relics: () => mapImg.effigy,
	eggs: () => mapImg.egg,
	journals: () => mapImg.journal,
	skill_fruits: () => mapImg.fruit,
	kinship_peach: () => mapImg.kinshipPeach,
	ancient_ruins: () => mapImg.ancientRuin,
	alpha_pals: () => assetLoader.loadMenuImage('anubis'),
	boss_pals: () => mapImg.boss,
	predator_pals: () => assetLoader.loadMenuImage('nightbluehorse'),
	bounty: () => mapImg.bounty,
	camps: () => mapImg.camp
};

const EXTRA_ICONS: Record<PanelExtraId, () => string> = {
	origin: () => compass,
	players: () => mapImg.player,
	bases: () => mapImg.baseCamp,
	labels: () => mapImg.signboard
};

const isExtra = (id: PanelOptionId): id is PanelExtraId => id in EXTRA_LABELS;

export function mapLayerLabel(id: MapLayerId): string {
	return LABELS[id]();
}

export function panelOptionLabel(id: PanelOptionId): string {
	return isExtra(id) ? EXTRA_LABELS[id]() : LABELS[id]();
}

export function panelIcon(id: PanelOptionId): string {
	return isExtra(id) ? EXTRA_ICONS[id]() : ICONS[id]();
}

export function mapLayerGroupLabel(group: MapLayerGroup): string {
	return GROUP_LABELS[group]();
}

export const showAllLabel = m.show_all;
export const hideAllLabel = m.hide_all;
export const loadingLabel = m.loading;

export type MapLayerRowModel = {
	id: PanelOptionId;
	label: string;
	icon: string;
	visible: boolean;
	count: string | undefined;
	loading: boolean;
};

export type MapLayerGroupModel = {
	group: MapLayerGroup;
	label: string;
	rows: MapLayerRowModel[];
	allVisible: boolean;
	noneVisible: boolean;
};

export type MapLayerVisibility = Partial<Record<PanelOptionId, boolean>>;

export type PanelProbes = {
	count?: (id: PanelOptionId) => string | undefined;
	loading?: (id: PanelOptionId) => boolean;
	// False drops the row -- Players and Bases need a save loaded.
	available?: (id: PanelOptionId) => boolean;
};

function defaultVisible(id: PanelOptionId): boolean {
	if (isExtra(id)) return PANEL_EXTRAS.find((extra) => extra.id === id)!.defaultVisible;
	return getMapLayer(id).defaultVisible;
}

export function defaultLayerVisibility(): Record<MapLayerId, boolean> {
	const visibility = {} as Record<MapLayerId, boolean>;
	for (const layer of MAP_LAYERS) visibility[layer.id] = layer.defaultVisible;
	return visibility;
}

export function defaultPanelVisibility(): Record<PanelOptionId, boolean> {
	const visibility = defaultLayerVisibility() as Record<PanelOptionId, boolean>;
	for (const extra of PANEL_EXTRAS) visibility[extra.id] = extra.defaultVisible;
	return visibility;
}

function optionsInGroup(group: MapLayerGroup): PanelOptionId[] {
	return [
		...PANEL_EXTRAS.filter((extra) => extra.group === group).map((extra) => extra.id),
		...mapLayersInGroup(group).map((layer) => layer.id)
	];
}

export function buildPanelGroups(
	layers: MapLayerVisibility,
	probes: PanelProbes = {}
): MapLayerGroupModel[] {
	const { count, loading, available } = probes;
	const groups: MapLayerGroupModel[] = [];
	for (const group of MAP_LAYER_GROUPS) {
		const rows = optionsInGroup(group)
			.filter((id) => available?.(id) ?? true)
			.map((id) => ({
				id,
				label: panelOptionLabel(id),
				icon: panelIcon(id),
				visible: layers[id] ?? defaultVisible(id),
				count: count?.(id),
				loading: loading?.(id) ?? false
			}));
		if (rows.length === 0) continue;
		groups.push({
			group,
			label: mapLayerGroupLabel(group),
			rows,
			allVisible: rows.every((row) => row.visible),
			noneVisible: rows.every((row) => !row.visible)
		});
	}
	return groups;
}

export function groupVisibilityPatch(group: MapLayerGroup, visible: boolean): MapLayerVisibility {
	const patch: MapLayerVisibility = {};
	for (const id of optionsInGroup(group)) patch[id] = visible;
	return patch;
}

export function allVisibilityPatch(visible: boolean): MapLayerVisibility {
	const patch: MapLayerVisibility = {};
	for (const group of MAP_LAYER_GROUPS) {
		for (const id of optionsInGroup(group)) patch[id] = visible;
	}
	return patch;
}
