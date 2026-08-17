import { describe, expect, it } from 'vitest';
import { WATCHTOWER_CLASS } from './fastTravel';
import {
	BESPOKE_RENDER_LAYERS,
	MAP_LAYERS,
	MAP_LAYER_GROUPS,
	artifactsForLayers,
	genericRenderLayers,
	getMapLayer,
	mapLayersInGroup,
	selectLayerEntries
} from './layerRegistry';

// The wire contract accepts exactly these artifact ids; an unknown one is an
// error frame, so a typo in the table is a runtime failure with no local signal.
const WIRE_ARTIFACTS = [
	'fast_travel_points',
	'dungeons',
	'bosses',
	'relics',
	'effigies',
	'towers',
	'notes',
	'eggs_spawners',
	'chests',
	'camps',
	'ancient_ruins',
	'kinship_peach',
	'skill_fruits'
];

describe('MAP_LAYERS', () => {
	it('registers every layer whose artifact exists, and no others', () => {
		expect(MAP_LAYERS.map((layer) => layer.id)).toEqual([
			'fast_travel',
			'watchtower',
			'tower_boss',
			'dungeons',
			'ancient_ruins',
			'relics',
			'eggs',
			'journals',
			'skill_fruits',
			'kinship_peach',
			'alpha_pals',
			'boss_pals',
			'predator_pals',
			'bounty',
			'camps'
		]);
	});

	it('names only artifacts the wire contract accepts', () => {
		for (const layer of MAP_LAYERS) {
			expect(WIRE_ARTIFACTS).toContain(layer.artifact);
		}
	});

	// effigies.json is the capture_power subset of relics.json, and chests.json
	// was removed for weight. Both are still served, so they stay in the artifact
	// allowlist, but no layer may bind to either.
	it('binds no layer to the artifacts that exist but are not drawn', () => {
		const bound = MAP_LAYERS.map((layer) => layer.artifact);
		expect(bound).not.toContain('effigies');
		expect(bound).not.toContain('chests');
		expect(getMapLayer('relics').artifact).toBe('relics');
	});

	it('draws the journals layer from notes.json', () => {
		expect(getMapLayer('journals').artifact).toBe('notes');
	});

	it('places every layer in one of the legend groups', () => {
		for (const layer of MAP_LAYERS) {
			expect(MAP_LAYER_GROUPS).toContain(layer.group);
			expect(typeof layer.defaultVisible).toBe('boolean');
		}
	});

	it('has no duplicate ids', () => {
		const ids = MAP_LAYERS.map((layer) => layer.id);
		expect(new Set(ids).size).toBe(ids.length);
	});

	// Anything defaulting to visible is fetched at page load. Only the layers that
	// already shipped that way may do so; every layer added since is opt-in.
	it('defaults every newly added layer to hidden', () => {
		const shipped = ['fast_travel', 'watchtower', 'dungeons', 'relics'];
		const spawns = ['alpha_pals', 'boss_pals', 'predator_pals'];
		for (const layer of MAP_LAYERS) {
			const expected = shipped.includes(layer.id) || spawns.includes(layer.id);
			expect([layer.id, layer.defaultVisible]).toEqual([layer.id, expected]);
		}
	});

	it('gives the dense layers a zoom floor and leaves the sparse ones unrestricted', () => {
		expect(getMapLayer('eggs').minZoom).toBeGreaterThan(0);
		expect(getMapLayer('tower_boss').minZoom).toBeUndefined();
		expect(getMapLayer('camps').minZoom).toBeUndefined();
	});
});

// Map.svelte draws a layer one of two ways, and the split used to live as a
// hardcoded list inside that component: a layer added to the registry but not to
// that list simply never drew, with nothing failing.
describe('render partition', () => {
	it('assigns every layer to exactly one render path', () => {
		const both = BESPOKE_RENDER_LAYERS.filter((id) => genericRenderLayers().includes(id));
		expect(both).toEqual([]);
		expect([...BESPOKE_RENDER_LAYERS, ...genericRenderLayers()].sort()).toEqual(
			MAP_LAYERS.map((layer) => layer.id).sort()
		);
	});

	it('routes a newly registered layer to the generic renderer by default', () => {
		// The safe default: a new layer draws with its own icon rather than not at
		// all, and opting into a bespoke path is the deliberate act.
		for (const id of ['skill_fruits', 'kinship_peach', 'ancient_ruins'] as const) {
			expect(genericRenderLayers()).toContain(id);
		}
	});
});

