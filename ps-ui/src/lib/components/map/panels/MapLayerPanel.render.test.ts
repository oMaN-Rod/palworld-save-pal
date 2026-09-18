import { render } from '@testing-library/svelte';
import userEvent from '@testing-library/user-event';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import '../../live/__tests__/fixtures/animatePolyfill';
import '../../pal/__tests__/fixtures/matchMediaPolyfill';

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
	mapLayerGroupLabel,
	mapLayerLabel,
	panelOptionLabel
} from '../layers/layerPanelModel';
import { MAP_LAYERS, MAP_LAYER_GROUPS } from '../layers/layerRegistry';
import MapLayerPanel from './MapLayerPanel.svelte';

// Only an expanded group keeps its rows in the DOM, so every structural
// assertion below opens the collapsed ones first.
const html = async (props: Record<string, unknown> = {}) => {
	const { container } = render(MapLayerPanel, {
		props: {
			layers: Object.fromEntries([
				...MAP_LAYERS.map((layer) => [layer.id, true]),
				...PANEL_EXTRAS.map((extra) => [extra.id, true])
			]),
			onVisibilityChange: () => {},
			...props
		}
	});
	const user = userEvent.setup();
	for (const trigger of container.querySelectorAll<HTMLElement>(
		'[data-part="item-trigger"][aria-expanded="false"]'
	)) {
		await user.click(trigger);
	}
	return container.innerHTML;
};

beforeEach(() => {
	peek.mockReset();
	getLayer.mockReset();
	isLoading.mockReset();
	peek.mockReturnValue(undefined);
	isLoading.mockReturnValue(false);
});

describe('MapLayerPanel structure', () => {
	it('renders every group, labelled, in legend order', async () => {
		const body = await html();
		const positions = MAP_LAYER_GROUPS.map((group) => body.indexOf(mapLayerGroupLabel(group)));
		expect(positions.every((index) => index >= 0)).toBe(true);
		expect([...positions].sort((a, b) => a - b)).toEqual(positions);
	});

	it('renders a row for every layer and every extra', async () => {
		const body = await html();
		for (const layer of MAP_LAYERS) expect(body).toContain(mapLayerLabel(layer.id));
		for (const extra of PANEL_EXTRAS) expect(body).toContain(panelOptionLabel(extra.id));
	});

	it('uses a button per option, never a checkbox or radio', async () => {
		const body = await html();
		expect(body).not.toMatch(/type="checkbox"/);
		expect(body).not.toMatch(/type="radio"/);
		const buttons = body.match(/<button[^>]*data-option="/g) ?? [];
		expect(buttons).toHaveLength(MAP_LAYERS.length + PANEL_EXTRAS.length);
	});

	it('gives every option an icon image with the existing sizing', async () => {
		const body = await html();
		const icons = body.match(/<img[^>]*class="[^"]*mr-2 h-6 w-6/g) ?? [];
		expect(icons).toHaveLength(MAP_LAYERS.length + PANEL_EXTRAS.length);
	});

	it('lays each group out in a two column grid', async () => {
		const body = await html();
		expect(body.match(/grid grid-cols-2 gap-2/g) ?? []).toHaveLength(MAP_LAYER_GROUPS.length);
	});

	it('rules off every category and draws no box around itself', async () => {
		const body = await html();
		expect(body.match(/border-b-surface-800/g) ?? []).toHaveLength(MAP_LAYER_GROUPS.length);
		expect(body).not.toMatch(/rounded-sm border /);
	});
});

describe('MapLayerPanel visibility', () => {
	it('dims an option that is switched off and leaves an active one undimmed', async () => {
		const body = await html({ layers: { dungeons: false, camps: true } });
		expect(body).toMatch(/data-option="dungeons"[^>]*opacity-25/);
		expect(body).not.toMatch(/data-option="camps"[^>]*opacity-25/);
	});

	it('falls back to the registry default for a layer the record omits', async () => {
		const body = await html({ layers: {} });
		expect(body).not.toMatch(/data-option="dungeons"[^>]*opacity-25/);
		expect(body).toMatch(/data-option="camps"[^>]*opacity-25/);
	});

	it('reports the clicked option back to the caller', async () => {
		const changes: Record<string, boolean>[] = [];
		const { container } = render(MapLayerPanel, {
			props: {
				layers: Object.fromEntries([
					...MAP_LAYERS.map((layer) => [layer.id, true]),
					...PANEL_EXTRAS.map((extra) => [extra.id, true])
				]),
				onVisibilityChange: (patch: Record<string, boolean>) => changes.push(patch)
			}
		});
		await userEvent.setup().click(container.querySelector<HTMLElement>('[data-option="origin"]')!);
		expect(changes).toEqual([{ origin: false }]);
	});
});

describe('MapLayerPanel counts', () => {
	it('shows a marker count once the artifact has landed', async () => {
		peek.mockImplementation((id: string) =>
			id === 'dungeons' ? { shape: 'keyed', points: [{}, {}, {}] } : undefined
		);
		expect(await html()).toContain('>3<');
	});

	it('renders a two-part count from the caller verbatim', async () => {
		const body = await html({
			count: (id: string) => (id === 'fast_travel' ? '17/24' : undefined)
		});
		expect(body).toContain('>17/24<');
	});

	it('reads through peek and never asks the store to fetch', async () => {
		await html();
		expect(peek).toHaveBeenCalled();
		expect(getLayer).not.toHaveBeenCalled();
	});

	it('marks an in-flight row as loading', async () => {
		isLoading.mockImplementation((id: string) => id === 'camps');
		const body = await html();
		expect(body).toMatch(/data-loading="camps"/);
		expect(body).not.toMatch(/data-loading="dungeons"/);
	});
});

describe('MapLayerPanel availability', () => {
	it('omits an option the caller reports as unavailable', async () => {
		const body = await html({ available: (id: string) => id !== 'players' && id !== 'bases' });
		expect(body).not.toMatch(/data-option="players"/);
		expect(body).not.toMatch(/data-option="bases"/);
		expect(body).toMatch(/data-option="origin"/);
	});
});
