// @vitest-environment jsdom
import type { GameCapabilitiesJson } from '$states/gameState.svelte';
import { MessageType } from '$types';
import { fireEvent, render, screen, within } from '@testing-library/svelte';
import userEvent from '@testing-library/user-event';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import '../../pal/__tests__/fixtures/matchMediaPolyfill';
import './fixtures/animatePolyfill';
import { clearFriendship, seedFriendship } from './fixtures/friendshipFixture';

const capabilities: GameCapabilitiesJson = {
	version: 1,
	ops: {
		'pal.heal': { available: true, reason: null },
		'item.setSlot': { available: true, reason: null },
		'pal.remove': { available: true, reason: null },
		'pal.move': { available: true, reason: null },
		'player.edit': { available: true, reason: null },
		'guild.edit': { available: true, reason: null },
		'guild.setRole': { available: true, reason: null },
		'pal.add': { available: false, reason: null },
		'pal.edit': { available: false, reason: null }
	}
};

let releaseSetLevel: (() => void) | null = null;
let holdSetLevel = false;
let releaseBasePals: (() => void) | null = null;
let holdBasePals = false;

const mockSendAndWait = vi.fn(async (type: string, _data?: unknown) => {
	switch (type) {
		case MessageType.GAME_STATUS:
			return {
				authoritative: true,
				modVersion: '1.0.0',
				mode: 'coop_host',
				protocolVersion: 1,
				queueDepth: 0,
				worldLoaded: true
			};
		case MessageType.GAME_CAPABILITIES:
			return capabilities;
		case MessageType.GAME_PLAYERS:
			return {
				players: [
					{ uid: 'p1', nickname: 'Aurora', level: 5, exp: 100, status: 'ok' },
					{ uid: 'p2', nickname: 'Fenrir', level: 9, exp: 400, status: 'ok' }
				],
				status: 'ok'
			};
		case MessageType.GAME_PALS:
			return {
				pals: [
					{
						characterId: 'Foxparks',
						instanceId: 'pal-1',
						ownerUid: 'p1',
						slotIndex: 0,
						nickname: 'Sparky',
						level: 12
					}
				],
				page: 0,
				pageCount: 1,
				slotCount: 30,
				slotBase: 0,
				party: [
					{
						characterId: 'Pengullet',
						instanceId: 'otomo-1',
						ownerUid: 'p1',
						slotIndex: 0,
						nickname: 'Waddles',
						level: 9
					}
				],
				partyStatus: 'ok',
				status: 'ok'
			};
		case MessageType.GAME_GUILDS:
			return {
				guilds: [
					{ id: 'g1', name: 'DeBugging', adminUid: 'u1', members: [] },
					{ id: 'g2', name: 'Gobblin', adminUid: null, members: [] }
				],
				status: 'ok'
			};
		case MessageType.GAME_GUILD:
			return {
				guild: {
					id: 'g1',
					name: 'DeBugging',
					groupName: '00000000000000000000000000000001',
					baseCampLevel: 35,
					baseCampIds: ['b1', 'b2'],
					baseCampPointIds: ['p1', 'p2'],
					memberUids: ['00000000-0000-0000-0000-000000000001'],
					adminUid: 'u1',
					roleOptions: ['GuildMaster', 'SubMaster', 'Member', 'Guest'],
					members: [
						{ uid: 'u1', name: 'Aurora', role: 'GuildMaster', status: 'Online', lastOnlineTicks: 1 },
						{ uid: 'u2', name: 'Fenrir', role: 'Member', status: 'Offline', lastOnlineTicks: 2 }
					],
					lab: { currentResearchId: 'Research_A', research: [] },
					bases: [
						{ id: 'b1', name: 'Main', level: 35, buildingNum: 259, containerId: 'c1', palSlotNum: 15 },
						{ id: 'b2', name: 'Outpost', level: 35, buildingNum: 12, containerId: 'c2', palSlotNum: 15 }
					]
				},
				status: 'ok'
			};
		case MessageType.GAME_GUILD_CONTAINERS:
			return {
				guildId: 'g1',
				containers: [
					{ containerId: 'gc1', type: 'GuildChest', status: 'ok', slotNum: 1, slots: [] }
				],
				status: 'ok'
			};
		case MessageType.GAME_SET_GUILD_ROLE:
			return {
				commandId: 'cmd-role',
				op: 'guild.setRole',
				applied: true,
				verified: true,
				retrySafe: true,
				data: {}
			};
		case MessageType.GAME_EDIT_GUILD:
			return {
				commandId: 'cmd-guild',
				op: 'guild.edit',
				applied: true,
				verified: true,
				retrySafe: true,
				data: {}
			};
		case MessageType.GAME_BASE_PALS:
			if (holdBasePals) await new Promise<void>((resolve) => (releaseBasePals = resolve));
			return {
				baseId: 'b1',
				containerId: 'c1',
				slotNum: 15,
				pals: [
					{
						instanceId: 'bp1',
						playerUid: '00000000-0000-0000-0000-000000000009',
						characterId: 'Anubis',
						slotIndex: 0,
						level: 60,
						nickname: 'Anubis'
					}
				],
				status: 'ok'
			};
		case MessageType.GAME_PAL_DETAIL:
			return { error: 'no detail', code: 'validation_failed' };
		case MessageType.GAME_INVENTORY:
			return {
				playerUid: 'p1',
				status: 'ok',
				containers: [
					{
						containerId: 'container-1',
						type: 'common',
						slotNum: 2,
						status: 'ok',
						slots: [{ slotIndex: 0, staticItemId: 'Wood', count: 5 }]
					}
				]
			};
		case MessageType.GET_EXP_DATA:
			return {
				'5': { TotalEXP: 740 },
				'20': { TotalEXP: 56840 },
				'50': { TotalEXP: 2378134 }
			};
		case MessageType.GAME_EDIT_PLAYER:
			if (holdSetLevel) await new Promise<void>((resolve) => (releaseSetLevel = resolve));
			return {
				commandId: 'c2',
				op: 'player.edit',
				applied: true,
				verified: true,
				retrySafe: false,
				data: { player: { level: 20, exp: 56840 } }
			};
		default:
			return {};
	}
});

