// @vitest-environment jsdom
import { getAppState } from '$states';
import type { GamePalJson, GamePlayerJson } from '$states/gameState.svelte';
import type { Player } from '$types';
import { fireEvent, render, screen, within } from '@testing-library/svelte';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import '../../pal/__tests__/fixtures/matchMediaPolyfill';
import LivePlayerDetail from '../LivePlayerDetail.svelte';
import { clearFriendship, seedFriendship } from './fixtures/friendshipFixture';
import './fixtures/animatePolyfill';

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

const handlers = {
	onHeal: vi.fn(),
	onHealAll: vi.fn(),
	onSetLevel: vi.fn(async () => true),
	onSetItemSlot: vi.fn(),
	onPalsPageChange: vi.fn()
};

function player(overrides: Partial<GamePlayerJson> = {}): GamePlayerJson {
	return { uid: 'p1', nickname: 'Aurora', status: 'ok', ...overrides };
}

beforeEach(() => {
	getAppState().selectedPlayer = undefined;
});

describe('LivePlayerDetail level cap', () => {
	it('caps the badge at the live player level', () => {
		render(LivePlayerDetail, {
			props: { player: player({ level: 12 }), pals: [gamePal()], ...handlers }
		});

		expect(screen.getByText('lvl 12')).toBeTruthy();
	});

	it('never borrows the save file player level when the game omits one', () => {
		getAppState().selectedPlayer = { level: 10 } as Player;

		render(LivePlayerDetail, {
			props: { player: player({ level: undefined }), pals: [gamePal()], ...handlers }
		});

		expect(screen.queryByText('lvl 10')).toBeNull();
		expect(screen.queryByText(/^lvl /)).toBeNull();
	});
});

describe('LivePlayerDetail tabs', () => {
	it('opens on the loadout and reveals the pal grid only on the palbox tab', async () => {
		const { container } = render(LivePlayerDetail, {
			props: { player: player({ level: 12 }), pals: [gamePal()], ...handlers }
		});

		const loadout = container.querySelector('#live-loadout-panel')!.parentElement!;
		const palbox = container.querySelector('#live-palbox-panel')!.parentElement!;
		expect(loadout.hasAttribute('hidden')).toBe(false);
		expect(palbox.hasAttribute('hidden')).toBe(true);

		await fireEvent.click(screen.getByText('Palbox'));

		expect(loadout.hasAttribute('hidden')).toBe(true);
		expect(palbox.hasAttribute('hidden')).toBe(false);
		expect(within(palbox as HTMLElement).getByText('Sparky')).toBeTruthy();
	});
});

describe('LivePlayerDetail heal progress', () => {
	it('marks only the pal whose heal is in flight as busy', () => {
		const { container } = render(LivePlayerDetail, {
			props: {
				player: player({ level: 12 }),
				pals: [gamePal(), gamePal({ instanceId: 'pal-2', nickname: 'Ember', slotIndex: 1 })],
				healingPalId: 'pal-2',
				...handlers
			}
		});

		expect(screen.getByAltText('Loading')).toBeTruthy();
		expect(container.querySelectorAll('[aria-busy="true"]')).toHaveLength(1);
	});

	it('shows no progress on any pal while nothing is healing', () => {
		render(LivePlayerDetail, {
			props: { player: player({ level: 12 }), pals: [gamePal()], ...handlers }
		});

		expect(screen.queryByAltText('Loading')).toBeNull();
	});
});

async function openPalbox(container: HTMLElement) {
	await fireEvent.click(screen.getByText('Palbox'));
	return container.querySelector('#live-palbox-panel') as HTMLElement;
}

