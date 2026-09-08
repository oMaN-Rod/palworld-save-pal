// @vitest-environment jsdom
import '../../pal/__tests__/fixtures/matchMediaPolyfill';
import './fixtures/animatePolyfill';
import { render, screen, fireEvent } from '@testing-library/svelte';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import LivePalBadge from '../LivePalBadge.svelte';
import PalBadge from '../../pal/PalBadge.svelte';
import { getAppState, getPalEditorState } from '$states';
import { EntryState, PalGender, type Pal, type WorkSuitability } from '$types';
import type { GamePalJson } from '$states/gameState.svelte';

function makeGamePal(overrides: Partial<GamePalJson> = {}): GamePalJson {
	return {
		characterId: 'TestPal',
		instanceId: 'pal-1',
		ownerUid: 'player-1',
		slotIndex: 0,
		nickname: 'Fluffy',
		level: 15,
		...overrides
	};
}

function makePal(overrides: Partial<Pal> = {}): Pal {
	return {
		name: 'Testpal',
		instance_id: 'pal-1',
		character_id: 'TestPal',
		character_key: 'testpal',
		is_lucky: false,
		is_boss: false,
		is_predator: false,
		is_awakened: false,
		is_imported: false,
		friendship_point: 0,
		gender: PalGender.MALE,
		rank_hp: 0,
		rank_attack: 0,
		rank_defense: 0,
		rank_craftspeed: 0,
		talent_hp: 0,
		talent_shot: 0,
		talent_defense: 0,
		rank: 1,
		level: 15,
		is_tower: false,
		stomach: 0,
		storage_slot: 0,
		learned_skills: [],
		active_skills: [],
		passive_skills: [],
		work_suitability: {} as Record<WorkSuitability, number>,
		hp: 0,
		max_hp: 0,
		elements: [],
		state: EntryState.NONE,
		sanity: 0,
		exp: 0,
		is_sick: false,
		...overrides
	};
}

const noop = () => {};

beforeEach(() => {
	vi.spyOn(getAppState(), 'saveState').mockResolvedValue(undefined);
	getPalEditorState().close();
});

afterEach(() => {
	getPalEditorState().close();
	vi.restoreAllMocks();
});

describe('LivePalBadge rendering', () => {
	it('renders the pal nickname and level', () => {
		render(LivePalBadge, { props: { pal: makeGamePal() } });

		expect(screen.getByText('Fluffy')).toBeTruthy();
		expect(screen.getByText('lvl 15')).toBeTruthy();
	});

	it('degrades without throwing when nickname and level are absent', () => {
		expect(() =>
			render(LivePalBadge, {
				props: { pal: makeGamePal({ nickname: undefined, level: undefined }) }
			})
		).not.toThrow();

		expect(screen.getByText('TestPal')).toBeTruthy();
		expect(screen.queryByText(/^lvl /)).toBeNull();
	});
});

describe('LivePalBadge click ownership', () => {
	it('edits the pal it holds and never opens the save editor', async () => {
		const onEdit = vi.fn();
		const { container } = render(LivePalBadge, {
			props: { pal: makeGamePal(), slotIndex: 4, onEdit }
		});

		const button = container.querySelector('button') as HTMLElement;
		await fireEvent.click(button);

		expect(onEdit).toHaveBeenCalledTimes(1);
		expect(onEdit.mock.calls[0][1]).toBe(4);
		expect(getPalEditorState().isOpen).toBe(false);
	});

	it('adds a pal when the slot is empty, without needing the context menu', async () => {
		const onAdd = vi.fn();
		const onEdit = vi.fn();
		const { container } = render(LivePalBadge, {
			props: { pal: undefined, slotIndex: 7, onAdd, onEdit }
		});

		await fireEvent.click(container.querySelector('button') as HTMLElement);

		expect(onAdd).toHaveBeenCalledWith(7);
		expect(onEdit).not.toHaveBeenCalled();
	});

	it('sends a click to the armed move rather than to edit or add', async () => {
		const onMove = vi.fn();
		const onEdit = vi.fn();
		const { container } = render(LivePalBadge, {
			props: { pal: makeGamePal(), slotIndex: 2, movePending: true, onMove, onEdit }
		});

		await fireEvent.click(container.querySelector('button') as HTMLElement);

		expect(onMove).toHaveBeenCalledWith(2);
		expect(onEdit).not.toHaveBeenCalled();
	});

	it('does nothing on click while the action is unavailable', async () => {
		const onEdit = vi.fn();
		const { container } = render(LivePalBadge, {
			props: { pal: makeGamePal(), slotIndex: 1, onEdit, palEditDisabledReason: 'World is not loaded' }
		});

		await fireEvent.click(container.querySelector('button') as HTMLElement);

		expect(onEdit).not.toHaveBeenCalled();
	});

	it('proves the guard is load-bearing: an un-disabled PalBadge click does open the save editor', async () => {
		render(PalBadge, {
			props: { pal: makePal(), onMove: noop, onAdd: noop, onClone: noop, onDelete: noop }
		});

		const button = screen.getByRole('button');
		await fireEvent.click(button);

		expect(getPalEditorState().isOpen).toBe(true);
	});
});