vi.mock('$utils/websocketUtils', () => ({
	sendAndWait: (...args: [string, unknown?]) => mockSendAndWait(...args),
	send: vi.fn(),
	sendBytes: vi.fn(),
	isReady: () => true,
	pushProgressMessage: vi.fn()
}));

import { getGameState, getModalState } from '$states';
import LivePage from '../../../../routes/live/+page.svelte';

const gameState = getGameState();

function resetGameState() {
	gameState.status = null;
	gameState.statusError = null;
	gameState.capabilities = null;
	gameState.players = [];
	gameState.playersError = null;
	gameState.pals = [];
	gameState.palsPage = 0;
	gameState.palsPageCount = 0;
	gameState.palsError = null;
	gameState.inventory = null;
	gameState.inventoryError = null;
	gameState.writeBusy = false;
}

beforeEach(() => {
	mockSendAndWait.mockClear();
	capabilities.ops['pal.heal'] = { available: true, reason: null };
	capabilities.ops['pal.add'] = { available: false, reason: null };
	capabilities.ops['pal.edit'] = { available: false, reason: null };
	holdSetLevel = false;
	releaseSetLevel = null;
	holdBasePals = false;
	releaseBasePals = null;
	resetGameState();
	seedFriendship();
});

afterEach(() => {
	clearFriendship();
});

