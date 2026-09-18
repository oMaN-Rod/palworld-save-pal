// @vitest-environment jsdom
import '../../pal/__tests__/fixtures/matchMediaPolyfill';
import './fixtures/animatePolyfill';
import { render, screen, fireEvent, waitFor } from '@testing-library/svelte';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

vi.mock('$components/pal/PalModelViewer.svelte', async () => ({
	default: (await import('./fixtures/PalModelViewerStub.svelte')).default
}));

import LivePalEditModal from '../LivePalEditModal.svelte';
import { expData } from '$lib/data';
import { clearFriendship, seedFriendship } from './fixtures/friendshipFixture';
import type { GamePalDetailJson, GamePalJson } from '$states/gameState.svelte';

function seedExp(): void {
	expData.expData = Object.fromEntries(
		Array.from({ length: 81 }, (_, level) => [
			String(level + 1),
			{
				DropEXP: 0,
				NextEXP: 100,
				PalNextEXP: 100,
				TotalEXP: level * 100,
				PalTotalEXP: level * 100,
				BuildEXP: 0,
				CraftEXP: 0,
				PalBuildEXP: 0,
				PalCraftEXP: 0
			}
		])
	);
}

function makeGamePal(overrides: Partial<GamePalJson> = {}): GamePalJson {
	return {
		characterId: 'TestPal',
		instanceId: 'pal-1',
		ownerUid: 'player-1',
		slotIndex: 12,
		nickname: 'Fluffy',
		level: 15,
		gender: 'male',
		...overrides
	};
}

function makeDetail(overrides: Partial<GamePalDetailJson> = {}): GamePalDetailJson {
	return {
		...makeGamePal(),
		status: 'ok',
		exp: 4200,
		rank: 3,
		rankHp: 4,
		rankAttack: 5,
		rankDefense: 6,
		rankCraftSpeed: 7,
		talentHp: 60,
		talentShot: 61,
		talentDefense: 62,
		equipWaza: ['EPalWazaID::FireBall'],
		passiveSkills: ['Legend'],
		workSuitability: { Handcraft: 2 },
		sanity: 88,
		stomach: 120,
		friendshipPoint: 900,
		...overrides
	};
}

function save(container: HTMLElement) {
	return fireEvent.click(container.querySelector('[data-modal-primary]') as HTMLElement);
}

function cancel(container: HTMLElement) {
	const rail = container.querySelector('#live-pal-quick-actions') as HTMLElement;
	const buttons = Array.from(rail.querySelectorAll('button'));
	return fireEvent.click(buttons[buttons.length - 1]);
}

describe('LivePalEditModal', () => {
	beforeEach(() => {
		seedFriendship();
		seedExp();
	});

	afterEach(() => {
		clearFriendship();
	});

	it('sends nothing when the user saves without changing anything', async () => {
		const closeModal = vi.fn();
		const { container } = render(LivePalEditModal, {
			props: { pal: makeGamePal(), detail: makeDetail(), levelCap: 45, closeModal }
		});

		await save(container);

		await waitFor(() => expect(closeModal).toHaveBeenCalled());
		expect(closeModal.mock.calls[0][0]).toEqual({});
	});

	it('sends undefined when the edit is cancelled, even after a change', async () => {
		const closeModal = vi.fn();
		const { container } = render(LivePalEditModal, {
			props: { pal: makeGamePal(), detail: makeDetail(), levelCap: 45, closeModal }
		});

		await cancel(container);

		await waitFor(() => expect(closeModal).toHaveBeenCalledWith(undefined));
	});

	it('marks a pal above the player level as synced rather than refusing it', () => {
		const { container } = render(LivePalEditModal, {
			props: {
				pal: makeGamePal({ level: 60 }),
				detail: makeDetail({ level: 60 }),
				levelCap: 45,
				closeModal: vi.fn()
			}
		});

		expect(container.querySelector('.text-error-500')).toBeTruthy();
	});

	it('leaves a pal at or below the player level untinted', () => {
		const { container } = render(LivePalEditModal, {
			props: { pal: makeGamePal(), detail: makeDetail(), levelCap: 45, closeModal: vi.fn() }
		});

		expect(container.querySelector('.text-error-500')).toBeNull();
	});

	it('offers no species toggles, which a live pal cannot be re-keyed by', () => {
		render(LivePalEditModal, {
			props: { pal: makeGamePal(), detail: makeDetail(), levelCap: 45, closeModal: vi.fn() }
		});

		expect(screen.queryByRole('button', { name: /alpha/i })).toBeNull();
		expect(screen.queryByRole('button', { name: /lucky/i })).toBeNull();
		expect(screen.getByRole('button', { name: /awakened/i })).toBeTruthy();
	});

	it('sends only the field that changed', async () => {
		const closeModal = vi.fn();
		const { container } = render(LivePalEditModal, {
			props: { pal: makeGamePal(), detail: makeDetail(), levelCap: 45, closeModal }
		});

		const awakened = screen.getByRole('button', { name: /awakened/i });
		await fireEvent.click(awakened);
		await save(container);

		await waitFor(() => expect(closeModal).toHaveBeenCalled());
		expect(closeModal.mock.calls[0][0]).toEqual({ isAwakened: true });
	});

	it('sends a gender change in the spelling the game reports', async () => {
		const closeModal = vi.fn();
		const { container } = render(LivePalEditModal, {
			props: {
				pal: makeGamePal({ gender: 'male' }),
				detail: makeDetail({ gender: 'male' }),
				levelCap: 45,
				closeModal
			}
		});

		await fireEvent.click(screen.getByAltText('Male').closest('button') as HTMLElement);
		await save(container);

		await waitFor(() => expect(closeModal).toHaveBeenCalled());
		expect(closeModal.mock.calls[0][0]).toEqual({ gender: 'female' });
	});

	it('shows the pal it was handed rather than an empty form', () => {
		render(LivePalEditModal, {
			props: { pal: makeGamePal(), detail: makeDetail(), levelCap: 45, closeModal: vi.fn() }
		});

		expect(screen.getByText('Fluffy')).toBeTruthy();
		expect(screen.getAllByText('TestPal').length).toBeGreaterThan(0);
	});
});
