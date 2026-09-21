// @vitest-environment jsdom
import '$utils/__tests__/fixtures/animatePolyfill';
import { fireEvent, render, screen, within } from '@testing-library/svelte';
import { tick } from 'svelte';
import { beforeEach, describe, expect, it, vi } from 'vitest';

import { installViewportStub } from '$utils/__tests__/fixtures/viewportStub';

import type { ActionDescriptor } from '../../../ui/actions/actionDescriptor';

const { setViewport } = installViewportStub();
const { default: PalContainerToolbar } = await import('../PalContainerToolbar.svelte');

function makeActions() {
	const healFn = vi.fn();
	const deleteFn = vi.fn();
	const shareFn = vi.fn();
	const hiddenFn = vi.fn();

	const actions: ActionDescriptor[] = [
		{ id: 'heal', label: 'Heal selected', icon: 'tabler:bandage', run: () => healFn() },
		{
			id: 'delete',
			label: 'Delete selected',
			icon: 'tabler:trash',
			run: () => deleteFn('delete'),
			danger: true
		},
		{ id: 'clone', label: 'Clone to UPS', icon: 'tabler:copy', run: () => shareFn('clone') },
		{ id: 'export', label: 'Export to file', icon: 'tabler:upload', run: () => shareFn('export') },
		{
			id: 'hidden',
			label: 'Hidden action',
			icon: 'tabler:eye-off',
			run: () => hiddenFn(),
			available: () => false
		}
	];

	return { actions, healFn, deleteFn, shareFn, hiddenFn };
}

