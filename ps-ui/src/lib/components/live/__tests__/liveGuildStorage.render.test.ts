// @vitest-environment jsdom
import { getModalState } from '$states';
import { fireEvent, render, screen, within } from '@testing-library/svelte';
import userEvent from '@testing-library/user-event';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import '../../pal/__tests__/fixtures/matchMediaPolyfill';
import './fixtures/animatePolyfill';
import { buildingsData } from '$lib/data/buildings.svelte';
import { normalizeKeys } from '$utils';
import type { GameGuildBaseJson, GameInventoryContainerJson } from '$states/gameState.svelte';
import LiveGuildChest from '../LiveGuildChest.svelte';
import LiveGuildStorage from '../LiveGuildStorage.svelte';
import { clearItems, seedItems } from './fixtures/itemsFixture';

afterEach(() => {
	clearItems();
	vi.restoreAllMocks();
});

beforeEach(() => {
	buildingsData.buildings = {
		ItemChest_04: { localized_name: 'Advanced Chest', icon: 'T_itemIcon_ItemChest_04' },
		Refrigerator: { localized_name: 'Refrigerator', icon: 'T_itemIcon_Refrigerator' },
		GuildChest: { localized_name: 'Guild Chest', icon: 'T_itemIcon_GuildChest' }
	} as never;
	(buildingsData as unknown as { keyMap: Record<string, string> }).keyMap = normalizeKeys(
		Object.keys(buildingsData.buildings)
	);
});

const bases: GameGuildBaseJson[] = [
	{ id: 'b1', name: 'Ironworks Prime', level: 20, containerId: 'bc1', palSlotNum: 8 },
	{ id: 'b2', name: 'Ore Shelf', level: 14, containerId: 'bc2', palSlotNum: 8 }
];

function container(
	id: string,
	buildObjectId: string,
	slotNum: number,
	overrides: Partial<GameInventoryContainerJson> = {}
): GameInventoryContainerJson {
	return {
		containerId: id,
		type: 'Common',
		slotNum,
		status: 'ok',
		slots: [{ slotIndex: 0, staticItemId: 'Wood', count: 42, dynamicItem: null }],
		buildObjectId,
		baseId: 'b1',
		...overrides
	};
}