describe('getMapLayer', () => {
	it('returns the definition for a known id', () => {
		expect(getMapLayer('dungeons').artifact).toBe('dungeons');
	});

	it('throws on an unknown id rather than returning undefined', () => {
		expect(() => getMapLayer('ore_nodes' as never)).toThrow(/ore_nodes/);
	});
});

describe('mapLayersInGroup', () => {
	it('lists layers in registration order', () => {
		expect(mapLayersInGroup('locations').map((layer) => layer.id)).toEqual([
			'fast_travel',
			'watchtower',
			'tower_boss',
			'dungeons',
			'ancient_ruins'
		]);
	});
});

describe('artifactsForLayers', () => {
	it('collapses layers that share one artifact into a single fetch', () => {
		expect(artifactsForLayers(['fast_travel', 'watchtower'])).toEqual(['fast_travel_points']);
		expect(artifactsForLayers(['alpha_pals', 'boss_pals', 'predator_pals'])).toEqual(['bosses']);
	});

	it('preserves first-seen order across distinct artifacts', () => {
		expect(artifactsForLayers(['dungeons', 'watchtower', 'fast_travel', 'alpha_pals'])).toEqual([
			'dungeons',
			'fast_travel_points',
			'bosses'
		]);
	});
});

// Some artifacts arrive as keyed objects (towers, notes, dungeons, bosses,
// fast_travel_points, relics) and some as top-level arrays (eggs_spawners,
// camps). Nothing on the wire normalises them.
describe('selectLayerEntries: keyed artifacts', () => {
	const towers = {
		BOSS_BATTLE_NAME_DesertBoss: { class: 'BP_PalBossTower_C', x: 1, y: 2 },
		BOSS_BATTLE_NAME_IceBoss: { class: 'BP_PalBossTower_C', x: 3, y: 4 }
	};

	it('reports the keyed shape', () => {
		expect(selectLayerEntries('tower_boss', towers).shape).toBe('keyed');
	});

	it('uses the object key as identity, in insertion order', () => {
		const { points } = selectLayerEntries('tower_boss', towers);
		expect(points.map((p) => p.key)).toEqual([
			'BOSS_BATTLE_NAME_DesertBoss',
			'BOSS_BATTLE_NAME_IceBoss'
		]);
	});

	it('hands back the exact entry objects, not copies', () => {
		const { points } = selectLayerEntries('tower_boss', towers);
		expect(points[0].entry).toBe(towers.BOSS_BATTLE_NAME_DesertBoss);
	});
});

describe('selectLayerEntries: array artifacts', () => {
	const camps = [
		{ class: 'A', name: 'camp_a', instance_id: 'IID_A', x: 1, y: 2 },
		{ class: 'B', name: 'camp_b', x: 3, y: 4 }
	];

	it('reports the list shape', () => {
		expect(selectLayerEntries('camps', camps).shape).toBe('list');
	});

	it('takes identity off the entry, never the array index', () => {
		const { points } = selectLayerEntries('camps', camps);
		expect(points.map((p) => p.key)).toEqual(['IID_A', 'camp_b']);
	});

	it('falls back to a positional key only when the entry carries no identity', () => {
		const { points } = selectLayerEntries('camps', [{ x: 1, y: 2 }]);
		expect(points[0].key).toContain('camps');
		expect(points).toHaveLength(1);
	});

	it('keeps every entry of a large array', () => {
		const eggs = Array.from({ length: 50 }, (_, i) => ({ instance_id: `E${i}`, x: i, y: i }));
		expect(selectLayerEntries('eggs', eggs).points).toHaveLength(50);
	});
});

