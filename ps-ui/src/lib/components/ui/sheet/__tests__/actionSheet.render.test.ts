// @vitest-environment jsdom
import { render, screen, within } from '@testing-library/svelte';
import userEvent from '@testing-library/user-event';
import { tick } from 'svelte';
import { describe, expect, it, vi } from 'vitest';

import type { ActionDescriptor } from '$components/ui/actions/actionDescriptor';

import ActionSheet from '../ActionSheet.svelte';

function actions(overrides: Partial<ActionDescriptor>[] = []): ActionDescriptor[] {
	const base: ActionDescriptor[] = [
		{
			id: 'quantity',
			label: 'Set quantity',
			icon: 'tabler:hash',
			run: vi.fn(),
			detail: () => '12'
		},
		{ id: 'replace', label: 'Replace item', icon: 'tabler:plus', run: vi.fn() },
		{ id: 'clear', label: 'Clear slot', icon: 'tabler:trash', run: vi.fn(), danger: true }
	];
	return base.map((action, index) => ({ ...action, ...overrides[index] }));
}

describe('ActionSheet', () => {
	it('renders one button per available action, labelled', async () => {
		render(ActionSheet, { open: true, title: 'Pal Sphere', actions: actions(), onClose: vi.fn() });
		await tick();

		expect(screen.getByRole('button', { name: /set quantity/i })).not.toBeNull();
		expect(screen.getByRole('button', { name: /replace item/i })).not.toBeNull();
		expect(screen.getByRole('button', { name: /clear slot/i })).not.toBeNull();
	});

	it('omits actions that are unavailable', async () => {
		const list = actions([{}, { available: () => false }]);
		render(ActionSheet, { open: true, title: 'Pal Sphere', actions: list, onClose: vi.fn() });
		await tick();

		expect(screen.queryByRole('button', { name: /replace item/i })).toBeNull();
	});

	it('shows an action detail value', async () => {
		render(ActionSheet, { open: true, title: 'Pal Sphere', actions: actions(), onClose: vi.fn() });
		await tick();

		const row = screen.getByRole('button', { name: /set quantity/i });
		expect(within(row).getByText('12')).not.toBeNull();
	});

	it('runs the action and closes when a row is activated', async () => {
		const onClose = vi.fn();
		const list = actions();
		render(ActionSheet, { open: true, title: 'Pal Sphere', actions: list, onClose });
		await tick();

		await userEvent.click(screen.getByRole('button', { name: /replace item/i }));

		expect(list[1].run).toHaveBeenCalledTimes(1);
		expect(onClose).toHaveBeenCalledTimes(1);
	});

	it('still closes when the action rejects, without swallowing the rejection', async () => {
		const onClose = vi.fn();
		const error = new Error('boom');
		const list = actions([{ run: vi.fn().mockRejectedValue(error) }]);

		const caught: unknown[] = [];
		const onRejection = (reason: unknown) => caught.push(reason);
		process.on('unhandledRejection', onRejection);

		try {
			render(ActionSheet, { open: true, title: 'Pal Sphere', actions: list, onClose });
			await tick();

			await userEvent.click(screen.getByRole('button', { name: /set quantity/i }));

			await vi.waitFor(() => {
				expect(caught).toContain(error);
			});
			expect(onClose).toHaveBeenCalledTimes(1);
		} finally {
			process.off('unhandledRejection', onRejection);
		}
	});

	it('renders the subtitle when given one', async () => {
		render(ActionSheet, {
			open: true,
			title: 'Pal Sphere',
			subtitle: 'Slot 1',
			actions: actions(),
			onClose: vi.fn()
		});
		await tick();

		expect(screen.getByText('Slot 1')).not.toBeNull();
	});

	it('sorts danger actions last while preserving declared order otherwise', async () => {
		const list: ActionDescriptor[] = [
			{ id: 'clear', label: 'Clear slot', icon: 'tabler:trash', run: vi.fn(), danger: true },
			{ id: 'quantity', label: 'Set quantity', icon: 'tabler:hash', run: vi.fn() },
			{ id: 'favorite', label: 'Favorite', icon: 'tabler:star', run: vi.fn(), danger: true },
			{ id: 'replace', label: 'Replace item', icon: 'tabler:plus', run: vi.fn() },
			{ id: 'move', label: 'Move item', icon: 'tabler:arrows-move', run: vi.fn() }
		];
		render(ActionSheet, { open: true, title: 'Pal Sphere', actions: list, onClose: vi.fn() });
		await tick();

		const labels = screen
			.getAllByRole('button')
			.map((button) => button.textContent?.trim())
			.filter(Boolean);

		expect(labels).toEqual(['Set quantity', 'Replace item', 'Move item', 'Clear slot', 'Favorite']);
	});
});
