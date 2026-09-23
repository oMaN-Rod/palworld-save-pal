// @vitest-environment jsdom
import { render, screen } from '@testing-library/svelte';
import { createRawSnippet, tick } from 'svelte';
import { beforeEach, describe, expect, it, vi } from 'vitest';

import { installViewportStub } from '$utils/__tests__/fixtures/viewportStub';

const { appState, modal, palEditor, navigation, goto, send } = vi.hoisted(() => ({
	appState: {
		saveFile: undefined as { name: string } | undefined,
		selectedPlayer: undefined as { uid: string; dps?: unknown } | undefined,
		selectedPal: undefined as unknown,
		settings: { debug_mode: false },
		players: {},
		playerSummaries: {},
		loadingPlayer: false,
		hasGpsAvailable: false
	},
	modal: { isOpen: false, showConfirmModal: vi.fn() },
	palEditor: { isOpen: false, open: vi.fn() },
	navigation: { saveAndNavigate: vi.fn() },
	goto: vi.fn(),
	send: vi.fn()
}));

vi.mock('$app/navigation', () => ({ goto: (...args: unknown[]) => goto(...args) }));
vi.mock('$app/state', () => ({ page: { url: new URL('http://localhost/edit') } }));
vi.mock('$states', () => ({
	getAppState: () => appState,
	getModalState: () => modal,
	getPalEditorState: () => palEditor,
	getNavigationState: () => navigation
}));
vi.mock('$utils/websocketUtils', () => ({ send: (...args: unknown[]) => send(...args) }));

const { setViewport } = installViewportStub();
const { default: EditLayout } = await import('./+layout.svelte');

function childrenSnippet() {
	return createRawSnippet(() => ({ render: () => '<div></div>' }));
}

describe('edit layout header', () => {
	beforeEach(() => {
		vi.clearAllMocks();
		setViewport(1440);
		appState.saveFile = undefined;
		appState.selectedPlayer = undefined;
	});

	it('gives the section tabs a row of their own', async () => {
		appState.saveFile = { name: 'Level.sav' };
		render(EditLayout, { props: { children: childrenSnippet() } });
		await tick();

		const list = document.querySelector('#player-list');
		const tabs = document.querySelector('#player-tabs');
		expect(list).not.toBeNull();
		expect(tabs).not.toBeNull();
		expect(tabs?.previousElementSibling?.contains(list!)).toBe(true);
	});

	it('holds the player controls back until a save is loaded', async () => {
		render(EditLayout, { props: { children: childrenSnippet() } });
		await tick();

		expect(document.querySelector('#player-list')).toBeNull();
		expect(document.querySelector('#player-tabs')).not.toBeNull();
	});

	it('leaves the drawer trigger to the shell at phone width', async () => {
		setViewport(390);
		render(EditLayout, { props: { children: childrenSnippet() } });
		await tick();

		expect(screen.queryByRole('button', { name: /menu/i })).toBeNull();
	});
});