describe('LivePlayerDetail box slots', () => {
	it('seats each pal at the slot the game reported and fills the rest of the page', async () => {
		const { container } = render(LivePlayerDetail, {
			props: {
				player: player({ level: 12 }),
				pals: [gamePal({ slotIndex: 3 })],
				palsSlotCount: 6,
				...handlers
			}
		});

		const palbox = await openPalbox(container);
		const badges = palbox.querySelectorAll('[role="button"]');

		expect(badges).toHaveLength(6);
		expect(within(badges[3] as HTMLElement).getByAltText('Foxparks')).toBeTruthy();
		expect(within(palbox).getAllByText('Sparky')).toHaveLength(1);
	});

	it('seats a later page from its own base rather than the storage-wide index', async () => {
		const { container } = render(LivePlayerDetail, {
			props: {
				player: player({ level: 12 }),
				pals: [gamePal({ slotIndex: 32 })],
				palsPage: 1,
				palsPageCount: 32,
				palsSlotCount: 30,
				palsSlotBase: 30,
				...handlers
			}
		});

		const palbox = await openPalbox(container);
		const badges = palbox.querySelectorAll('[role="button"]');

		expect(badges).toHaveLength(30);
		expect(within(badges[2] as HTMLElement).getByAltText('Foxparks')).toBeTruthy();
	});

	it('never drops a pal seated past the reported slot count', async () => {
		const { container } = render(LivePlayerDetail, {
			props: {
				player: player({ level: 12 }),
				pals: [gamePal({ slotIndex: 9 })],
				palsSlotCount: 4,
				...handlers
			}
		});

		const palbox = await openPalbox(container);

		expect(palbox.querySelectorAll('[role="button"]')).toHaveLength(10);
		expect(within(palbox).getByText('Sparky')).toBeTruthy();
	});
});

describe('LivePlayerDetail palbox pager', () => {
	it('asks for the page the user picked', async () => {
		const onPalsPageChange = vi.fn();
		const { container } = render(LivePlayerDetail, {
			props: {
				player: player({ level: 12 }),
				pals: [gamePal()],
				palsPage: 0,
				palsPageCount: 4,
				palsSlotCount: 1,
				...handlers,
				onPalsPageChange
			}
		});

		const palbox = await openPalbox(container);
		await fireEvent.click(within(palbox).getByText('3'));

		expect(onPalsPageChange).toHaveBeenCalledWith(2);
	});

	it('wraps past the last page back to the first', async () => {
		const onPalsPageChange = vi.fn();
		const { container } = render(LivePlayerDetail, {
			props: {
				player: player({ level: 12 }),
				pals: [gamePal()],
				palsPage: 3,
				palsPageCount: 4,
				palsSlotCount: 1,
				...handlers,
				onPalsPageChange
			}
		});

		const palbox = await openPalbox(container);
		await fireEvent.click(within(palbox).getByAltText('Next'));

		expect(onPalsPageChange).toHaveBeenCalledWith(0);
	});

	it('shows no pager for a box that fits on one page', async () => {
		const { container } = render(LivePlayerDetail, {
			props: {
				player: player({ level: 12 }),
				pals: [gamePal()],
				palsPage: 0,
				palsPageCount: 1,
				palsSlotCount: 1,
				...handlers
			}
		});

		const palbox = await openPalbox(container);

		expect(palbox.querySelector('#live-palbox-pager')).toBeNull();
	});
});

describe('LivePlayerDetail pal actions toolbar', () => {
	it('offers heal all from the toolbar beside the grid', async () => {
		const onHealAll = vi.fn();
		const { container } = render(LivePlayerDetail, {
			props: {
				player: player({ level: 12 }),
				pals: [gamePal()],
				palsSlotCount: 1,
				...handlers,
				onHealAll
			}
		});

		const palbox = await openPalbox(container);
		const toolbar = palbox.querySelector('#live-pal-actions') as HTMLElement;

		await fireEvent.click(within(toolbar).getByRole('button'));

		expect(onHealAll).toHaveBeenCalledTimes(1);
	});

	it('disables heal all and names the reason when the capability is unavailable', async () => {
		const { container } = render(LivePlayerDetail, {
			props: {
				player: player({ level: 12 }),
				pals: [gamePal()],
				palsSlotCount: 1,
				healReason: 'World not loaded',
				...handlers
			}
		});

		const palbox = await openPalbox(container);
		const toolbar = palbox.querySelector('#live-pal-actions') as HTMLElement;

		expect(within(toolbar).getByRole('button')).toHaveProperty('disabled', true);

		await fireEvent.mouseEnter(toolbar.firstElementChild as HTMLElement);
		expect(await screen.findByText('World not loaded')).toBeTruthy();
	});
});
