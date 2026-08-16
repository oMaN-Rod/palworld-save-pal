import * as m from '$i18n/messages';
import { c } from '$lib/utils/commonTranslations';
import { readFile } from 'node:fs/promises';
import { fileURLToPath } from 'node:url';
import { render } from 'svelte/server';
import { beforeEach, describe, expect, it, vi } from 'vitest';

const peek = vi.fn();
const getLayer = vi.fn();
const isLoading = vi.fn();

vi.mock('$lib/data/mapLayerStore.svelte', () => ({
	mapLayers: {
		peek: (id: unknown) => peek(id),
		getLayer: (id: unknown) => getLayer(id),
		isLoading: (id: unknown) => isLoading(id)
	}
}));

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
import { MAP_LAYERS, MAP_LAYER_GROUPS } from './layerRegistry';
import MapLayerPanel from './MapLayerPanel.svelte';

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

	// Reading the translated message rather than a literal is the whole point;
	// a hardcoded 'Dungeons' would pass a truthiness check and fail this.
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

	// Every layer label now has a key; none may fall back to an English literal.
	it('reads every group heading and bulk control from a message too', () => {
		expect(mapLayerGroupLabel('locations')).toBe(m.locations());
		expect(mapLayerGroupLabel('collectibles')).toBe(m.collectibles());
		expect(mapLayerGroupLabel('poi')).toBe(m.poi());
		expect(showAllLabel()).toBe(m.show_all());
		expect(hideAllLabel()).toBe(m.hide_all());
		expect(mapLayerGroupLabel('general')).toBe(m.general());
		expect(loadingLabel()).toBe(m.loading());
	});

	// Tests run under the base locale, where an English literal and the message
	// it replaced are the same string -- so comparing the two cannot tell them
	// apart. The source can. Every label in the file now has a key, so none of
	// these may appear as a literal.
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
			'dungeons'
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

	// Fast Travel, Watchtower and Relics show "unlocked/total" when a player is
	// selected, and Players/Bases show "loaded/total". The model must carry the
	// string through untouched rather than reducing it to one number.
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

// The non-layer options (Origin, Players, Bases, Labels) are not backed by an
// artifact, so they are not registry layers -- but they render through the same
// model so there is only one list on screen.
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

	// Players and Bases were inside {#if appState.saveFile} and must not appear
	// when no save is loaded.
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

// The legacy block and the panel both rendered Fast Travel, Watchtower,
// Dungeons, Relics, Bosses, Alpha and Predator. There is one list now.
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

	// Two options sharing one image reads as a rendering bug, not as a category.
	// Labels and Fast Travel both drew the fast-travel tower two rows apart.
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
		expect(Object.values(groupVisibilityPatch('collectibles', false))).toEqual([
			false,
			false,
			false
		]);
	});

	it('touches no layer outside the group', () => {
		expect(groupVisibilityPatch('poi', true)).not.toHaveProperty('dungeons');
	});
});

const html = (props: Record<string, unknown> = {}) =>
	render(MapLayerPanel, {
		props: {
			layers: Object.fromEntries([
				...MAP_LAYERS.map((layer) => [layer.id, true]),
				...PANEL_EXTRAS.map((extra) => [extra.id, true])
			]),
			onVisibilityChange: () => {},
			...props
		}
	}).body;

beforeEach(() => {
	peek.mockReset();
	getLayer.mockReset();
	isLoading.mockReset();
	peek.mockReturnValue(undefined);
	isLoading.mockReturnValue(false);
});

describe('MapLayerPanel structure', () => {
	it('renders every group, labelled, in legend order', () => {
		const body = html();
		const labels = [
			mapLayerGroupLabel('general'),
			mapLayerGroupLabel('locations'),
			mapLayerGroupLabel('collectibles'),
			mapLayerGroupLabel('poi')
		];
		const positions = labels.map((label) => body.indexOf(label));
		expect(positions.every((index) => index >= 0)).toBe(true);
		expect([...positions].sort((a, b) => a - b)).toEqual(positions);
	});

	it('renders a row for every layer and every extra', () => {
		const body = html();
		for (const layer of MAP_LAYERS) expect(body).toContain(mapLayerLabel(layer.id));
		for (const extra of PANEL_EXTRAS) expect(body).toContain(panelOptionLabel(extra.id));
	});

	// The user asked for the existing option shape back: a clickable button with
	// an icon and a label, dimmed when off. No checkboxes, no radios.
	it('uses a button per option, never a checkbox or radio', () => {
		const body = html();
		expect(body).not.toMatch(/type="checkbox"/);
		expect(body).not.toMatch(/type="radio"/);
		const buttons = body.match(/<button[^>]*data-option="/g) ?? [];
		expect(buttons).toHaveLength(MAP_LAYERS.length + PANEL_EXTRAS.length);
	});

	it('gives every option an icon image with the existing sizing', () => {
		const body = html();
		const icons = body.match(/<img[^>]*class="[^"]*mr-2 h-6 w-6/g) ?? [];
		expect(icons).toHaveLength(MAP_LAYERS.length + PANEL_EXTRAS.length);
	});

	it('lays the options out in a two column grid', () => {
		expect(html().match(/grid grid-cols-2 gap-2/g) ?? []).toHaveLength(4);
	});

	// A rule between categories, in the idiom the Show All / Hide All row uses.
	it('separates the categories with a rule and draws no box around itself', () => {
		const body = html();
		expect(body.match(/border-b-surface-800/g) ?? []).toHaveLength(3);
		expect(body).not.toMatch(/rounded-sm border /);
	});
});

describe('MapLayerPanel visibility', () => {
	it('dims an option that is switched off and leaves an active one undimmed', () => {
		const body = html({ layers: { dungeons: false, camps: true } });
		expect(body).toMatch(/data-option="dungeons"[^>]*opacity-25/);
		expect(body).not.toMatch(/data-option="camps"[^>]*opacity-25/);
	});

	it('falls back to the registry default for a layer the record omits', () => {
		const body = html({ layers: {} });
		expect(body).not.toMatch(/data-option="dungeons"[^>]*opacity-25/);
		expect(body).toMatch(/data-option="camps"[^>]*opacity-25/);
	});
});

describe('MapLayerPanel counts', () => {
	it('shows a marker count once the artifact has landed', () => {
		peek.mockImplementation((id: string) =>
			id === 'dungeons' ? { shape: 'keyed', points: [{}, {}, {}] } : undefined
		);
		expect(html()).toContain('>3<');
	});

	it('renders a two-part count from the caller verbatim', () => {
		expect(html({ count: (id: string) => (id === 'fast_travel' ? '17/24' : undefined) })).toContain(
			'>17/24<'
		);
	});

	// Rendering the panel must never pull 20k markers over the wire.
	it('reads through peek and never asks the store to fetch', () => {
		html();
		expect(peek).toHaveBeenCalled();
		expect(getLayer).not.toHaveBeenCalled();
	});

	// A layer that has been asked for but has not arrived must not look identical
	// to one nobody requested.
	it('marks an in-flight row as loading', () => {
		isLoading.mockImplementation((id: string) => id === 'camps');
		const body = html();
		expect(body).toMatch(/data-loading="camps"/);
		expect(body).not.toMatch(/data-loading="dungeons"/);
	});
});

describe('MapLayerPanel availability', () => {
	it('omits an option the caller reports as unavailable', () => {
		const body = html({ available: (id: string) => id !== 'players' && id !== 'bases' });
		expect(body).not.toMatch(/data-option="players"/);
		expect(body).not.toMatch(/data-option="bases"/);
		expect(body).toMatch(/data-option="origin"/);
	});
});