describe('live page selection', () => {
	it('opens the detail pane for the player that was picked', async () => {
		const user = userEvent.setup();
		render(LivePage);

		const row = await screen.findByRole('button', { name: /Aurora/ });
		expect(screen.queryByRole('button', { name: 'Heal All' })).toBeNull();

		await user.click(row);

		expect(await screen.findByText('Sparky')).toBeTruthy();
		expect(screen.getByRole('spinbutton')).toBeTruthy();

		await user.click(screen.getByRole('tab', { name: 'Palbox' }));
		expect(screen.getByRole('button', { name: 'Heal All' })).toBeTruthy();
	});
});

describe('live page capability gating', () => {
	it('disables an action and names the reason the game gave', async () => {
		capabilities.ops['pal.heal'] = { available: false, reason: 'Heal is not wired up here' };
		const user = userEvent.setup();
		render(LivePage);

		await user.click(await screen.findByRole('button', { name: /Aurora/ }));
		await user.click(await screen.findByRole('tab', { name: 'Palbox' }));

		const healAll = await screen.findByRole('button', { name: 'Heal All' });
		expect((healAll as HTMLButtonElement).disabled).toBe(true);

		await user.hover(document.querySelector('#live-pal-actions')!.firstElementChild!);
		expect(await screen.findByText('Heal is not wired up here')).toBeTruthy();
	});
});

describe('live page player level', () => {
	it('offers a level control rather than a raw exp box', async () => {
		const user = userEvent.setup();
		render(LivePage);

		await user.click(await screen.findByRole('button', { name: /Aurora/ }));

		expect(await screen.findByLabelText('Level')).toBeTruthy();
		expect(screen.queryByLabelText('EXP amount')).toBeNull();
	});

	it('sends the level and the exp that level requires', async () => {
		const user = userEvent.setup();
		render(LivePage);

		await user.click(await screen.findByRole('button', { name: /Aurora/ }));
		const input = await screen.findByLabelText('Level');
		await user.clear(input);
		await user.type(input, '20');
		await user.click(screen.getByRole('button', { name: 'Set level' }));

		const call = await vi.waitFor(() => {
			const found = mockSendAndWait.mock.calls.find(
				([type]) => type === MessageType.GAME_EDIT_PLAYER
			);
			expect(found).toBeTruthy();
			return found as [string, Record<string, unknown>];
		});
		expect(call[1]).toMatchObject({ player_uid: 'p1', level: 20, exp: 56840 });
	});
});

describe('live page party', () => {
	it('shows the selected player party in the rail', async () => {
		const user = userEvent.setup();
		render(LivePage);

		await user.click(await screen.findByRole('button', { name: /Aurora/ }));

		const rail = await vi.waitFor(() => {
			const found = document.querySelector('#live-party-rail');
			expect(found).toBeTruthy();
			return found as HTMLElement;
		});
		expect(within(rail).getAllByAltText('Pengullet')).not.toHaveLength(0);
	});
});

describe('live page status', () => {
	it('reads out full server status until a player is picked', async () => {
		render(LivePage);

		expect(await screen.findByText('Server Status')).toBeTruthy();
		expect(document.querySelector('#live-status-chips')).toBeNull();
	});

	it('trades the status card for header chips once a player is picked', async () => {
		const user = userEvent.setup();
		render(LivePage);

		await user.click(await screen.findByRole('button', { name: /Aurora/ }));

		const chips = await vi.waitFor(() => {
			const found = document.querySelector('#live-status-chips');
			expect(found).toBeTruthy();
			return found as HTMLElement;
		});
		expect(within(chips).getByText('v1.0.0')).toBeTruthy();
		expect(within(chips).getByText('Solo/Host')).toBeTruthy();
		expect(within(chips).getByText('Host')).toBeTruthy();
		expect(within(chips).getByText('World loaded')).toBeTruthy();
		expect(screen.queryByText('Server Status')).toBeNull();
	});

	it('leaves queue depth out of the chips', async () => {
		const user = userEvent.setup();
		render(LivePage);

		await user.click(await screen.findByRole('button', { name: /Aurora/ }));

		const chips = await vi.waitFor(() => {
			const found = document.querySelector('#live-status-chips');
			expect(found).toBeTruthy();
			return found as HTMLElement;
		});
		expect(within(chips).queryByText('Queue')).toBeNull();
	});
});