describe('LivePalBadge heal action', () => {
	it('disables heal with a visible reason when healDisabledReason is set', async () => {
		const onHeal = vi.fn();
		const { container } = render(LivePalBadge, {
			props: { pal: makeGamePal(), onHeal, healDisabledReason: 'World is not loaded' }
		});

		const button = container.querySelector('button') as HTMLElement;
		await fireEvent.contextMenu(button);
		await new Promise((resolve) => setTimeout(resolve, 0));

		const healItem = await screen.findByRole('button', { name: /World is not loaded/ });
		await fireEvent.click(healItem);

		expect(onHeal).not.toHaveBeenCalled();
	});

	it('heals when the capability is available', async () => {
		const onHeal = vi.fn();
		const gamePal = makeGamePal();
		const { container } = render(LivePalBadge, {
			props: { pal: gamePal, onHeal }
		});

		const button = container.querySelector('button') as HTMLElement;
		await fireEvent.contextMenu(button);
		await new Promise((resolve) => setTimeout(resolve, 0));

		const healItem = await screen.findByRole('button', { name: 'Heal' });
		await fireEvent.click(healItem);

		expect(onHeal).toHaveBeenCalledTimes(1);
		expect(onHeal).toHaveBeenCalledWith(gamePal);
	});
});

function menuEntries(): (string | undefined)[] {
	return Array.from(document.querySelectorAll('.context-menu button')).map((entry) =>
		entry.textContent?.trim()
	);
}

describe('PalBadge context menu entries', () => {
	it('offers only heal on a live badge', async () => {
		const { container } = render(LivePalBadge, {
			props: { pal: makeGamePal(), onHeal: vi.fn() }
		});

		const button = container.querySelector('button') as HTMLElement;
		await fireEvent.contextMenu(button);
		await new Promise((resolve) => setTimeout(resolve, 0));

		expect(document.querySelectorAll('.context-menu')).toHaveLength(1);
		expect(menuEntries()).toEqual(['Heal']);
	});

	it('still offers every action a save route supplies', async () => {
		render(PalBadge, {
			props: {
				pal: makePal(),
				onMove: noop,
				onAdd: noop,
				onClone: noop,
				onDelete: noop,
				onCloneToUps: noop,
				onCloneToPlayer: noop
			}
		});

		const button = screen.getByRole('button');
		await fireEvent.contextMenu(button);
		await new Promise((resolve) => setTimeout(resolve, 0));

		expect(menuEntries()).toHaveLength(5);
	});

	it('leaves the right-click unhandled when no action was supplied', async () => {
		render(PalBadge, { props: { pal: makePal(), disabled: true } });

		const button = screen.getByRole('button');
		const event = new MouseEvent('contextmenu', { bubbles: true, cancelable: true });
		button.dispatchEvent(event);
		await new Promise((resolve) => setTimeout(resolve, 0));

		expect(document.querySelector('.context-menu')).toBeNull();
		expect(event.defaultPrevented).toBe(false);
	});
});

async function openMenu(container: Element): Promise<void> {
	const button = container.querySelector('button') as HTMLElement;
	await fireEvent.contextMenu(button);
	await new Promise((resolve) => setTimeout(resolve, 0));
}

describe('LivePalBadge pal slot actions', () => {
	it('offers heal, move and remove on a filled slot', async () => {
		const { container } = render(LivePalBadge, {
			props: { pal: makeGamePal(), slotIndex: 4, onHeal: vi.fn(), onMove: vi.fn(), onRemove: vi.fn() }
		});

		await openMenu(container);
		expect(menuEntries()).toEqual(['Heal', 'Move', 'Remove Pal']);
	});

	it('offers nothing on an empty slot until a move is armed', async () => {
		const { container, rerender } = render(LivePalBadge, {
			props: { slotIndex: 4, onHeal: vi.fn(), onMove: vi.fn(), onRemove: vi.fn() }
		});

		const button = container.querySelector('button') as HTMLElement;
		const event = new MouseEvent('contextmenu', { bubbles: true, cancelable: true });
		button.dispatchEvent(event);
		await new Promise((resolve) => setTimeout(resolve, 0));
		expect(document.querySelector('.context-menu')).toBeNull();

		await rerender({ slotIndex: 4, onHeal: vi.fn(), onMove: vi.fn(), onRemove: vi.fn(), movePending: true });
		await openMenu(container);
		expect(menuEntries()).toEqual(['Move here']);
	});

	it('offers the way out on the slot the move was armed from', async () => {
		const { container } = render(LivePalBadge, {
			props: {
				pal: makeGamePal(),
				slotIndex: 4,
				onHeal: vi.fn(),
				onMove: vi.fn(),
				movePending: true,
				isMoveSource: true
			}
		});

		await openMenu(container);
		expect(menuEntries()).toEqual(['Heal', 'Cancel move']);
	});

	it('reports the slot it was given, not the pal own slot', async () => {
		const onMove = vi.fn();
		const { container } = render(LivePalBadge, {
			props: { pal: makeGamePal({ slotIndex: 0 }), slotIndex: 37, onMove }
		});

		await openMenu(container);
		await fireEvent.click(await screen.findByRole('button', { name: 'Move' }));

		expect(onMove).toHaveBeenCalledWith(37);
	});

	it('does not act while a reason says editing is unavailable', async () => {
		const onRemove = vi.fn();
		const onMove = vi.fn();
		const { container } = render(LivePalBadge, {
			props: {
				pal: makeGamePal(),
				slotIndex: 4,
				onRemove,
				onMove,
				editDisabledReason: 'Another change is still running.'
			}
		});

		await openMenu(container);
		for (const entry of document.querySelectorAll('.context-menu button')) {
			await fireEvent.click(entry);
		}

		expect(onRemove).not.toHaveBeenCalled();
		expect(onMove).not.toHaveBeenCalled();
	});
});

