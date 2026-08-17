import { describe, expect, it } from 'vitest';
import { ICON_DUNGEON } from './iconIds';
import { mapLayerLabel } from './layerPanelModel';
import { MAP_LAYERS, type MapLayerId, type MapLayerSelection } from './layerRegistry';
import {
	buildMapLayerFC,
	mapLayerIcon,
	mapLayerIconScale,
	mapLayerMarkerCount
} from './mapLayerFeatures';

const keyed = (entries: Record<string, unknown>): MapLayerSelection => ({
	shape: 'keyed',
	points: Object.entries(entries).map(([key, entry]) => ({
		key,
		entry: entry as Record<string, unknown>
	}))
});

const list = (entries: unknown[]): MapLayerSelection => ({
	shape: 'list',
	points: entries.map((entry, i) => ({ key: `k${i}`, entry: entry as Record<string, unknown> }))
});

// A point somewhere inside MainMap.
const AT = { x: -100000, y: 100000, z: 500 };

describe('buildMapLayerFC', () => {
	it('places an entry with real coordinates', () => {
		const fc = buildMapLayerFC('tower_boss', keyed({ DesertBoss: AT }), 'MainMap');
		expect(fc.features).toHaveLength(1);
		const [lng, lat] = fc.features[0].geometry.coordinates;
		expect(Number.isFinite(lng)).toBe(true);
		expect(Number.isFinite(lat)).toBe(true);
	});

	it('returns an empty collection when the layer has not loaded', () => {
		expect(buildMapLayerFC('tower_boss', undefined, 'MainMap').features).toEqual([]);
	});

	it('namespaces the feature key by layer so two layers cannot collide', () => {
		const fc = buildMapLayerFC('tower_boss', keyed({ DesertBoss: AT }), 'MainMap');
		expect(fc.features[0].properties.key).toBe('tower_boss:DesertBoss');
		expect(fc.features[0].properties.layer).toBe('tower_boss');
	});

	it('gives every feature the layer icon', () => {
		const fc = buildMapLayerFC('camps', list([AT]), 'MainMap');
		expect(fc.features[0].properties.icon).toBe(mapLayerIcon('camps'));
	});

	it('skips entries belonging to another map area', () => {
		const treePoint = { x: 500000, y: -600000, z: 0 };
		const fc = buildMapLayerFC('camps', list([AT, treePoint]), 'MainMap');
		expect(fc.features).toHaveLength(1);
	});
});

// These artifacts are hand-maintained extracts; camps already ships 8 of 59
// entries missing instance_id, and a null coordinate would otherwise land a
// marker at the map origin or produce NaN, which MapLibre renders as a silent
// hole rather than an error.
describe('null and non-numeric coordinates', () => {
	it('drops an entry whose x is null', () => {
		const fc = buildMapLayerFC('camps', list([{ x: null, y: 100000, z: 0 }]), 'MainMap');
		expect(fc.features).toEqual([]);
	});

	it('drops an entry whose y is null', () => {
		const fc = buildMapLayerFC('camps', list([{ x: -100000, y: null, z: 0 }]), 'MainMap');
		expect(fc.features).toEqual([]);
	});

	it('drops an entry missing coordinates entirely', () => {
		const fc = buildMapLayerFC('camps', list([{ class: 'BP_Camp_C' }]), 'MainMap');
		expect(fc.features).toEqual([]);
	});

	it('drops NaN and non-numeric coordinates', () => {
		const fc = buildMapLayerFC(
			'camps',
			list([
				{ x: NaN, y: 1, z: 0 },
				{ x: '−100000', y: 100000, z: 0 },
				{ x: Infinity, y: 100000, z: 0 }
			]),
			'MainMap'
		);
		expect(fc.features).toEqual([]);
	});

	it('keeps the good entries either side of a bad one', () => {
		const fc = buildMapLayerFC(
			'camps',
			list([AT, { x: null, y: null, z: 0 }, { ...AT, y: 110000 }]),
			'MainMap'
		);
		expect(fc.features).toHaveLength(2);
	});

	it('renumbers feature ids contiguously after dropping', () => {
		const fc = buildMapLayerFC(
			'camps',
			list([{ x: null, y: null, z: 0 }, AT, { ...AT, y: 110000 }]),
			'MainMap'
		);
		expect(fc.features.map((f) => f.id)).toEqual([0, 1]);
	});
});