// fast_travel_points.json carries fast travel points AND watchtowers, told apart
// by `class`. Two layers, one artifact, one request.
describe('subset by class: fast travel vs watchtower', () => {
	const artifact = {
		a: { class: 'BP_LevelObject_TowerFastTravelPoint_C', x: 1 },
		b: { class: WATCHTOWER_CLASS, x: 2 },
		c: { class: 'BP_LevelObject_TowerFastTravelPoint_C', x: 3 }
	};

	it('backs both layers with the same artifact', () => {
		expect(getMapLayer('fast_travel').artifact).toBe('fast_travel_points');
		expect(getMapLayer('watchtower').artifact).toBe('fast_travel_points');
	});

	it('selects only watchtowers for the watchtower layer', () => {
		expect(selectLayerEntries('watchtower', artifact).points.map((p) => p.key)).toEqual(['b']);
	});

	it('selects everything that is not a watchtower for the fast travel layer', () => {
		expect(selectLayerEntries('fast_travel', artifact).points.map((p) => p.key)).toEqual([
			'a',
			'c'
		]);
	});

	it('partitions the artifact with no entry lost or shared', () => {
		const ft = selectLayerEntries('fast_travel', artifact).points.map((p) => p.key);
		const wt = selectLayerEntries('watchtower', artifact).points.map((p) => p.key);
		expect([...ft, ...wt].sort()).toEqual(Object.keys(artifact).sort());
	});
});

// bosses.json carries alpha, boss, predator and bounty spawns in one table, told
// apart by `spawn_type`, and the UI toggles the four separately.
describe('subset by spawn_type: alpha / boss / predator / bounty', () => {
	const artifact = {
		a: { spawn_type: 'alpha' },
		b: { spawn_type: 'boss' },
		c: { spawn_type: 'predator' },
		d: { spawn_type: 'alpha' },
		e: { spawn_type: 'bounty' }
	};

	it('backs all four layers with the bosses artifact', () => {
		for (const id of ['alpha_pals', 'boss_pals', 'predator_pals', 'bounty'] as const) {
			expect(getMapLayer(id).artifact).toBe('bosses');
		}
	});

	it('selects only its own spawn_type', () => {
		expect(selectLayerEntries('alpha_pals', artifact).points.map((p) => p.key)).toEqual(['a', 'd']);
		expect(selectLayerEntries('boss_pals', artifact).points.map((p) => p.key)).toEqual(['b']);
		expect(selectLayerEntries('predator_pals', artifact).points.map((p) => p.key)).toEqual(['c']);
		expect(selectLayerEntries('bounty', artifact).points.map((p) => p.key)).toEqual(['e']);
	});

	it('keeps bounty targets out of the boss layer', () => {
		// Both are character_id "None" humans; only spawn_type separates them, and
		// bounty rows shipped as spawn_type "boss" until the parser split them.
		expect(selectLayerEntries('boss_pals', artifact).points.map((p) => p.key)).not.toContain('e');
	});

	it('partitions the artifact with no entry lost or shared', () => {
		const selected = (['alpha_pals', 'boss_pals', 'predator_pals', 'bounty'] as const).flatMap(
			(id) => selectLayerEntries(id, artifact).points.map((p) => p.key)
		);
		expect(selected.sort()).toEqual(Object.keys(artifact).sort());
	});

	it('drops an entry whose spawn_type is unrecognised instead of defaulting it', () => {
		const withJunk = { ...artifact, z: { spawn_type: 'unknown_future_type' } };
		const selected = (['alpha_pals', 'boss_pals', 'predator_pals', 'bounty'] as const).flatMap(
			(id) => selectLayerEntries(id, withJunk).points.map((p) => p.key)
		);
		expect(selected).not.toContain('z');
	});
});

describe('layers that own their whole artifact', () => {
	it('passes every entry through', () => {
		const artifact = { a: { x: 1 }, b: { x: 2 } };
		const { points } = selectLayerEntries('dungeons', artifact);
		expect(points.map((p) => p.key)).toEqual(['a', 'b']);
		expect(points.map((p) => p.entry)).toEqual([artifact.a, artifact.b]);
	});
});
