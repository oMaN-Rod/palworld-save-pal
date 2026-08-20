import { WATCHTOWER_CLASS } from './fastTravel';

export const MAP_LAYER_GROUPS = ['general', 'locations', 'collectibles', 'poi'] as const;
export type MapLayerGroup = (typeof MAP_LAYER_GROUPS)[number];

export const MAP_LAYER_ARTIFACTS = [
	'fast_travel_points',
	'dungeons',
	'bosses',
	'relics',
	'effigies',
	'towers',
	'notes',
	'eggs_spawners',
	// chests.json was removed for weight (3 MB, and the worker fetches the whole
	// manifest at init). Still served, but no layer binds to it until artifacts load lazily.
	'chests',
	'camps',
	'ancient_ruins',
	'kinship_peach',
	'skill_fruits'
] as const;
export type MapLayerArtifact = (typeof MAP_LAYER_ARTIFACTS)[number];

export type MapLayerEntry = Record<string, unknown>;

export type RawArtifact<T = MapLayerEntry> = Record<string, T> | readonly T[];

export type MapLayerShape = 'keyed' | 'list';

export type MapLayerPoint<T = MapLayerEntry> = { key: string; entry: T };

export type MapLayerSelection<T = MapLayerEntry> = {
	shape: MapLayerShape;
	points: MapLayerPoint<T>[];
};

const LIST_KEY_FIELDS = ['instance_id', 'name'] as const;

export type MapLayerSubset =
	| { readonly kind: 'all' }
	| { readonly kind: 'equals'; readonly field: string; readonly value: string }
	| { readonly kind: 'notEquals'; readonly field: string; readonly value: string };

type MapLayerRow = {
	readonly id: string;
	readonly group: MapLayerGroup;
	readonly artifact: MapLayerArtifact;
	readonly subset: MapLayerSubset;
	readonly defaultVisible: boolean;
	readonly minZoom?: number;
};

const all: MapLayerSubset = { kind: 'all' };
const spawnType = (value: string): MapLayerSubset => ({
	kind: 'equals',
	field: 'spawn_type',
	value
});
const watchtowerClass = (kind: 'equals' | 'notEquals'): MapLayerSubset => ({
	kind,
	field: 'class',
	value: WATCHTOWER_CLASS
});

export const MAP_LAYERS = [
	{
		id: 'fast_travel',
		group: 'locations',
		artifact: 'fast_travel_points',
		subset: watchtowerClass('notEquals'),
		defaultVisible: true
	},
	{
		id: 'watchtower',
		group: 'locations',
		artifact: 'fast_travel_points',
		subset: watchtowerClass('equals'),
		defaultVisible: true
	},
	{ id: 'tower_boss', group: 'locations', artifact: 'towers', subset: all, defaultVisible: false },
	{ id: 'dungeons', group: 'locations', artifact: 'dungeons', subset: all, defaultVisible: true },
	{
		id: 'ancient_ruins',
		group: 'locations',
		artifact: 'ancient_ruins',
		subset: all,
		defaultVisible: false
	},
	{ id: 'relics', group: 'collectibles', artifact: 'relics', subset: all, defaultVisible: true },
	{
		id: 'eggs',
		group: 'collectibles',
		artifact: 'eggs_spawners',
		subset: all,
		defaultVisible: false,
		// 1816 markers against 8-64 for every other new layer. Drawn only once the
		// view is tight enough for them to be distinguishable rather than a wash.
		minZoom: 4
	},
	{ id: 'journals', group: 'collectibles', artifact: 'notes', subset: all, defaultVisible: false },
	{
		id: 'skill_fruits',
		group: 'collectibles',
		artifact: 'skill_fruits',
		subset: all,
		defaultVisible: false
	},
	{
		id: 'kinship_peach',
		group: 'collectibles',
		artifact: 'kinship_peach',
		subset: all,
		defaultVisible: false
	},
	{
		id: 'alpha_pals',
		group: 'poi',
		artifact: 'bosses',
		subset: spawnType('alpha'),
		defaultVisible: true
	},
	{
		id: 'boss_pals',
		group: 'poi',
		artifact: 'bosses',
		subset: spawnType('boss'),
		defaultVisible: true
	},
	{
		id: 'predator_pals',
		group: 'poi',
		artifact: 'bosses',
		subset: spawnType('predator'),
		defaultVisible: true
	},
	{
		id: 'bounty',
		group: 'poi',
		artifact: 'bosses',
		subset: spawnType('bounty'),
		defaultVisible: false
	},
	{ id: 'camps', group: 'poi', artifact: 'camps', subset: all, defaultVisible: false }
] as const satisfies readonly MapLayerRow[];

