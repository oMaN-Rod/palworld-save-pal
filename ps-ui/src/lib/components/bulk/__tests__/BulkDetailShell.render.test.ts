// @vitest-environment jsdom
import '$utils/__tests__/fixtures/animatePolyfill';
import { fireEvent, render, screen, within } from '@testing-library/svelte';
import { createRawSnippet, tick } from 'svelte';
import { beforeEach, describe, expect, it, vi } from 'vitest';

import { installViewportStub } from '$utils/__tests__/fixtures/viewportStub';

// Install before the component loads, since it reaches the layout store.
const { setViewport } = installViewportStub();
const { default: BulkDetailShell } = await import('../BulkDetailShell.svelte');

function body() {
	return createRawSnippet(() => ({
		render: () => '<p data-testid="bulk-detail-body">Detail</p>'
	}));
}

function renderShell(expanded = true, onclose = vi.fn()) {
	render(BulkDetailShell, {
		props: { title: 'Player', expanded, onclose, children: body() }
	});
	return onclose;
}

beforeEach(() => {
	vi.clearAllMocks();
	setViewport(1440);
});

describe('BulkDetailShell', () => {
	it('keeps the detail beside the table on a desktop', async () => {
		renderShell();
		await tick();

		expect(screen.queryByRole('dialog')).toBeNull();
		expect(screen.getByTestId('bulk-detail-body')).not.toBeNull();
	});

	it('lifts the detail into a sheet on a phone', async () => {
		setViewport(390);
		renderShell();
		await tick();

		const sheet = screen.getByRole('dialog');
		expect(within(sheet).getByTestId('bulk-detail-body')).not.toBeNull();
		expect(within(sheet).getByText('Player')).not.toBeNull();
	});

	it('shows nothing until a row is picked', async () => {
		setViewport(390);
		renderShell(false);
		await tick();

		expect(screen.queryByRole('dialog')).toBeNull();
		expect(screen.queryByTestId('bulk-detail-body')).toBeNull();
	});

	it('reports a dismissal from the sheet', async () => {
		setViewport(390);
		const onclose = renderShell();
		await tick();

		await fireEvent.click(screen.getByTestId('sheet-backdrop'));

		expect(onclose).toHaveBeenCalledTimes(1);
	});

	it('reports a dismissal from the drawer', async () => {
		const onclose = renderShell();
		await tick();

		await fireEvent.click(screen.getByLabelText('Close drawer'));

		expect(onclose).toHaveBeenCalledTimes(1);
	});
});