describe('LivePalBadge add action', () => {
	it('offers add on an empty slot', async () => {
		const { container } = render(LivePalBadge, {
			props: { slotIndex: 7, onAdd: vi.fn(), onMove: vi.fn(), onRemove: vi.fn() }
		});

		await openMenu(container);
		expect(menuEntries()).toEqual(['Add Pal']);
	});

	it('never offers add on a slot that already holds a pal', async () => {
		const { container } = render(LivePalBadge, {
			props: { pal: makeGamePal(), slotIndex: 7, onAdd: vi.fn(), onRemove: vi.fn() }
		});

		await openMenu(container);
		expect(menuEntries()).not.toContain('Add Pal');
	});

	it('offers the destination instead of add while a move is armed', async () => {
		const { container } = render(LivePalBadge, {
			props: { slotIndex: 7, onAdd: vi.fn(), onMove: vi.fn(), movePending: true }
		});

		await openMenu(container);
		expect(menuEntries()).toEqual(['Move here']);
	});

	it('reports the slot it was given', async () => {
		const onAdd = vi.fn();
		const { container } = render(LivePalBadge, { props: { slotIndex: 41, onAdd } });

		await openMenu(container);
		await fireEvent.click(await screen.findByRole('button', { name: 'Add Pal' }));

		expect(onAdd).toHaveBeenCalledWith(41);
	});
});

describe('LivePalBadge action reasons', () => {
	it('disables only the action its reason belongs to', async () => {
		const onMove = vi.fn();
		const onRemove = vi.fn();
		const { container } = render(LivePalBadge, {
			props: {
				pal: makeGamePal(),
				slotIndex: 7,
				onMove,
				onRemove,
				removeDisabledReason: 'Removal is not wired up'
			}
		});

		await openMenu(container);
		expect(menuEntries()).toContain('Move');
		expect(menuEntries()).toContain('Remove Pal — Removal is not wired up');

		await fireEvent.click(await screen.findByRole('button', { name: 'Move' }));
		expect(onMove).toHaveBeenCalledWith(7);
		expect(onRemove).not.toHaveBeenCalled();
	});

	it('stops the action its reason belongs to', async () => {
		const onRemove = vi.fn();
		const { container } = render(LivePalBadge, {
			props: {
				pal: makeGamePal(),
				slotIndex: 7,
				onRemove,
				removeDisabledReason: 'Removal is not wired up'
			}
		});

		await openMenu(container);
		await fireEvent.click(
			await screen.findByRole('button', { name: /Removal is not wired up/ })
		);
		expect(onRemove).not.toHaveBeenCalled();
	});
});

describe('LivePalBadge edit action', () => {
	it('offers edit on a filled slot and reports the slot it was given', async () => {
		const onEdit = vi.fn();
		const gamePal = makeGamePal({ slotIndex: 0 });
		const { container } = render(LivePalBadge, {
			props: { pal: gamePal, slotIndex: 41, onEdit }
		});

		await openMenu(container);
		expect(menuEntries()).toContain('Edit');
		await fireEvent.click(await screen.findByRole('button', { name: 'Edit' }));
		expect(onEdit).toHaveBeenCalledWith(gamePal, 41);
	});

	it('offers no edit on an empty slot', async () => {
		const { container } = render(LivePalBadge, {
			props: { slotIndex: 41, onEdit: vi.fn(), onAdd: vi.fn() }
		});

		await openMenu(container);
		expect(menuEntries()).not.toContain('Edit');
	});

	it('does not act while a reason says editing is unavailable', async () => {
		const onEdit = vi.fn();
		const { container } = render(LivePalBadge, {
			props: {
				pal: makeGamePal(),
				slotIndex: 41,
				onEdit,
				palEditDisabledReason: 'Editing is not wired up'
			}
		});

		await openMenu(container);
		await fireEvent.click(
			await screen.findByRole('button', { name: /Editing is not wired up/ })
		);
		expect(onEdit).not.toHaveBeenCalled();
	});
});
