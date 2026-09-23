// @vitest-environment jsdom
import { render, screen } from '@testing-library/svelte';
import { createRawSnippet, tick } from 'svelte';
import { beforeEach, describe, expect, it, vi } from 'vitest';

import { installViewportStub } from '$utils/__tests__/fixtures/viewportStub';

const { modalState } = vi.hoisted(() => ({
	modalState: {
		isOpen: true,
		stack: [] as { id: string; component: unknown; props: Record<string, unknown> }[],
		closeModal: vi.fn(),
		closeEntry: vi.fn()
	}
}));

vi.mock('$states', () => ({ getModalState: () => modalState }));

const { setViewport } = installViewportStub();
const { default: Modal } = await import('../Modal.svelte');
const { default: ModalPanelStub } = await import('./fixtures/ModalPanelStub.svelte');

function mount() {
	return render(Modal, {
		props: { children: createRawSnippet(() => ({ render: () => '<div></div>' })) }
	});
}

function closeButton(): HTMLElement {
	return screen.getByRole('button', { name: 'Close' });
}

beforeEach(() => {
	vi.clearAllMocks();
	setViewport(1440);
	modalState.stack = [{ id: 'entry-1', component: ModalPanelStub, props: {} }];
});

describe('Modal', () => {
	it('keeps a gutter around the panel', async () => {
		mount();
		await tick();

		expect(screen.getByRole('dialog').className).toContain('p-4');
	});

	it('parks the close button beside the panel on a desktop', async () => {
		mount();
		await tick();

		expect(closeButton().className).toContain('left-full');
	});

	it('gives the close button its own row on a phone', async () => {
		setViewport(390);
		mount();
		await tick();

		const close = closeButton();
		expect(close.className).not.toContain('left-full');
		expect(close.contains(screen.getByTestId('modal-body'))).toBe(false);
	});

	it('lets a tall panel scroll on a phone', async () => {
		setViewport(390);
		mount();
		await tick();

		const scroller = screen.getByTestId('modal-body').parentElement;
		expect(scroller?.className).toContain('overflow-y-auto');
	});

	it('closes the entry the panel reports against', async () => {
		mount();
		await tick();

		screen.getByTestId('modal-body').click();

		expect(modalState.closeEntry).toHaveBeenCalledWith('entry-1', 'ok');
	});
});