describe('live page party rail width', () => {
	it('widens the column when the party is expanded', async () => {
		const user = userEvent.setup();
		render(LivePage);

		await user.click(await screen.findByRole('button', { name: /Aurora/ }));

		const column = await vi.waitFor(() => {
			const found = document.querySelector('[data-sidebar]');
			expect(found).toBeTruthy();
			return found as HTMLElement;
		});
		expect(column.className).toContain('w-20');

		await user.click(await screen.findByRole('button', { name: 'Expand party' }));

		expect(column.className).not.toContain('w-20');
	});

	it('returns to the rail width when the party is collapsed again', async () => {
		const user = userEvent.setup();
		render(LivePage);

		await user.click(await screen.findByRole('button', { name: /Aurora/ }));
		await user.click(await screen.findByRole('button', { name: 'Expand party' }));
		await user.click(await screen.findByRole('button', { name: 'Collapse party' }));

		const column = document.querySelector('[data-sidebar]') as HTMLElement;
		expect(column.className).toContain('w-20');
	});
});

describe('live page player switcher', () => {
	it('switches to another player from the header', async () => {
		const user = userEvent.setup();
		render(LivePage);

		await user.click(await screen.findByRole('button', { name: /Aurora/ }));
		await vi.waitFor(() => expect(document.querySelector('#live-status-chips')).toBeTruthy());

		await user.click(screen.getByRole('button', { name: 'Switch player' }));
		await user.click(await screen.findByRole('button', { name: /Fenrir/ }));

		const switcher = await vi.waitFor(() => {
			const found = document.querySelector('#live-player-switcher');
			expect(found).toBeTruthy();
			return found as HTMLElement;
		});
		expect(within(switcher).getByText('Fenrir')).toBeTruthy();
	});

	it('narrows the list to what was typed', async () => {
		const user = userEvent.setup();
		render(LivePage);

		await user.click(await screen.findByRole('button', { name: /Aurora/ }));
		await vi.waitFor(() => expect(document.querySelector('#live-status-chips')).toBeTruthy());

		await user.click(screen.getByRole('button', { name: 'Switch player' }));
		await user.type(screen.getByRole('searchbox'), 'Fen');

		expect(await screen.findByRole('button', { name: /Fenrir/ })).toBeTruthy();
		expect(screen.queryByRole('button', { name: /Aurora/ })).toBeNull();
	});
});

describe('live page pal move', () => {
	async function openPalbox(user: ReturnType<typeof userEvent.setup>) {
		await user.click(await screen.findByRole('button', { name: /Aurora/ }));
		await user.click(await screen.findByRole('tab', { name: 'Palbox' }));
		return await vi.waitFor(() => {
			const slots = document.querySelectorAll('#live-palbox-grid button');
			expect(slots.length).toBeGreaterThan(1);
			return slots;
		});
	}

	async function pick(slot: Element, name: string | RegExp) {
		await fireEvent.contextMenu(slot);
		await new Promise((resolve) => setTimeout(resolve, 0));
		await fireEvent.click(await screen.findByRole('button', { name }));
	}

	it('sends nothing until a destination is picked, then sends both ends', async () => {
		const user = userEvent.setup();
		render(LivePage);
		const slots = await openPalbox(user);

		await pick(slots[0], 'Move');
		expect(
			mockSendAndWait.mock.calls.filter(([type]) => type === MessageType.GAME_MOVE_PAL)
		).toHaveLength(0);

		await pick(slots[3], 'Move here');
		const sent = mockSendAndWait.mock.calls.filter(([type]) => type === MessageType.GAME_MOVE_PAL);
		expect(sent).toHaveLength(1);
		expect(sent[0][1]).toMatchObject({ from_slot_index: 0, to_slot_index: 3 });
	});

	it('cancels when the armed slot is picked again', async () => {
		const user = userEvent.setup();
		render(LivePage);
		const slots = await openPalbox(user);

		await pick(slots[0], 'Move');
		await pick(slots[0], 'Cancel move');

		expect(
			mockSendAndWait.mock.calls.filter(([type]) => type === MessageType.GAME_MOVE_PAL)
		).toHaveLength(0);
	});
});