describe('LiveGuildStorage', () => {
	it('lists every container the selected base owns', async () => {
		render(LiveGuildStorage, {
			containers: [container('c1', 'ItemChest_04', 15), container('c2', 'Refrigerator', 10)],
			bases
		});

		expect(await screen.findByText('Advanced Chest')).toBeTruthy();
		expect(screen.getByText('Refrigerator')).toBeTruthy();
	});

	it('opens the first container and draws a cell for every slot it holds', async () => {
		render(LiveGuildStorage, {
			containers: [container('c1', 'ItemChest_04', 15)],
			bases
		});

		const grid = await screen.findByTestId('live-guild-container-slots');
		expect(within(grid).getAllByRole('button')).toHaveLength(15);
	});

	it('shows the container the user picked', async () => {
		render(LiveGuildStorage, {
			containers: [container('c1', 'ItemChest_04', 15), container('c2', 'Refrigerator', 4)],
			bases
		});

		await userEvent.click(await screen.findByText('Refrigerator'));

		const grid = await screen.findByTestId('live-guild-container-slots');
		expect(within(grid).getAllByRole('button')).toHaveLength(4);
	});

	it('says there is no storage only when the reply really is empty', async () => {
		render(LiveGuildStorage, { containers: [], bases });

		expect(await screen.findByText(/no storage/i)).toBeTruthy();
	});

	it('falls back to the first container when the selected one drops out of a later reply', async () => {
		const { rerender } = render(LiveGuildStorage, {
			containers: [container('c1', 'ItemChest_04', 15), container('c2', 'Refrigerator', 4)],
			bases
		});

		await userEvent.click(await screen.findByText('Refrigerator'));
		let grid = await screen.findByTestId('live-guild-container-slots');
		expect(within(grid).getAllByRole('button')).toHaveLength(4);

		await rerender({ containers: [container('c1', 'ItemChest_04', 15)], bases });

		grid = await screen.findByTestId('live-guild-container-slots');
		expect(within(grid).getAllByRole('button')).toHaveLength(15);
	});

	it('shows only the containers standing on the selected base', async () => {
		render(LiveGuildStorage, {
			containers: [
				container('c1', 'ItemChest_04', 15, { baseId: 'b1' }),
				container('c2', 'Refrigerator', 10, { baseId: 'b2' })
			],
			bases
		});

		expect(await screen.findByText('Advanced Chest')).toBeTruthy();
		expect(screen.queryByText('Refrigerator')).toBeNull();
	});

	it('moves to the other base through the pager', async () => {
		const containers = [
			container('c1', 'ItemChest_04', 15, { baseId: 'b1' }),
			container('c2', 'Refrigerator', 10, { baseId: 'b2' })
		];
		const onBasePageChange = vi.fn();
		const { rerender } = render(LiveGuildStorage, {
			containers,
			bases,
			basePage: 0,
			onBasePageChange
		});

		expect(await screen.findByText('Advanced Chest')).toBeTruthy();

		await userEvent.click(screen.getByRole('button', { name: '2' }));
		expect(onBasePageChange).toHaveBeenCalledWith(1);

		await rerender({ containers, bases, basePage: 1, onBasePageChange });

		expect(await screen.findByText('Refrigerator')).toBeTruthy();
		expect(screen.queryByText('Advanced Chest')).toBeNull();
	});

	it('names a row by the building it holds rather than its guid', async () => {
		render(LiveGuildStorage, {
			containers: [container('c1', 'ItemChest_04', 15)],
			bases
		});

		expect(await screen.findByText('Advanced Chest')).toBeTruthy();
		expect(screen.queryByText(/c1/)).toBeNull();
	});

	it('excludes empty-slot-count containers and bookkeeping keys like None', async () => {
		render(LiveGuildStorage, {
			containers: [
				container('c1', 'ItemChest_04', 15),
				container('c2', 'None', 0),
				container('c3', 'None', 5)
			],
			bases
		});

		expect(await screen.findByText('Advanced Chest')).toBeTruthy();
		expect(screen.queryByText('c2')).toBeNull();
		expect(screen.queryByText('c3')).toBeNull();
		const grid = await screen.findByTestId('live-guild-container-slots');
		expect(within(grid).getAllByRole('button')).toHaveLength(15);
	});
});

describe('LiveGuildStorage slot editing', () => {
	it('commits a slot change to the selected container', async () => {
		seedItems(['Wood'], 99);
		vi.spyOn(getModalState(), 'showModal').mockResolvedValue(['Wood', 7, undefined]);
		const onSetSlot = vi.fn();

		render(LiveGuildStorage, {
			containers: [container('c1', 'ItemChest_04', 15)],
			bases,
			onSetSlot
		});

		const grid = await screen.findByTestId('live-guild-container-slots');
		await fireEvent.click(within(grid).getAllByRole('button')[0]);

		expect(onSetSlot).toHaveBeenCalledWith('c1', 0, 'Wood', 7);
	});

	it('sends a null item when the picker clears the slot', async () => {
		seedItems(['Wood']);
		vi.spyOn(getModalState(), 'showModal').mockResolvedValue(['None', 0, undefined]);
		const onSetSlot = vi.fn();

		render(LiveGuildStorage, {
			containers: [container('c1', 'ItemChest_04', 15)],
			bases,
			onSetSlot
		});

		const grid = await screen.findByTestId('live-guild-container-slots');
		await fireEvent.click(within(grid).getAllByRole('button')[0]);

		expect(onSetSlot).toHaveBeenCalledWith('c1', 0, null, 0);
	});

	it('opens no picker and fires no callback while a reason says editing is unavailable', async () => {
		seedItems(['Wood']);
		const showModal = vi.spyOn(getModalState(), 'showModal');
		const onSetSlot = vi.fn();

		render(LiveGuildStorage, {
			containers: [container('c1', 'ItemChest_04', 15)],
			bases,
			onSetSlot,
			editDisabledReason: 'Another change is still running.'
		});

		const grid = await screen.findByTestId('live-guild-container-slots');
		await fireEvent.click(within(grid).getAllByRole('button')[0]);

		expect(showModal).not.toHaveBeenCalled();
		expect(onSetSlot).not.toHaveBeenCalled();
	});
});

