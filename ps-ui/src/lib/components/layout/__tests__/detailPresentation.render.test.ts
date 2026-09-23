// @vitest-environment jsdom
import '$utils/__tests__/fixtures/animatePolyfill';
import { fireEvent, render, screen, within } from '@testing-library/svelte';
import { createRawSnippet, tick } from 'svelte';
import { beforeEach, describe, expect, it, vi } from 'vitest';

import { installViewportStub } from '$utils/__tests__/fixtures/viewportStub';

// Install before the component loads, since it reaches the layout store.
const { setViewport } = installViewportStub();
const { default: DetailPresentation } = await import('../DetailPresentation.svelte');

function body() {
	return createRawSnippet(() => ({
		render: () => '<p data-testid="detail-body">Detail</p>'
	}));
}

function renderPresentation(props: Record<string, unknown> = {}) {
	const onClose = vi.fn();
	render(DetailPresentation, {
		props: { title: 'Cool Mod', onClose, children: body(), ...props }
	});
	return onClose;
}

beforeEach(() => {
	vi.clearAllMocks();
	setViewport(1440);
});

describe('DetailPresentation', () => {
	it('keeps the detail beside the list on a desktop', async () => {
		renderPresentation();
		await tick();

		expect(screen.queryByRole('dialog')).toBeNull();
		expect(screen.getByTestId('detail-body')).not.toBeNull();
	});

	it('lets the caller place the desktop pane', async () => {
		renderPresentation({ class: 'order-first lg:order-none' });
		await tick();

		expect(screen.getByTestId('detail-body').parentElement?.className).toBe(
			'order-first lg:order-none'
		);
	});

	it('lifts the detail into a sheet on a phone', async () => {
		setViewport(390);
		renderPresentation();
		await tick();

		const sheet = screen.getByRole('dialog');
		expect(within(sheet).getByTestId('detail-body')).not.toBeNull();
		expect(within(sheet).getByText('Cool Mod')).not.toBeNull();
	});

	it('leaves the list alone until something is picked', async () => {
		setViewport(390);
		renderPresentation({ active: false });
		await tick();

		expect(screen.queryByRole('dialog')).toBeNull();
		expect(screen.queryByTestId('detail-body')).toBeNull();
	});

	it('still renders the pane on a desktop with nothing picked', async () => {
		renderPresentation({ active: false });
		await tick();

		expect(screen.getByTestId('detail-body')).not.toBeNull();
	});

	it('reports a dismissal so the list can drop its selection', async () => {
		setViewport(390);
		const onClose = renderPresentation();
		await tick();

		await fireEvent.click(screen.getByTestId('sheet-backdrop'));

		expect(onClose).toHaveBeenCalledTimes(1);
	});
});