// get_map_layer folds localized_name in only where an l10n table exists, so the
// field is present on towers and absent on notes, camps and eggs_spawners.
describe('display name', () => {
	it('uses localized_name when the l10n merge provided one', () => {
		const fc = buildMapLayerFC(
			'tower_boss',
			keyed({ BOSS_BATTLE_NAME_DesertBoss: { ...AT, localized_name: 'Zoe & Grizzbolt' } }),
			'MainMap'
		);
		expect(fc.features[0].properties.name).toBe('Zoe & Grizzbolt');
	});

	it('falls back to the object key for a keyed artifact, which carries meaning', () => {
		const fc = buildMapLayerFC('tower_boss', keyed({ BOSS_BATTLE_NAME_DesertBoss: AT }), 'MainMap');
		expect(fc.features[0].properties.name).toBe('BOSS_BATTLE_NAME_DesertBoss');
	});

	// An array artifact's key is a GUID or a UAID blob, never a display name.
	it('falls back to the layer label for an array artifact', () => {
		const fc = buildMapLayerFC('camps', list([AT]), 'MainMap');
		expect(fc.features[0].properties.name).toBe(mapLayerLabel('camps'));
	});

	it('never renders the literal undefined', () => {
		const fc = buildMapLayerFC('camps', list([AT]), 'MainMap');
		expect(fc.features[0].properties.name).not.toContain('undefined');
	});

	it('ignores an empty localized_name rather than showing a blank label', () => {
		const fc = buildMapLayerFC(
			'tower_boss',
			keyed({ IceBoss: { ...AT, localized_name: '' } }),
			'MainMap'
		);
		expect(fc.features[0].properties.name).toBe('IceBoss');
	});
});

// The panel count has to agree with what the map draws. The legacy layers all
// report their area-filtered total (relics 360 of 407), and a registry layer
// reporting its raw entry count instead read as 188 skill fruits beside 35
// markers.
describe('mapLayerMarkerCount', () => {
	const AT = { x: -343155, y: 244585, z: 0 };
	const TREE = { x: 512112, y: -510663, z: 0 };

	it('counts only what buildMapLayerFC would draw', () => {
		const selection = list([AT, TREE, { x: null, y: null, z: 0 }, AT]);
		expect(mapLayerMarkerCount('camps', selection, 'MainMap')).toBe(
			buildMapLayerFC('camps', selection, 'MainMap').features.length
		);
	});

	it('drops entries with no coordinate', () => {
		// 141 of skill_fruits' 188 rows are positionless location components.
		expect(mapLayerMarkerCount('camps', list([AT, { x: null, y: null, z: 0 }]), 'MainMap')).toBe(1);
	});

	it('drops entries belonging to another area', () => {
		expect(mapLayerMarkerCount('camps', list([AT, TREE]), 'MainMap')).toBe(1);
		expect(mapLayerMarkerCount('camps', list([AT, TREE]), 'Tree')).toBe(1);
	});

	it('is zero for a layer with nothing loaded yet', () => {
		// Distinct from "loaded and empty" only to the caller; both draw nothing.
		expect(mapLayerMarkerCount('camps', undefined, 'MainMap')).toBe(0);
	});
});

// icon-size multiplies the sprite's natural size, and the artwork is not one
// size: the game's compass markers are 64px while its item icons are 256px, so
// an item-icon layer drew four times the size of every other marker.
describe('mapLayerIconScale', () => {
	it('leaves a 64px compass-marker layer unscaled', () => {
		expect(mapLayerIconScale('camps')).toBe(1);
		expect(mapLayerIconScale('tower_boss')).toBe(1);
	});

	it('scales the 256px item-icon layers back to the common size', () => {
		expect(mapLayerIconScale('journals')).toBe(0.25);
		expect(mapLayerIconScale('kinship_peach')).toBe(0.25);
	});

	it('gives every layer a positive scale, defaulting to 1', () => {
		// A layer missing from the table must fall back to 1 rather than
		// undefined: undefined reaches the stops as NaN and the marker vanishes.
		for (const layer of MAP_LAYERS) {
			expect([layer.id, mapLayerIconScale(layer.id) > 0]).toEqual([layer.id, true]);
			expect(Number.isFinite(mapLayerIconScale(layer.id))).toBe(true);
		}
	});
});

describe('mapLayerIcon', () => {
	// Layers with a bespoke feature builder never reach mapLayerIcon: the spawn
	// layers take a per-pal sprite and relics one per relic_type.
	const BESPOKE: MapLayerId[] = ['alpha_pals', 'boss_pals', 'predator_pals', 'bounty', 'relics'];
	const generic = MAP_LAYERS.map((layer) => layer.id).filter((id) => !BESPOKE.includes(id));

	it('gives every generically drawn layer a sprite id', () => {
		for (const id of generic) expect(mapLayerIcon(id)).toBeTruthy();
	});

	it('gives every generically drawn layer an icon of its own', () => {
		// mapLayerIcon falls back to ICON_DUNGEON, so a layer missing from the
		// table draws as a dungeon rather than failing - silent and wrong.
		for (const id of generic) {
			if (id === 'dungeons') continue;
			expect([id, mapLayerIcon(id)]).not.toEqual([id, ICON_DUNGEON]);
		}
	});

	it('does not give two layers the same sprite', () => {
		const icons = generic.map((id) => mapLayerIcon(id));
		expect(new Set(icons).size).toBe(icons.length);
	});
});