describe('live page write gate', () => {
	it('refuses a second write while one is still in flight', async () => {
		holdSetLevel = true;
		const user = userEvent.setup();
		render(LivePage);

		await user.click(await screen.findByRole('button', { name: /Aurora/ }));

		const amount = await screen.findByRole('spinbutton');
		await fireEvent.input(amount, { target: { value: '50' } });
		await fireEvent.change(amount);

		const setLevel = screen.getByRole('button', { name: 'Set level' });
		await user.click(setLevel);

		const slot = document.querySelector('#live-inventory-panel button');
		await user.click(slot!);
		expect(
			mockSendAndWait.mock.calls.filter(([type]) => type === MessageType.GAME_SET_ITEM_SLOT)
		).toHaveLength(0);

		await user.click(screen.getByRole('tab', { name: 'Palbox' }));
		await user.click(screen.getByRole('button', { name: 'Heal All' }));
		expect(
			mockSendAndWait.mock.calls.filter(([type]) => type === MessageType.GAME_HEAL_PALS)
		).toHaveLength(0);

		releaseSetLevel?.();
		await vi.waitFor(() => expect((setLevel as HTMLButtonElement).disabled).toBe(false));
	});
});

describe('live page detail polling', () => {
	function callsOf(type: string): number {
		return mockSendAndWait.mock.calls.filter(([messageType]) => messageType === type).length;
	}

	async function selectAurora() {
		vi.useFakeTimers();
		const user = userEvent.setup({ advanceTimers: vi.advanceTimersByTime.bind(vi) });
		render(LivePage);
		await user.click(await screen.findByRole('button', { name: /Aurora/ }));
		return user;
	}

	async function poll() {
		await vi.advanceTimersByTimeAsync(5000);
	}

	afterEach(() => {
		vi.useRealTimers();
	});

	it('refreshes the loadout on its own without a reselect', async () => {
		await selectAurora();
		const before = callsOf(MessageType.GAME_INVENTORY);

		await poll();

		expect(callsOf(MessageType.GAME_INVENTORY)).toBeGreaterThan(before);
	});

	it('polls only the tab on screen', async () => {
		const user = await selectAurora();
		await user.click(await screen.findByRole('tab', { name: 'Palbox' }));
		const inventoryBefore = callsOf(MessageType.GAME_INVENTORY);
		const palsBefore = callsOf(MessageType.GAME_PALS);

		await poll();

		expect(callsOf(MessageType.GAME_PALS)).toBeGreaterThan(palsBefore);
		expect(callsOf(MessageType.GAME_INVENTORY)).toBe(inventoryBefore);
	});

	it('never blanks the pane it is refreshing', async () => {
		await selectAurora();

		await poll();

		expect(screen.queryByText(/Loading/)).toBeNull();
		expect(screen.getByText('Sparky')).toBeTruthy();
	});

	it('leaves a write in flight alone', async () => {
		holdSetLevel = true;
		const user = await selectAurora();

		const amount = await screen.findByRole('spinbutton');
		await fireEvent.input(amount, { target: { value: '50' } });
		await fireEvent.change(amount);
		await user.click(screen.getByRole('button', { name: 'Set level' }));
		const before = callsOf(MessageType.GAME_INVENTORY);

		await poll();

		expect(callsOf(MessageType.GAME_INVENTORY)).toBe(before);
		releaseSetLevel?.();
	});

	it('does not blank the base pals when the poll refreshes them', async () => {
		const user = await selectAurora();
		await user.click(await screen.findByRole('tab', { name: 'Guild' }));
		await user.click(await screen.findByRole('button', { name: /base pals/i }));
		await screen.findByTestId('live-base-pals');

		holdBasePals = true;
		await poll();

		expect(screen.getByTestId('live-base-pals')).toBeTruthy();

		releaseBasePals?.();
	});
});

