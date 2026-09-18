import * as m from '$i18n/messages';
import { c } from '$lib/utils/commonTranslations';
import { readFile } from 'node:fs/promises';
import { fileURLToPath } from 'node:url';
import { describe, expect, it } from 'vitest';

import {
	PANEL_EXTRAS,
	buildPanelGroups,
	defaultLayerVisibility,
	groupVisibilityPatch,
	hideAllLabel,
	loadingLabel,
	mapLayerGroupLabel,
	mapLayerLabel,
	panelIcon,
	panelOptionLabel,
	showAllLabel,
	type MapLayerGroupModel
} from './layerPanelModel';
import { MAP_LAYERS, MAP_LAYER_GROUPS, mapLayersInGroup } from './layerRegistry';

const noCounts = () => undefined;
const noneLoading = () => false;
const allRows = (groups: MapLayerGroupModel[]) => groups.flatMap((g) => g.rows);

describe('mapLayerLabel', () => {
	it('labels every registered layer with a non-empty string', () => {
		for (const layer of MAP_LAYERS) {
			expect(mapLayerLabel(layer.id).trim()).not.toBe('');
		}
	});

	it('gives each layer a distinct label', () => {
		const labels = MAP_LAYERS.map((layer) => mapLayerLabel(layer.id));
		expect(new Set(labels).size).toBe(labels.length);
	});

	it('reads the existing translated message where one exists', () => {
		expect(mapLayerLabel('fast_travel')).toBe(m.fast_travel());
		expect(mapLayerLabel('watchtower')).toBe(m.watchtower());
		expect(mapLayerLabel('dungeons')).toBe(m.dungeons());
		expect(mapLayerLabel('relics')).toBe(m.relics());
		expect(mapLayerLabel('boss_pals')).toBe(m.bosses());
		expect(mapLayerLabel('alpha_pals')).toBe(c.alphaPals);
		expect(mapLayerLabel('predator_pals')).toBe(c.predatorPals);
		expect(mapLayerLabel('eggs')).toBe(m.eggs());
		expect(mapLayerLabel('tower_boss')).toBe(m.tower_boss());
		expect(mapLayerLabel('camps')).toBe(m.camps());
		expect(mapLayerLabel('journals')).toBe(m.journals());
	});

	it('reads every group heading and bulk control from a message too', () => {
		expect(mapLayerGroupLabel('locations')).toBe(m.locations());
		expect(mapLayerGroupLabel('collectibles')).toBe(m.collectibles());
		expect(mapLayerGroupLabel('poi')).toBe(m.poi());
		expect(showAllLabel()).toBe(m.show_all());
		expect(hideAllLabel()).toBe(m.hide_all());
		expect(mapLayerGroupLabel('general')).toBe(m.general());
		expect(loadingLabel()).toBe(m.loading());
	});

	it('leaves no hardcoded English label in the source', async () => {
		const source = await readFile(
			fileURLToPath(new URL('./layerPanelModel.ts', import.meta.url)),
			'utf8'
		);
		for (const literal of [
			'Tower Bosses',
			'Camps',
			'Locations',
			'Collectibles',
			'Points of Interest',
			'Show all',
			'Hide all',
			'General',
			'Loading'
		]) {
			expect(source).not.toContain(`'${literal}'`);
		}
	});
});

describe('mapLayerGroupLabel', () => {
	it('labels every group with a non-empty string', () => {
		for (const group of MAP_LAYER_GROUPS) {
			expect(mapLayerGroupLabel(group).trim()).not.toBe('');
		}
	});
});

describe('defaultLayerVisibility', () => {
	it('mirrors the registry defaults for every layer', () => {
		const defaults = defaultLayerVisibility();
		expect(Object.keys(defaults).sort()).toEqual(MAP_LAYERS.map((layer) => layer.id).sort());
		for (const layer of MAP_LAYERS) {
			expect(defaults[layer.id]).toBe(layer.defaultVisible);
		}
	});
});

