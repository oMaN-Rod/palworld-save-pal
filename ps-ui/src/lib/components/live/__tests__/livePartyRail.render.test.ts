// @vitest-environment jsdom
import { getAppState } from '$states';
import type { GamePalJson } from '$states/gameState.svelte';
import { render, screen, within } from '@testing-library/svelte';
import userEvent from '@testing-library/user-event';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import '../../pal/__tests__/fixtures/matchMediaPolyfill';
import LivePartyRail from '../LivePartyRail.svelte';
import './fixtures/animatePolyfill';
import { clearFriendship, seedFriendship } from './fixtures/friendshipFixture';

function gamePal(overrides: Partial<GamePalJson> = {}): GamePalJson {
	return {
		characterId: 'Foxparks',
		instanceId: 'pal-1',
		ownerUid: 'p1',
		slotIndex: 0,
		nickname: 'Sparky',
		level: 20,
		...overrides
	};
}

function rail(): HTMLElement {
	return document.querySelector('#live-party-rail') as HTMLElement;
}

beforeEach(() => {
	getAppState().selectedPlayer = undefined;
	seedFriendship();
});

afterEach(() => {
	clearFriendship();
});

describe('LivePartyRail', () => {
	it('seats a pal at the slot it reports rather than in order', () => {
		render(LivePartyRail, {
			props: {
				party: [
					gamePal({ characterId: 'Pengullet', instanceId: 'otomo-2', slotIndex: 3 }),
					gamePal({ characterId: 'Foxparks', instanceId: 'otomo-1', slotIndex: 0 })
				],
				partyReadable: true,
				levelCap: 30
			}
		});

		const seats = within(rail()).getAllByRole('img');
		expect(seats[0].getAttribute('alt')).toBe('Foxparks');
		expect(seats[1].getAttribute('alt')).toBe('Pengullet');
	});

	it('caps a pal at the live player level', () => {
		render(LivePartyRail, {
			props: {
				party: [gamePal({ instanceId: 'otomo-1', level: 40 })],
				partyReadable: true,
				levelCap: 8
			}
		});

		expect(within(rail()).getByText('8')).toBeTruthy();
		expect(within(rail()).queryByText('40')).toBeNull();
	});

	it('tells an unreadable party apart from an empty one', () => {
		render(LivePartyRail, { props: { party: [], partyReadable: false } });

		expect(within(rail()).queryAllByRole('img')).toHaveLength(0);
	});

	it('opens the editor for the seat that was clicked', async () => {
		const onEdit = vi.fn();
		const user = userEvent.setup();
		render(LivePartyRail, {
			props: {
				party: [gamePal({ instanceId: 'otomo-1' })],
				partyReadable: true,
				levelCap: 30,
				onEdit
			}
		});

		await user.click(within(rail()).getByRole('button', { name: /Sparky/ }));

		expect(onEdit).toHaveBeenCalledWith(expect.objectContaining({ instanceId: 'otomo-1' }));
	});

	it('refuses to edit while a reason says it cannot', async () => {
		const onEdit = vi.fn();
		const user = userEvent.setup();
		render(LivePartyRail, {
			props: {
				party: [gamePal({ instanceId: 'otomo-1' })],
				partyReadable: true,
				levelCap: 30,
				editDisabledReason: 'Another change is still running.',
				onEdit
			}
		});

		await user.click(within(rail()).getByRole('button', { name: /Sparky/ }));

		expect(onEdit).not.toHaveBeenCalled();
	});

	it('offers the picker for an empty seat', async () => {
		const onAdd = vi.fn();
		const user = userEvent.setup();
		render(LivePartyRail, {
			props: {
				party: [gamePal({ instanceId: 'otomo-1', slotIndex: 0 })],
				partyReadable: true,
				levelCap: 30,
				onAdd
			}
		});

		await user.click(within(rail()).getByRole('button', { name: /Add Pal .* slot 2$/ }));

		expect(onAdd).toHaveBeenCalledWith(1);
	});

	it('leaves an empty seat inert when there is nothing to add with', () => {
		render(LivePartyRail, {
			props: { party: [gamePal({ instanceId: 'otomo-1' })], partyReadable: true, levelCap: 30 }
		});

		expect(within(rail()).queryByRole('button', { name: /Add Pal/ })).toBeNull();
	});

	it('refuses to add while a reason says it cannot', async () => {
		const onAdd = vi.fn();
		const user = userEvent.setup();
		render(LivePartyRail, {
			props: {
				party: [],
				partyReadable: true,
				levelCap: 30,
				addDisabledReason: 'Another change is still running.',
				onAdd
			}
		});

		await user.click(within(rail()).getAllByRole('button', { name: /Add Pal/ })[0]);

		expect(onAdd).not.toHaveBeenCalled();
	});

	it('offers the picker for an empty seat in the expanded pane too', async () => {
		const onAdd = vi.fn();
		const user = userEvent.setup();
		render(LivePartyRail, {
			props: { party: [], partyReadable: true, levelCap: 30, expanded: true, onAdd }
		});

		const pane = document.querySelector('#live-party') as HTMLElement;
		await user.click(within(pane).getAllByRole('button', { name: /Add Pal .* slot 1$/ })[0]);

		expect(onAdd).toHaveBeenCalledWith(0);
	});

	it('swaps the rail for the full party pane when expanded', async () => {
		const user = userEvent.setup();
		render(LivePartyRail, {
			props: {
				party: [gamePal({ characterId: 'Pengullet', instanceId: 'otomo-1' })],
				partyReadable: true,
				levelCap: 30
			}
		});

		await user.click(screen.getByRole('button', { name: 'Expand party' }));

		expect(rail()).toBeNull();
		const pane = document.querySelector('#live-party') as HTMLElement;
		expect(within(pane).getAllByAltText('Pengullet')).not.toHaveLength(0);
	});

	it('returns to the rail when collapsed again', async () => {
		const user = userEvent.setup();
		render(LivePartyRail, {
			props: { party: [gamePal()], partyReadable: true, levelCap: 30, expanded: true }
		});

		await user.click(screen.getByRole('button', { name: 'Collapse party' }));

		expect(rail()).toBeTruthy();
	});
});