describe('live page guild tab', () => {
	async function selectAuroraReal() {
		const user = userEvent.setup();
		render(LivePage);
		await user.click(await screen.findByRole('button', { name: /Aurora/ }));
		return user;
	}

	it('shows the guild of the selected player when the tab is opened', async () => {
		const user = await selectAuroraReal();

		await user.click(await screen.findByRole('tab', { name: 'Guild' }));

		expect(await screen.findByText('DeBugging')).toBeTruthy();
		const roster = document.querySelector('#live-guild-members') as HTMLElement;
		expect(within(roster).getByText('Fenrir')).toBeTruthy();
	});

	it('asks for the guild of the player that is selected', async () => {
		const user = await selectAuroraReal();

		await user.click(await screen.findByRole('tab', { name: 'Guild' }));

		const call = mockSendAndWait.mock.calls.find(([type]) => type === MessageType.GAME_GUILD);
		expect((call?.[1] as { player_uid: string }).player_uid).toBe('p1');
	});
});

describe('live page base pals', () => {
	async function openGuildTab() {
		const user = userEvent.setup();
		render(LivePage);
		await user.click(await screen.findByRole('button', { name: /Aurora/ }));
		await user.click(await screen.findByRole('tab', { name: 'Guild' }));
		await user.click(await screen.findByRole('button', { name: /base pals/i }));
		return user;
	}

	it('loads the first base of the guild it just showed', async () => {
		await openGuildTab();

		expect(await screen.findByText('Main')).toBeTruthy();
		const call = mockSendAndWait.mock.calls.find(([type]) => type === MessageType.GAME_BASE_PALS);
		expect((call?.[1] as { base_id: string }).base_id).toBe('b1');
	});

	it('asks for the base the pager moved to', async () => {
		const user = await openGuildTab();
		await screen.findByText('Main');

		const pager = document.querySelector('#live-base-pager') as HTMLElement;
		await user.click(within(pager).getByRole('button', { name: '2' }));

		const calls = mockSendAndWait.mock.calls.filter(([type]) => type === MessageType.GAME_BASE_PALS);
		expect((calls.at(-1)?.[1] as { base_id: string }).base_id).toBe('b2');
	});
});

describe('live page guild containers and level', () => {
	async function openGuildTab() {
		const user = userEvent.setup();
		render(LivePage);
		await user.click(await screen.findByRole('button', { name: /Aurora/ }));
		await user.click(await screen.findByRole('tab', { name: 'Guild' }));
		return user;
	}

	it('asks for containers by guild, not by player', async () => {
		const user = await openGuildTab();
		await user.click(await screen.findByRole('button', { name: /guild chest/i }));

		await screen.findByTestId('live-guild-chest-slots');
		const call = mockSendAndWait.mock.calls.find(
			([type]) => type === MessageType.GAME_GUILD_CONTAINERS
		);
		expect((call?.[1] as { guild_id: string }).guild_id).toBe('g1');
	});

	it('sends the base camp level for the guild on screen', async () => {
		const user = await openGuildTab();

		const input = await screen.findByLabelText('Base camp level');
		await user.clear(input);
		await user.type(input, '12');
		await user.click(screen.getByRole('button', { name: 'Set level' }));

		const call = mockSendAndWait.mock.calls.find(([type]) => type === MessageType.GAME_EDIT_GUILD);
		expect(call?.[1]).toMatchObject({ guild_id: 'g1', base_camp_level: 12 });
	});
});