describe('buildPanelGroups', () => {
	const allOn = Object.fromEntries(MAP_LAYERS.map((layer) => [layer.id, true]));

	it('returns the groups in legend order', () => {
		expect(
			buildPanelGroups(allOn, { count: noCounts, loading: noneLoading }).map((g) => g.group)
		).toEqual([...MAP_LAYER_GROUPS]);
	});

	it('keeps rows in registration order within a group', () => {
		const locations = buildPanelGroups(allOn, { count: noCounts, loading: noneLoading }).find(
			(g) => g.group === 'locations'
		)!;
		expect(locations.rows.map((row) => row.id)).toEqual([
			'fast_travel',
			'watchtower',
			'tower_boss',
			'dungeons',
			'ancient_ruins'
		]);
	});

	it('reflects the visibility record passed in', () => {
		const rows = buildPanelGroups(
			{ ...allOn, dungeons: false },
			{ count: noCounts, loading: noneLoading }
		).flatMap((g) => g.rows);
		expect(rows.find((row) => row.id === 'dungeons')!.visible).toBe(false);
		expect(rows.find((row) => row.id === 'fast_travel')!.visible).toBe(true);
	});

	it('falls back to the registry default when the record omits a layer', () => {
		const rows = buildPanelGroups({}, { count: noCounts, loading: noneLoading }).flatMap(
			(g) => g.rows
		);
		expect(rows.find((row) => row.id === 'dungeons')!.visible).toBe(true);
		expect(rows.find((row) => row.id === 'camps')!.visible).toBe(false);
	});

	it('marks a row loading while its artifact is on the way', () => {
		const rows = buildPanelGroups(allOn, {
			count: noCounts,
			loading: (id) => id === 'camps'
		}).flatMap((g) => g.rows);
		expect(rows.find((row) => row.id === 'camps')!.loading).toBe(true);
		expect(rows.find((row) => row.id === 'dungeons')!.loading).toBe(false);
	});

	it('carries the count string through verbatim, two-part forms included', () => {
		const counts = (id: string) =>
			id === 'dungeons' ? '42' : id === 'fast_travel' ? '17/24' : undefined;
		const rows = allRows(buildPanelGroups(allOn, { count: counts, loading: noneLoading }));
		expect(rows.find((row) => row.id === 'dungeons')!.count).toBe('42');
		expect(rows.find((row) => row.id === 'fast_travel')!.count).toBe('17/24');
		expect(rows.find((row) => row.id === 'camps')!.count).toBeUndefined();
	});

	it('reports whether a group is fully or entirely not shown', () => {
		const on = buildPanelGroups(allOn, { count: noCounts, loading: noneLoading }).find(
			(g) => g.group === 'locations'
		)!;
		expect(on.allVisible).toBe(true);
		expect(on.noneVisible).toBe(false);

		const mixed = buildPanelGroups(
			{ ...allOn, dungeons: false },
			{ count: noCounts, loading: noneLoading }
		).find((g) => g.group === 'locations')!;
		expect(mixed.allVisible).toBe(false);
		expect(mixed.noneVisible).toBe(false);

		const off = buildPanelGroups(groupVisibilityPatch('locations', false), {
			count: noCounts,
			loading: noneLoading
		}).find((g) => g.group === 'locations')!;
		expect(off.noneVisible).toBe(true);
	});
});

describe('non-layer options', () => {
	const allOn = Object.fromEntries([
		...MAP_LAYERS.map((layer) => [layer.id, true]),
		...PANEL_EXTRAS.map((extra) => [extra.id, true])
	]);

	it('appears in the model alongside the registry layers', () => {
		const ids = allRows(buildPanelGroups(allOn, {})).map((row) => row.id);
		for (const extra of PANEL_EXTRAS) expect(ids).toContain(extra.id);
	});

	it('puts them in the first group, before any layer group', () => {
		const groups = buildPanelGroups(allOn, {});
		expect(groups[0].rows.map((row) => row.id)).toEqual(PANEL_EXTRAS.map((e) => e.id));
	});

	it('labels every one of them', () => {
		for (const extra of PANEL_EXTRAS) {
			expect(panelOptionLabel(extra.id).trim()).not.toBe('');
		}
	});

	it('drops the save-gated options when they are unavailable', () => {
		const groups = buildPanelGroups(allOn, {
			available: (id) => id !== 'players' && id !== 'bases'
		});
		const ids = allRows(groups).map((row) => row.id);
		expect(ids).not.toContain('players');
		expect(ids).not.toContain('bases');
		expect(ids).toContain('origin');
		expect(ids).toContain('labels');
	});

	it('drops a group entirely when nothing in it is available', () => {
		const groups = buildPanelGroups(allOn, { available: (id) => id !== 'relics' });
		const collectibles = groups.find((g) => g.group === 'collectibles')!;
		expect(collectibles.rows.map((row) => row.id)).not.toContain('relics');
	});
});

describe('no option renders twice', () => {
	it('yields each id exactly once across every group', () => {
		const allOn = Object.fromEntries([
			...MAP_LAYERS.map((layer) => [layer.id, true]),
			...PANEL_EXTRAS.map((extra) => [extra.id, true])
		]);
		const ids = allRows(buildPanelGroups(allOn, {})).map((row) => row.id);
		expect(new Set(ids).size).toBe(ids.length);
	});
});

describe('panelIcon', () => {
	it('gives every layer and every extra an image url', () => {
		for (const layer of MAP_LAYERS) expect(panelIcon(layer.id)).toBeTruthy();
		for (const extra of PANEL_EXTRAS) expect(panelIcon(extra.id)).toBeTruthy();
	});

	it('points journals at the technology book art palpedia uses', () => {
		expect(panelIcon('journals')).toContain('technologybook');
	});

	it('gives no two options the same image', () => {
		const ids = [...MAP_LAYERS.map((layer) => layer.id), ...PANEL_EXTRAS.map((e) => e.id)];
		const byIcon = new Map<string, string[]>();
		for (const id of ids) {
			const icon = panelIcon(id);
			byIcon.set(icon, [...(byIcon.get(icon) ?? []), id]);
		}
		const shared = [...byIcon.entries()].filter(([, owners]) => owners.length > 1);
		expect(shared).toEqual([]);
	});
});

describe('groupVisibilityPatch', () => {
	it('covers exactly the ids in that group', () => {
		expect(Object.keys(groupVisibilityPatch('poi', true))).toEqual([
			'alpha_pals',
			'boss_pals',
			'predator_pals',
			'bounty',
			'camps'
		]);
	});

	it('sets every id in the group to the requested value', () => {
		const patch = groupVisibilityPatch('collectibles', false);
		const ids = mapLayersInGroup('collectibles').map((layer) => layer.id);
		expect(Object.keys(patch).sort()).toEqual([...ids].sort());
		expect(Object.values(patch).every((visible) => visible === false)).toBe(true);
	});

	it('touches no layer outside the group', () => {
		expect(groupVisibilityPatch('poi', true)).not.toHaveProperty('dungeons');
	});
});
