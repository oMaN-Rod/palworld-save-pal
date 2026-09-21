// @vitest-environment jsdom
import { fireEvent, render, screen } from '@testing-library/svelte';
import { createRawSnippet, tick } from 'svelte';
import { beforeEach, describe, expect, it, vi } from 'vitest';

import { navDrawer } from '$states/navDrawer.svelte';
import { installViewportStub } from '$utils/__tests__/fixtures/viewportStub';

const { appState, modal, palEditor, goto, send } = vi.hoisted(() => ({
	appState: {
		saveFile: undefined as { name: string } | undefined,
		selectedPlayer: undefined as { uid: string; dps?: unknown } | undefined,
		selectedPal: undefined as unknown,
		settings: { debug_mode: false },
		hasGpsAvailable: false
	},
	modal: { isOpen: false, showConfirmModal: vi.fn() },
	palEditor: { isOpen: false, open: vi.fn() },
	goto: vi.fn(),
	send: vi.fn()
}));

vi.mock('$app/navigation', () => ({ goto: (...args: unknown[]) => goto(...args) }));
vi.mock('$app/state', () => ({ page: { url: new URL('http://localhost/edit') } }));
vi.mock('$states', () => ({
	getAppState: () => appState,
	getModalState: () => modal,
	getPalEditorState: () => palEditor
}));
vi.mock('$utils/websocketUtils', () => ({ send: (...args: unknown[]) => send(...args) }));

const { setViewport } = installViewportStub();
const { default: EditLayout } = await import('./+layout.svelte');

function childrenSnippet() {
	return createRawSnippet(() => ({ render: () => '<div></div>' }));
}

describe('edit layout drawer trigger', () => {
	beforeEach(() => {
		vi.clearAllMocks();
		navDrawer.open = false;
		appState.saveFile = undefined;
		appState.selectedPlayer = undefined;
	});

	it('shows the drawer trigger at phone width', async () => {
		setViewport(390);
		render(EditLayout, { props: { children: childrenSnippet() } });
		await tick();

		expect(screen.getByRole('button', { name: /menu/i })).not.toBeNull();
	});

	it('hides the drawer trigger at desktop width', async () => {
		setViewport(1440);
		render(EditLayout, { props: { children: childrenSnippet() } });
		await tick();

		expect(screen.queryByRole('button', { name: /menu/i })).toBeNull();
	});

	it('opens the nav drawer when the trigger is activated', async () => {
		setViewport(390);
		render(EditLayout, { props: { children: childrenSnippet() } });
		await tick();

		await fireEvent.click(screen.getByRole('button', { name: /menu/i }));

		expect(navDrawer.open).toBe(true);
	});
});