describe('LiveGuildChest', () => {
	function chest(id: string, slotNum: number): GameInventoryContainerJson {
		return {
			containerId: id,
			type: 'GuildChest',
			slotNum,
			status: 'ok',
			slots: [{ slotIndex: 0, staticItemId: 'Wood', count: 42, dynamicItem: null }],
			buildObjectId: 'GuildChest',
			baseId: null
		};
	}

	it('draws the chest slots', async () => {
		render(LiveGuildChest, { containers: [chest('gc', 6)] });

		const grid = await screen.findByTestId('live-guild-chest-slots');
		expect(within(grid).getAllByRole('button')).toHaveLength(6);
	});

	it('names the chest by its building, not a guid', async () => {
		render(LiveGuildChest, { containers: [chest('gc', 6)] });

		expect(await screen.findByText(/Guild Chest/)).toBeTruthy();
		expect(screen.queryByText(/gc/)).toBeNull();
	});

	it('says so when the guild has no chest', async () => {
		render(LiveGuildChest, { containers: [] });

		expect(await screen.findByText(/no guild chest/i)).toBeTruthy();
	});

	it('keeps every guild-typed container reachable, not just the first', async () => {
		render(LiveGuildChest, {
			containers: [chest('gc1', 6), chest('gc2', 3)]
		});

		const rows = await screen.findAllByText(/Guild Chest/);
		expect(rows).toHaveLength(2);

		await userEvent.click(rows[1]);

		const grid = await screen.findByTestId('live-guild-chest-slots');
		expect(within(grid).getAllByRole('button')).toHaveLength(3);
	});

	it('commits a slot change to the selected chest', async () => {
		seedItems(['Wood'], 99);
		vi.spyOn(getModalState(), 'showModal').mockResolvedValue(['Wood', 7, undefined]);
		const onSetSlot = vi.fn();

		render(LiveGuildChest, { containers: [chest('gc', 6)], onSetSlot });

		const grid = await screen.findByTestId('live-guild-chest-slots');
		await fireEvent.click(within(grid).getAllByRole('button')[0]);

		expect(onSetSlot).toHaveBeenCalledWith('gc', 0, 'Wood', 7);
	});

	it('sends a null item when the picker clears a chest slot', async () => {
		seedItems(['Wood']);
		vi.spyOn(getModalState(), 'showModal').mockResolvedValue(['None', 0, undefined]);
		const onSetSlot = vi.fn();

		render(LiveGuildChest, { containers: [chest('gc', 6)], onSetSlot });

		const grid = await screen.findByTestId('live-guild-chest-slots');
		await fireEvent.click(within(grid).getAllByRole('button')[0]);

		expect(onSetSlot).toHaveBeenCalledWith('gc', 0, null, 0);
	});

	it('opens no picker and fires no callback while a reason says editing is unavailable', async () => {
		seedItems(['Wood']);
		const showModal = vi.spyOn(getModalState(), 'showModal');
		const onSetSlot = vi.fn();

		render(LiveGuildChest, {
			containers: [chest('gc', 6)],
			onSetSlot,
			editDisabledReason: 'Another change is still running.'
		});

		const grid = await screen.findByTestId('live-guild-chest-slots');
		await fireEvent.click(within(grid).getAllByRole('button')[0]);

		expect(showModal).not.toHaveBeenCalled();
		expect(onSetSlot).not.toHaveBeenCalled();
	});
});
