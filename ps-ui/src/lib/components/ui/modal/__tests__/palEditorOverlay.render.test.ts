// @vitest-environment jsdom
import '$utils/__tests__/fixtures/animatePolyfill';
import { render, screen } from '@testing-library/svelte';
import { tick } from 'svelte';
import { beforeEach, describe, expect, it, vi } from 'vitest';

import { installViewportStub } from '$utils/__tests__/fixtures/viewportStub';

const { palEditor, modalState, appState } = vi.hoisted(() => ({
	palEditor: { isOpen: false, loading: false, close: vi.fn() },
	modalState: { isOpen: false },
	appState: { selectedPal: undefined as unknown }
}));

vi.mock('$states', () => ({
	getPalEditorState: () => palEditor,
	getModalState: () => modalState,
	getAppState: () => appState
}));

vi.mock('$components/modals/pal-edit/PalEditModal.svelte', () => ({ default: vi.fn() }));

const { setViewport } = installViewportStub();
const { default: PalEditorOverlay } = await import('../PalEditorOverlay.svelte');

function surface(): HTMLElement {
	return screen.getByTestId('pal-editor-surface');
}

beforeEach(() => {
	setViewport(1440);
	palEditor.isOpen = true;
	palEditor.loading = false;
	palEditor.close.mockClear();
	modalState.isOpen = false;
	appState.selectedPal = { instance_id: 'pal-1' };
});

describe('PalEditorOverlay', () => {
	it('fills a phone screen rather than insetting the editor', async () => {
		setViewport(390);
		render(PalEditorOverlay);
		await tick();

		const className = surface().className;
		expect(className).not.toContain('90vw');
		expect(className).not.toContain('90vh');
		expect(className).toContain('h-full');
		expect(className).toContain('w-full');
	});

	it('keeps the inset box on a desktop', async () => {
		render(PalEditorOverlay);
		await tick();

		const className = surface().className;
		expect(className).toContain('90vw');
		expect(className).toContain('90vh');
	});

	it('brings the close button inside the sheet on a phone', async () => {
		setViewport(390);
		render(PalEditorOverlay);
		await tick();

		const close = screen.getByRole('button', { name: 'Close' });
		expect(close.className).not.toContain('left-full');
	});

	it('renders nothing while the editor is closed', async () => {
		palEditor.isOpen = false;
		render(PalEditorOverlay);
		await tick();

		expect(screen.queryByTestId('pal-editor-surface')).toBeNull();
	});
});
