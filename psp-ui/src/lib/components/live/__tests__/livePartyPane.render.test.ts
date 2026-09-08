// @vitest-environment jsdom
import { getAppState } from '$states';
import type { GamePalJson } from '$states/gameState.svelte';
import { render, within } from '@testing-library/svelte';
import { afterEach, beforeEach, describe, expect, it } from 'vitest';
import '../../pal/__tests__/fixtures/matchMediaPolyfill';
import LivePartyPane from '../LivePartyPane.svelte';
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

beforeEach(() => {
	getAppState().selectedPlayer = undefined;
	seedFriendship();
});

afterEach(() => {
	clearFriendship();
});

describe('LivePartyPane', () => {
	it('shows the party pals it was given', () => {
		const { container } = render(LivePartyPane, {
			props: {
				party: [
					gamePal({ characterId: 'Foxparks', instanceId: 'otomo-1', slotIndex: 0 }),
					gamePal({ characterId: 'Pengullet', instanceId: 'otomo-2', slotIndex: 1 })
				],
				partyReadable: true,
				levelCap: 12
			}
		});

		const party = container.querySelector('#live-party') as HTMLElement;
		expect(within(party).getAllByAltText('Foxparks')).not.toHaveLength(0);
		expect(within(party).getAllByAltText('Pengullet')).not.toHaveLength(0);
	});

	it('tells an unreadable party apart from an empty one', () => {
		const { getByText } = render(LivePartyPane, {
			props: { party: [], partyReadable: false }
		});

		expect(getByText('Party is readable only while the player is in the world.')).toBeTruthy();
	});

	it('caps a party pal at the live player level with no save loaded', () => {
		const { container } = render(LivePartyPane, {
			props: {
				party: [gamePal({ instanceId: 'otomo-1', level: 40 })],
				partyReadable: true,
				levelCap: 8
			}
		});

		const party = container.querySelector('#live-party') as HTMLElement;
		expect(within(party).getByText('8')).toBeTruthy();
		expect(within(party).queryByText('40')).toBeNull();
	});

	it('draws every seat, filled or not', () => {
		const { container } = render(LivePartyPane, {
			props: {
				party: [gamePal({ instanceId: 'otomo-1', slotIndex: 0 })],
				partyReadable: true,
				levelCap: 12
			}
		});

		const party = container.querySelector('#live-party') as HTMLElement;
		expect(party.children).toHaveLength(5);
	});

	it('seats a pal at the slot it reports rather than in order', () => {
		const { container } = render(LivePartyPane, {
			props: {
				party: [
					gamePal({ characterId: 'Pengullet', instanceId: 'otomo-2', slotIndex: 3 }),
					gamePal({ characterId: 'Foxparks', instanceId: 'otomo-1', slotIndex: 0 })
				],
				partyReadable: true,
				levelCap: 12
			}
		});

		const party = container.querySelector('#live-party') as HTMLElement;
		const seats = Array.from(party.children);
		expect(within(seats[0] as HTMLElement).getAllByAltText('Foxparks')).not.toHaveLength(0);
		expect(within(seats[3] as HTMLElement).getAllByAltText('Pengullet')).not.toHaveLength(0);
		expect(within(seats[1] as HTMLElement).queryByAltText('Pengullet')).toBeNull();
	});
});
