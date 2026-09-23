// @vitest-environment jsdom
import { fireEvent, render, screen } from '@testing-library/svelte';
import { tick } from 'svelte';
import { beforeEach, describe, expect, it, vi } from 'vitest';

const { appState, editors } = vi.hoisted(() => ({
	editors: [] as unknown[],
	appState: {
		saveFile: { name: 'save' } as unknown,
		guilds: {},
		players: {},
		playerSummaries: {},
		guildSummaries: {},
		selectedPlayer: undefined as unknown,
		settings: { debug_mode: true }
	}
}));

vi.mock('$states', () => ({
	getAppState: () => appState,
	getModalState: () => ({ showModal: vi.fn(), showConfirmModal: vi.fn() })
}));
vi.mock('$app/navigation', () => ({ goto: vi.fn() }));
vi.mock('$app/state', () => ({ page: { url: new URL('http://localhost/debug') } }));
vi.mock('$lib/utils/websocketUtils', () => ({ send: vi.fn(), sendAndWait: vi.fn() }));
vi.mock('$lib/data', () => ({ buildingsData: { getByKey: () => undefined } }));
vi.mock('svelte-jsoneditor', () => ({
	JSONEditor: (_anchor: unknown, props: unknown) => {
		editors.push(props);
	}
}));

import DebugPage from '../+page.svelte';

beforeEach(() => {
	vi.clearAllMocks();
	editors.length = 0;
});

describe('debug page', () => {
	it('stacks the picker column over the editor below the breakpoint', async () => {
		render(DebugPage);
		await tick();

		const shell = document.querySelector('#debug-shell');
		expect(shell).not.toBeNull();
		expect(shell?.className).toContain('flex-col');
		expect(shell?.className).toContain('md:grid');
		const unprefixed = (shell?.className ?? '').split(/ +/).filter((token) => !token.includes(':'));
		expect(unprefixed.some((token) => token.startsWith('grid-cols-'))).toBe(false);
	});

	it('mounts one editor at a time', async () => {
		render(DebugPage);
		await tick();

		expect(screen.getAllByRole('tabpanel').length).toBe(1);
		expect(editors.length).toBe(1);
	});

	it('swaps the panel when another tab is picked', async () => {
		render(DebugPage);
		await tick();

		const before = screen.getByRole('tabpanel').id;
		await fireEvent.click(screen.getByRole('tab', { name: /pal$/i }));
		await tick();

		expect(screen.getByRole('tabpanel').id).not.toBe(before);
	});

	it('keeps the tab strip on one scrolling row', async () => {
		render(DebugPage);
		await tick();

		const strip = screen.getByRole('tablist');
		expect(strip.className).not.toContain('flex-col');
		for (const tab of screen.getAllByRole('tab')) {
			expect(tab.className).toContain('whitespace-nowrap');
		}
	});
});
