// @vitest-environment jsdom
import { render, screen, within } from '@testing-library/svelte';
import userEvent from '@testing-library/user-event';
import { describe, expect, it, vi } from 'vitest';
import '../../pal/__tests__/fixtures/matchMediaPolyfill';
import './fixtures/animatePolyfill';
import type { GameBasePalsJson, GamePalJson } from '$states/gameState.svelte';
import LiveGuildBasePals from '../LiveGuildBasePals.svelte';

const bases = [
	{ id: 'b1', name: 'Ironworks Prime', level: 20, containerId: 'bc1', palSlotNum: 8 },
	{ id: 'b2', name: 'Ore Shelf', level: 14, containerId: 'bc2', palSlotNum: 8 }
];

function basePals(overrides: Partial<GameBasePalsJson> = {}): GameBasePalsJson {
	return {
		baseId: 'b1',
		containerId: 'bc1',
		slotNum: 8,
		pals: [
			{ characterId: 'Lamball', instanceId: 'p1', ownerUid: 'o', slotIndex: 0, level: 4 },
			{ characterId: 'Cattiva', instanceId: 'p2', ownerUid: 'o', slotIndex: 3, level: 9 }
		] as GamePalJson[],
		status: 'ok',
		...overrides
	};
}

const SLOT_BUTTON = /— slot \d+$/;

describe('LiveGuildBasePals', () => {
	it('draws one cell per slot the base reports, filled and empty alike', async () => {
		render(LiveGuildBasePals, { bases, basePals: basePals() });

		const grid = await screen.findByTestId('live-base-pals');
		expect(within(grid).getAllByRole('button', { name: SLOT_BUTTON })).toHaveLength(8);
	});

	it('names the base the grid belongs to', async () => {
		render(LiveGuildBasePals, { bases, basePals: basePals() });

		expect(await screen.findByText('Ironworks Prime')).toBeTruthy();
	});

	it('shows an empty grid while the reply still belongs to another base', async () => {
		render(LiveGuildBasePals, { bases, basePage: 1, basePals: basePals() });

		const grid = await screen.findByTestId('live-base-pals');
		expect(within(grid).queryByText('Lamball')).toBeNull();
	});

	it('sizes the grid from the base when the reply carries no slot count', async () => {
		render(LiveGuildBasePals, { bases, basePals: basePals({ slotNum: null }) });

		const grid = await screen.findByTestId('live-base-pals');
		expect(within(grid).getAllByRole('button', { name: SLOT_BUTTON })).toHaveLength(8);
	});

	it('opens the editor for the pal in the slot that was clicked', async () => {
		const onEditPal = vi.fn();
		const user = userEvent.setup();
		render(LiveGuildBasePals, { bases, basePals: basePals(), onEditPal });

		const grid = await screen.findByTestId('live-base-pals');
		await user.click(within(grid).getAllByRole('button', { name: SLOT_BUTTON })[0]);

		expect(onEditPal).toHaveBeenCalledWith(expect.objectContaining({ instanceId: 'p1' }), 0);
	});

	it('offers the picker for an empty slot rather than the editor', async () => {
		const onEditPal = vi.fn();
		const onAddPal = vi.fn();
		const user = userEvent.setup();
		render(LiveGuildBasePals, { bases, basePals: basePals(), onEditPal, onAddPal });

		const grid = await screen.findByTestId('live-base-pals');
		await user.click(within(grid).getAllByRole('button', { name: SLOT_BUTTON })[1]);

		expect(onAddPal).toHaveBeenCalledWith(1);
		expect(onEditPal).not.toHaveBeenCalled();
	});

	it('acts on neither click while a reason says it cannot', async () => {
		const onEditPal = vi.fn();
		const onAddPal = vi.fn();
		const user = userEvent.setup();
		render(LiveGuildBasePals, {
			bases,
			basePals: basePals(),
			onEditPal,
			onAddPal,
			editDisabledReason: 'Another change is still running.',
			addDisabledReason: 'Another change is still running.'
		});

		const grid = await screen.findByTestId('live-base-pals');
		const slots = within(grid).getAllByRole('button', { name: SLOT_BUTTON });
		await user.click(slots[0]);
		await user.click(slots[1]);

		expect(onEditPal).not.toHaveBeenCalled();
		expect(onAddPal).not.toHaveBeenCalled();
	});
});
