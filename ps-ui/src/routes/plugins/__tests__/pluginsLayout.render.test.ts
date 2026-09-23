// @vitest-environment jsdom
import { render, screen } from '@testing-library/svelte';
import { createRawSnippet, tick } from 'svelte';
import { beforeEach, describe, expect, it, vi } from 'vitest';

const { pageState, goto } = vi.hoisted(() => ({
	pageState: { params: {} as Record<string, string>, url: new URL('http://localhost/plugins') },
	goto: vi.fn()
}));

vi.mock('$app/state', () => ({ page: pageState }));
vi.mock('$app/navigation', () => ({ goto, beforeNavigate: vi.fn() }));

vi.mock('$lib/data', () => ({
	pluginsData: { plugins: [], list: vi.fn().mockResolvedValue(undefined) }
}));
vi.mock('$lib/plugins/pluginEditor.svelte', () => ({ pluginEditor: { dirty: false } }));
vi.mock('$states', () => ({
	getAppState: () => ({ settings: {} }),
	getModalState: () => ({ showModal: vi.fn() }),
	getToastState: () => ({ add: vi.fn() })
}));
vi.mock('$components', () => ({ TextInputModal: {} }));
vi.mock('../components/PluginList.svelte', () => ({ default: () => {} }));

import PluginsLayout from '../+layout.svelte';

function renderLayout() {
	render(PluginsLayout, {
		props: {
			children: createRawSnippet(() => ({
				render: () => '<p data-testid="plugin-detail">Detail</p>'
			}))
		}
	});
}

function unprefixed(className: string | undefined) {
	return (className ?? '').split(/ +/).filter((token) => !token.includes(':'));
}

beforeEach(() => {
	vi.clearAllMocks();
	pageState.params = {};
	pageState.url = new URL('http://localhost/plugins');
});

// jsdom applies no CSS breakpoints, so these pin the classes instead.
describe('plugins layout', () => {
	it('gives the list the whole width below the breakpoint', async () => {
		renderLayout();
		await tick();

		const list = document.querySelector('#plugins-list');
		expect(list).not.toBeNull();
		expect(list?.className).toContain('w-full');
		expect(list?.className).toContain('md:w-72');
		expect(unprefixed(list?.className)).not.toContain('w-72');
	});

	it('hides the empty detail pane while no plugin is picked', async () => {
		renderLayout();
		await tick();

		expect(document.querySelector('#plugins-detail')?.className).toContain('max-md:hidden');
		expect(document.querySelector('#plugins-list')?.className).not.toContain('max-md:hidden');
	});

	it('hides the list once a plugin is picked', async () => {
		pageState.params = { id: 'my-plugin' };
		renderLayout();
		await tick();

		expect(document.querySelector('#plugins-list')?.className).toContain('max-md:hidden');
		expect(document.querySelector('#plugins-detail')?.className).not.toContain('max-md:hidden');
		expect(screen.getByTestId('plugin-detail')).not.toBeNull();
	});

	it('offers a way back to the list only when one is hidden', async () => {
		renderLayout();
		await tick();
		expect(screen.queryByTestId('plugins-back')).toBeNull();

		pageState.params = { id: 'my-plugin' };
		renderLayout();
		await tick();

		const back = screen.getByTestId('plugins-back');
		expect(back.className).toContain('md:hidden');
		back.click();
		expect(goto).toHaveBeenCalledWith('/plugins');
	});
});