describe('live page guild roles', () => {
	it('sends the picked role for that member of the guild on screen', async () => {
		const user = userEvent.setup();
		render(LivePage);
		await user.click(await screen.findByRole('button', { name: /Aurora/ }));
		await user.click(await screen.findByRole('tab', { name: 'Guild' }));

		const select = await screen.findByLabelText('Role for Fenrir');
		await user.selectOptions(select, 'SubMaster');

		const call = mockSendAndWait.mock.calls.find(
			([type]) => type === MessageType.GAME_SET_GUILD_ROLE
		);
		expect(call?.[1]).toMatchObject({ guild_id: 'g1', member_uid: 'u2', role: 'SubMaster' });
	});
});

describe('live page guild selector', () => {
	it('asks for the picked guild by id, not through the player', async () => {
		const user = userEvent.setup();
		render(LivePage);
		await user.click(await screen.findByRole('button', { name: /Aurora/ }));
		await user.click(await screen.findByRole('tab', { name: 'Guild' }));

		await screen.findByText('DeBugging');
		const picker = document.querySelector('#live-guild-picker') as HTMLSelectElement;
		await user.selectOptions(picker, 'g2');

		const calls = mockSendAndWait.mock.calls.filter(([type]) => type === MessageType.GAME_GUILD);
		expect(calls.at(-1)?.[1]).toEqual({ guild_id: 'g2' });
	});
});

describe('live page pal placement', () => {
	const modal = getModalState();

	async function openBasePals() {
		capabilities.ops['pal.add'] = { available: true, reason: null };
		capabilities.ops['pal.edit'] = { available: true, reason: null };
		const user = userEvent.setup();
		render(LivePage);
		await user.click(await screen.findByRole('button', { name: /Aurora/ }));
		await user.click(await screen.findByRole('tab', { name: 'Guild' }));
		await user.click(await screen.findByRole('button', { name: /base pals/i }));
		return user;
	}

	function addCalls() {
		return mockSendAndWait.mock.calls.filter(([type]) => type === MessageType.GAME_ADD_PAL);
	}

	it('puts a new pal in the base slot that was clicked', async () => {
		const user = await openBasePals();
		const grid = await screen.findByTestId('live-base-pals');

		await user.click(within(grid).getByRole('button', { name: /Add Pal .* slot 4$/ }));
		modal.closeModal(['Lamball', '', 'Female']);
		await vi.waitFor(() => expect(addCalls()).toHaveLength(1));

		expect(addCalls()[0][1]).toMatchObject({
			player_uid: 'p1',
			slot_index: 3,
			character_id: 'Lamball',
			base_id: 'b1',
			gender: 'female'
		});
	});

	it('puts a new pal in the party without naming a seat', async () => {
		const user = await openBasePals();
		const rail = document.querySelector('#live-party-rail') as HTMLElement;

		await user.click(within(rail).getByRole('button', { name: /Add Pal .* slot 2$/ }));
		modal.closeModal(['Lamball', '', 'Female']);
		await vi.waitFor(() => expect(addCalls()).toHaveLength(1));

		expect(addCalls()[0][1]).toMatchObject({
			player_uid: 'p1',
			slot_index: null,
			party: true,
			base_id: null
		});
	});

	it('reads a base pal by its own id and player uid', async () => {
		const user = await openBasePals();
		const grid = await screen.findByTestId('live-base-pals');

		await user.click(within(grid).getByRole('button', { name: /Edit .* slot 1$/ }));

		const calls = mockSendAndWait.mock.calls.filter(
			([type]) => type === MessageType.GAME_PAL_DETAIL
		);
		expect(calls.at(-1)?.[1]).toEqual({
			player_uid: '00000000-0000-0000-0000-000000000009',
			slot_index: null,
			instance_id: 'bp1'
		});
	});
});