export type MapLayerId = (typeof MAP_LAYERS)[number]['id'];
export type MapLayerDefinition = MapLayerRow & { readonly id: MapLayerId };

const byId = new Map<string, MapLayerDefinition>(MAP_LAYERS.map((layer) => [layer.id, layer]));

export function isMapLayerId(id: string): id is MapLayerId {
	return byId.has(id);
}

export function getMapLayer(id: MapLayerId): MapLayerDefinition {
	const layer = byId.get(id);
	if (!layer) throw new Error(`Unknown map layer "${id}"`);
	return layer;
}

export function mapLayersInGroup(group: MapLayerGroup): MapLayerDefinition[] {
	return MAP_LAYERS.filter((layer) => layer.group === group);
}

// Layers Map.svelte draws through a builder of their own, because one sprite for the
// whole layer will not do: the spawn layers take a per-pal portrait, relics one sprite
// per relic_type, and fast travel and dungeons predate the registry with their own
// stores and props. Everything else draws through buildMapLayerFC.
export const BESPOKE_RENDER_LAYERS = [
	'fast_travel',
	'watchtower',
	'dungeons',
	'relics',
	'alpha_pals',
	'boss_pals',
	'predator_pals',
	'bounty'
] as const satisfies readonly MapLayerId[];

export function genericRenderLayers(): MapLayerId[] {
	const bespoke = new Set<string>(BESPOKE_RENDER_LAYERS);
	return MAP_LAYERS.map((layer) => layer.id).filter((id) => !bespoke.has(id));
}

export function artifactsForLayers(ids: readonly MapLayerId[]): MapLayerArtifact[] {
	const seen = new Set<MapLayerArtifact>();
	for (const id of ids) seen.add(getMapLayer(id).artifact);
	return [...seen];
}

function matches(subset: MapLayerSubset, entry: MapLayerEntry): boolean {
	if (subset.kind === 'all') return true;
	const actual = entry?.[subset.field];
	return subset.kind === 'equals' ? actual === subset.value : actual !== subset.value;
}

function listKey(entry: MapLayerEntry, artifact: MapLayerArtifact, index: number): string {
	for (const field of LIST_KEY_FIELDS) {
		const value = entry?.[field];
		if (typeof value === 'string' && value !== '') return value;
	}
	return `${artifact}:${index}`;
}

// Entry objects are handed back by reference -- these tables run to thousands of
// entries and copying them buys nothing.
export function selectLayerEntries<T extends MapLayerEntry>(
	id: MapLayerId,
	artifact: RawArtifact<T>
): MapLayerSelection<T> {
	const layer = getMapLayer(id);
	const points: MapLayerPoint<T>[] = [];
	if (Array.isArray(artifact)) {
		artifact.forEach((entry, index) => {
			if (matches(layer.subset, entry)) {
				points.push({ key: listKey(entry, layer.artifact, index), entry });
			}
		});
		return { shape: 'list', points };
	}
	for (const [key, entry] of Object.entries(artifact as Record<string, T>)) {
		if (matches(layer.subset, entry)) points.push({ key, entry });
	}
	return { shape: 'keyed', points };
}
