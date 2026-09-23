// @vitest-environment jsdom
import { render, screen } from '@testing-library/svelte';
import { tick } from 'svelte';
import { describe, expect, it, vi } from 'vitest';

vi.mock('$lib/utils/websocketUtils', () => ({ send: vi.fn(), sendAndWait: vi.fn() }));
vi.mock('../ViewSection.svelte', () => ({ default: () => {} }));

const { PluginViewState } = await import('$lib/plugins/viewState.svelte');
const { default: PluginView } = await import('../PluginView.svelte');

const UI = [
	{ group: 'Pals', widgets: [{ type: 'text', id: 'pals', value: 'Pals' }] },
	{ group: 'Bases', widgets: [{ type: 'text', id: 'bases', value: 'Bases' }] }
];

function renderView() {
	render(PluginView, {
		props: {
			state: new PluginViewState(UI as never, []),
			commands: [],
			onRun: vi.fn()
		}
	});
}

// jsdom applies no CSS breakpoints, so this pins the classes instead.
describe('PluginView', () => {
	it('stacks the group rail over its detail below the breakpoint', async () => {
		renderView();
		await tick();

		const pane = screen.getByRole('button', { name: 'Pals' }).parentElement?.parentElement;
		expect(pane?.className).toContain('flex-col');
		expect(pane?.className).toContain('md:grid');
		const unprefixed = (pane?.className ?? '').split(/ +/).filter((token) => !token.includes(':'));
		expect(unprefixed.some((token) => token.startsWith('grid-cols-'))).toBe(false);
	});

	it('keeps every group reachable', async () => {
		renderView();
		await tick();

		expect(screen.getByRole('button', { name: 'Pals' })).not.toBeNull();
		expect(screen.getByRole('button', { name: 'Bases' })).not.toBeNull();
	});
});
