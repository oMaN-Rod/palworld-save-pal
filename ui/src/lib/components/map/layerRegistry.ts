import { WATCHTOWER_CLASS } from './fastTravel';

/** Legend sections, in the order the panel renders them. `general` holds the
 *  options that are not artifact-backed layers, so no layer belongs to it. */
export const MAP_LAYER_GROUPS = ['general', 'locations', 'collectibles', 'poi'] as const;
export type MapLayerGroup = (typeof MAP_LAYER_GROUPS)[number];

/** Artifact ids the backend accepts for `get_map_layer`. An id outside this set
 *  comes back as an error frame, so the table below may name nothing else. */
export const MAP_LAYER_ARTIFACTS = [
	'fast_travel_points',
	'dungeons',
	'bosses',
	'relics',
	// The capture_power subset of `relics`, and the narrower of the two files.
	// Served, but no layer binds to it — the map draws the whole relics table.
	'effigies',
	'towers',
	'notes',
	'eggs_spawners',
	// chests.json was removed for weight (3 MB, and the worker fetches the whole
	// manifest at init). Still served, but no layer binds to it until artifacts
	// load lazily.
	'chests',
	'camps'
] as const;
export type MapLayerArtifact = (typeof MAP_LAYER_ARTIFACTS)[number];

export type MapLayerEntry = Record<string, unknown>;

/**
 * An artifact exactly as it arrives. Some are keyed objects (`towers`, `notes`,
 * `dungeons`, `bosses`, `fast_travel_points`, `relics`) and some are top-level
 * arrays (`eggs_spawners`, `camps`). Nothing on the wire normalises the two, and
 * the l10n merge skips arrays entirely because there is no key to target.
 */
export type RawArtifact<T = MapLayerEntry> = Record<string, T> | readonly T[];

export type MapLayerShape = 'keyed' | 'list';

/**
 * An entry paired with the id it is known by. For a keyed artifact that is the
 * object key, which carries meaning (`Day-xx`, a boss battle name). An array
 * artifact has no such key, so identity comes off the entry — never the array
 * index, which is positional rather than stable.
 */
export type MapLayerPoint<T = MapLayerEntry> = { key: string; entry: T };

export type MapLayerSelection<T = MapLayerEntry> = {
	shape: MapLayerShape;
	points: MapLayerPoint<T>[];
};

/** Identity fields an array entry may carry, most specific first. */
const LIST_KEY_FIELDS = ['instance_id', 'name'] as const;

/**
 * How a layer narrows its artifact. Several layers share one artifact and one
 * request, then split it here: `fast_travel_points` holds both fast travel
 * points and watchtowers, told apart by `class`; `bosses` holds alpha, boss and
 * predator spawns in one table, told apart by `spawn_type`.
 */
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
	/** Zoom below which the layer is not drawn. Dense layers only. */
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

/** The distinct artifacts backing `ids`, in first-seen order — the batch to ask
 *  the backend for. Layers sharing an artifact collapse to one fetch. */
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

/**
 * The entries of `artifact` belonging to `id`, each with a stable key. Entry
 * objects are handed back by reference — these tables run to thousands of
 * entries and copying them buys nothing.
 */
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