describe('PalContainerToolbar', () => {
	beforeEach(() => setViewport(1440));

	it('renders a grid/list toggle with both options labelled', () => {
		const { actions } = makeActions();
		render(PalContainerToolbar, {
			viewMode: 'grid',
			selectionCount: 0,
			actions,
			onOpenFilter: vi.fn(),
			title: 'Palbox'
		});

		expect(screen.getByRole('button', { name: /grid view/i })).not.toBeNull();
		expect(screen.getByRole('button', { name: /list view/i })).not.toBeNull();
	});

	it('carries aria-pressed="true" on the active mode only', () => {
		const { actions } = makeActions();
		render(PalContainerToolbar, {
			viewMode: 'grid',
			selectionCount: 0,
			actions,
			onOpenFilter: vi.fn(),
			title: 'Palbox'
		});

		expect(screen.getByRole('button', { name: /grid view/i }).getAttribute('aria-pressed')).toBe(
			'true'
		);
		expect(screen.getByRole('button', { name: /list view/i }).getAttribute('aria-pressed')).toBe(
			'false'
		);
	});

	it('activating the other option updates the bound viewMode', async () => {
		const { actions } = makeActions();
		let viewMode: 'grid' | 'list' = 'grid';
		render(PalContainerToolbar, {
			get viewMode() {
				return viewMode;
			},
			set viewMode(value: 'grid' | 'list') {
				viewMode = value;
			},
			selectionCount: 0,
			actions,
			onOpenFilter: vi.fn(),
			title: 'Palbox'
		});

		await fireEvent.click(screen.getByRole('button', { name: /list view/i }));
		expect(viewMode).toBe('list');
	});

	it('the filter control is a labelled button that opens the filter', async () => {
		const { actions } = makeActions();
		const onOpenFilter = vi.fn();
		render(PalContainerToolbar, {
			viewMode: 'grid',
			selectionCount: 0,
			actions,
			onOpenFilter,
			title: 'Palbox'
		});

		const filterButton = screen.getByRole('button', { name: /^filter/i });
		expect(filterButton.tagName).toBe('BUTTON');
		await fireEvent.click(filterButton);
		expect(onOpenFilter).toHaveBeenCalledTimes(1);
	});

	it('omits the filter control when nothing is filterable', () => {
		const { actions } = makeActions();
		render(PalContainerToolbar, {
			viewMode: 'grid',
			selectionCount: 0,
			actions,
			filterable: false,
			onOpenFilter: vi.fn(),
			title: 'Palbox'
		});

		expect(screen.queryByRole('button', { name: /^filter/i })).toBeNull();
		expect(screen.getByRole('button', { name: /grid view/i })).not.toBeNull();
		expect(screen.getByRole('button', { name: /list view/i })).not.toBeNull();
	});

	it('shows text labels beside the icons at desktop width', () => {
		setViewport(1440);
		const { actions } = makeActions();
		render(PalContainerToolbar, {
			viewMode: 'grid',
			selectionCount: 0,
			actions,
			onOpenFilter: vi.fn(),
			title: 'Palbox'
		});

		expect(screen.getByText('Grid View')).not.toBeNull();
		expect(screen.getByText('List View')).not.toBeNull();
	});

	it('hides the text labels at phone width, keeping the accessible name', () => {
		setViewport(390);
		const { actions } = makeActions();
		render(PalContainerToolbar, {
			viewMode: 'grid',
			selectionCount: 0,
			actions,
			onOpenFilter: vi.fn(),
			title: 'Palbox'
		});

		expect(screen.queryByText('Grid View')).toBeNull();
		expect(screen.queryByText('List View')).toBeNull();
		expect(screen.getByRole('button', { name: /grid view/i })).not.toBeNull();
	});

	it('does not show the bulk-action bar when nothing is selected', () => {
		const { actions } = makeActions();
		render(PalContainerToolbar, {
			viewMode: 'grid',
			selectionCount: 0,
			actions,
			onOpenFilter: vi.fn(),
			title: 'Palbox'
		});

		expect(screen.queryByRole('button', { name: /heal selected/i })).toBeNull();
		expect(screen.queryByText(/selected/i)).toBeNull();
	});

	it('shows the bulk-action bar with the selection count once something is selected', () => {
		const { actions } = makeActions();
		render(PalContainerToolbar, {
			viewMode: 'grid',
			selectionCount: 3,
			actions,
			onOpenFilter: vi.fn(),
			title: 'Palbox'
		});

		expect(screen.getByText(/3.*selected/i)).not.toBeNull();
	});

	describe('bulk action wiring', () => {
		it('renders only the available descriptors, in their given order', async () => {
			const { actions } = makeActions();
			render(PalContainerToolbar, {
				viewMode: 'grid',
				selectionCount: 2,
				actions,
				onOpenFilter: vi.fn(),
				title: 'Palbox'
			});
			await tick();

			expect(screen.queryByRole('button', { name: /hidden action/i })).toBeNull();

			const rail = document.getElementById('pal-container-actions');
			expect(rail).not.toBeNull();
			const labels = within(rail as HTMLElement)
				.getAllByRole('button')
				.map((button) => button.getAttribute('aria-label'));

			// Unlike ActionSheet, ActionRail does not sort danger last.
			expect(labels).toEqual([
				'Heal selected',
				'Delete selected',
				'Clone to UPS',
				'Export to file'
			]);
		});

		it('invokes exactly the clicked descriptor, exactly once, for every available action', async () => {
			const { actions, healFn, deleteFn, shareFn, hiddenFn } = makeActions();
			render(PalContainerToolbar, {
				viewMode: 'grid',
				selectionCount: 2,
				actions,
				onOpenFilter: vi.fn(),
				title: 'Palbox'
			});
			await tick();

			await fireEvent.click(screen.getByRole('button', { name: /heal selected/i }));
			expect(healFn).toHaveBeenCalledTimes(1);
			expect(deleteFn).not.toHaveBeenCalled();
			expect(shareFn).not.toHaveBeenCalled();

			await fireEvent.click(screen.getByRole('button', { name: /delete selected/i }));
			expect(deleteFn).toHaveBeenCalledTimes(1);
			expect(deleteFn).toHaveBeenCalledWith('delete');
			expect(healFn).toHaveBeenCalledTimes(1);

			await fireEvent.click(screen.getByRole('button', { name: /clone to ups/i }));
			expect(shareFn).toHaveBeenCalledTimes(1);
			expect(shareFn).toHaveBeenNthCalledWith(1, 'clone');

			await fireEvent.click(screen.getByRole('button', { name: /export to file/i }));
			expect(shareFn).toHaveBeenCalledTimes(2);
			expect(shareFn).toHaveBeenNthCalledWith(2, 'export');

			expect(hiddenFn).not.toHaveBeenCalled();
			expect(healFn).toHaveBeenCalledTimes(1);
			expect(deleteFn).toHaveBeenCalledTimes(1);
		});
	});
});
