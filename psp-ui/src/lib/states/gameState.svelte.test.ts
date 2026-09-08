import { beforeEach, describe, expect, it, vi } from 'vitest';

const sendAndWait = vi.fn();

vi.mock('$lib/utils/websocketUtils', () => ({
	sendAndWait: (type: unknown, data?: unknown) => sendAndWait(type, data),
	send: vi.fn()
}));

import type {
	GameCapabilitiesJson,
	GameCommandResultJson,
	GameHealPalsJson,
	GameBasePalsJson,
	GameGuildJson,
	GameInventoryJson,
	GamePalJson,
	GamePalsJson,
	GamePlayersJson,
	GameStatusJson
} from './gameState.svelte';
import { GameState } from './gameState.svelte';

function statusJson(overrides: Partial<GameStatusJson> = {}): GameStatusJson {
	return {
		authoritative: true,
		modVersion: '0.1.0',
		mode: 'coop_host',
		protocolVersion: 1,
		queueDepth: 0,
		worldLoaded: true,
		...overrides
	};
}

function capsJson(): GameCapabilitiesJson {
	return { version: 1, ops: {} };
}

function playersJson(): GamePlayersJson {
	return { players: [{ uid: 'p1', status: 'ok' }], status: 'ok' };
}

function inventoryJson(): GameInventoryJson {
	return {
		playerUid: 'p1',
		status: 'ok',
		containers: [
			{
				containerId: 'c1',
				type: 'common',
				slotNum: 42,
				status: 'ok',
				slots: [{ slotIndex: 0, staticItemId: 'Wood', count: 340 }],
				buildObjectId: null,
				baseId: null
			}
		]
	};
}

function palJson(): GamePalJson {
	return { characterId: 'Foxparks', instanceId: 'pal-1', ownerUid: 'p1', slotIndex: 0 };
}

function palsJson(overrides: Partial<GamePalsJson> = {}): GamePalsJson {
	return { pals: [palJson()], page: 0, pageCount: 1, slotCount: 30, slotBase: 0, containerId: 'c-1', party: [], partyStatus: 'ok', status: 'ok', ...overrides };
}

function deferred<T>() {
	let resolve!: (value: T) => void;
	let reject!: (reason: unknown) => void;
	const promise = new Promise<T>((res, rej) => {
		resolve = res;
		reject = rej;
	});
	promise.catch(() => {});
	return { promise, resolve, reject };
}

beforeEach(() => {
	sendAndWait.mockReset();
});

describe('GameState per-field request guards', () => {
	it('populates status, capabilities and players when refreshed concurrently, regardless of reply order', async () => {
		const gameState = new GameState();
		const status = deferred<GameStatusJson>();
		const caps = deferred<GameCapabilitiesJson>();
		const players = deferred<GamePlayersJson>();

		sendAndWait.mockImplementationOnce(() => status.promise);
		const statusPromise = gameState.refreshStatus();
		sendAndWait.mockImplementationOnce(() => caps.promise);
		const capsPromise = gameState.refreshCapabilities();
		sendAndWait.mockImplementationOnce(() => players.promise);
		const playersPromise = gameState.refreshPlayers();

		players.resolve(playersJson());
		await playersPromise;
		status.resolve(statusJson());
		await statusPromise;
		caps.resolve(capsJson());
		await capsPromise;

		expect(gameState.players).toEqual(playersJson().players);
		expect(gameState.status).toEqual(statusJson());
		expect(gameState.capabilities).toEqual(capsJson());
	});

	it('still discards a stale reply from an earlier call to the same method', async () => {
		const gameState = new GameState();
		const first = deferred<GameStatusJson>();
		const second = deferred<GameStatusJson>();

		sendAndWait.mockImplementationOnce(() => first.promise);
		const firstPromise = gameState.refreshStatus();
		sendAndWait.mockImplementationOnce(() => second.promise);
		const secondPromise = gameState.refreshStatus();

		second.resolve(statusJson({ queueDepth: 5 }));
		await secondPromise;
		expect(gameState.status?.queueDepth).toBe(5);

		first.resolve(statusJson({ queueDepth: 999 }));
		await firstPromise;

		expect(gameState.status?.queueDepth).toBe(5);
	});

	it('does not let a stale players reply clobber a newer status reply, or vice versa', async () => {
		const gameState = new GameState();
		const status = deferred<GameStatusJson>();
		const players = deferred<GamePlayersJson>();

		sendAndWait.mockImplementationOnce(() => status.promise);
		const statusPromise = gameState.refreshStatus();
		sendAndWait.mockImplementationOnce(() => players.promise);
		const playersPromise = gameState.refreshPlayers();

		status.resolve(statusJson());
		await statusPromise;
		expect(gameState.status).toEqual(statusJson());
		expect(gameState.players).toEqual([]);

		players.resolve(playersJson());
		await playersPromise;
		expect(gameState.players).toEqual(playersJson().players);
		expect(gameState.status).toEqual(statusJson());
	});
});

describe('GameState.loadInventory', () => {
	it('populates inventory on a successful reply', async () => {
		const gameState = new GameState();
		sendAndWait.mockResolvedValueOnce(inventoryJson());
		await gameState.loadInventory('p1');
		expect(gameState.inventory).toEqual(inventoryJson());
		expect(gameState.inventoryError).toBeNull();
		expect((sendAndWait.mock.calls[0][1] as { player_uid: string }).player_uid).toBe('p1');
	});

	it('records a refusal without touching stale inventory', async () => {
		const gameState = new GameState();
		const refusal = { error: 'world not loaded', code: 'capability_unavailable' };
		sendAndWait.mockResolvedValueOnce(refusal);
		await gameState.loadInventory('p1');
		expect(gameState.inventory).toBeNull();
		expect(gameState.inventoryError).toEqual(refusal);
	});

	it('discards a stale reply from an earlier call', async () => {
		const gameState = new GameState();
		const first = deferred<GameInventoryJson>();
		const second = deferred<GameInventoryJson>();

		sendAndWait.mockImplementationOnce(() => first.promise);
		const firstPromise = gameState.loadInventory('p1');
		sendAndWait.mockImplementationOnce(() => second.promise);
		const secondPromise = gameState.loadInventory('p2');

		second.resolve({ ...inventoryJson(), playerUid: 'p2' });
		await secondPromise;
		expect(gameState.inventory?.playerUid).toBe('p2');

		first.resolve({ ...inventoryJson(), playerUid: 'p1' });
		await firstPromise;
		expect(gameState.inventory?.playerUid).toBe('p2');
	});

	it('is not gated by an in-flight write', async () => {
		const gameState = new GameState();
		const write = deferred<GameCommandResultJson>();
		sendAndWait.mockImplementationOnce(() => write.promise);
		const inFlight = gameState.editPlayer('p1', 81, 100);
		expect(gameState.writeBusy).toBe(true);

		sendAndWait.mockResolvedValueOnce(inventoryJson());
		await gameState.loadInventory('p1');
		expect(gameState.inventory).toEqual(inventoryJson());
		expect(gameState.writeBusy).toBe(true);

		write.resolve(commandResult());
		await inFlight;
		expect(gameState.writeBusy).toBe(false);
	});
});

describe('GameState detail loads rejected by the transport', () => {
	it('clears the inventory and reports the bridge as offline', async () => {
		const gameState = new GameState();
		sendAndWait.mockResolvedValueOnce(inventoryJson());
		await gameState.loadInventory('p1');

		sendAndWait.mockRejectedValueOnce(new Error('socket closed'));
		await gameState.loadInventory('p2');

		expect(gameState.inventory).toBeNull();
		expect(gameState.inventoryError).toEqual({ error: 'socket closed', code: 'bridge_offline' });
	});

	it('clears the pal list and its paging, and reports the bridge as offline', async () => {
		const gameState = new GameState();
		sendAndWait.mockResolvedValueOnce(palsJson());
		await gameState.loadPals('p1');
		expect(gameState.pals).toHaveLength(1);

		sendAndWait.mockRejectedValueOnce(new Error('socket closed'));
		await gameState.loadPals('p2');

		expect(gameState.pals).toEqual([]);
		expect(gameState.palsPage).toBe(0);
		expect(gameState.palsPageCount).toBe(0);
		expect(gameState.palsSlotCount).toBe(0);
		expect(gameState.palsError).toEqual({ error: 'socket closed', code: 'bridge_offline' });
	});

	it('keeps the slot count the page reported', async () => {
		const gameState = new GameState();
		sendAndWait.mockResolvedValueOnce(palsJson({ page: 2, pageCount: 32, slotCount: 30, slotBase: 60 }));

		await gameState.loadPals('p1', 2);

		expect(gameState.palsPage).toBe(2);
		expect(gameState.palsPageCount).toBe(32);
		expect(gameState.palsSlotCount).toBe(30);
		expect(gameState.palsSlotBase).toBe(60);
	});

	it('lets a superseded inventory rejection pass without clearing the newer reply', async () => {
		const gameState = new GameState();
		const first = deferred<GameInventoryJson>();
		const second = deferred<GameInventoryJson>();

		sendAndWait.mockImplementationOnce(() => first.promise);
		const firstPromise = gameState.loadInventory('p1');
		sendAndWait.mockImplementationOnce(() => second.promise);
		const secondPromise = gameState.loadInventory('p2');

		second.resolve({ ...inventoryJson(), playerUid: 'p2' });
		await secondPromise;

		first.reject(new Error('socket closed'));
		await firstPromise;

		expect(gameState.inventory?.playerUid).toBe('p2');
		expect(gameState.inventoryError).toBeNull();
	});

	it('lets a superseded pals rejection pass without clearing the newer reply', async () => {
		const gameState = new GameState();
		const first = deferred<GamePalsJson>();
		const second = deferred<GamePalsJson>();

		sendAndWait.mockImplementationOnce(() => first.promise);
		const firstPromise = gameState.loadPals('p1');
		sendAndWait.mockImplementationOnce(() => second.promise);
		const secondPromise = gameState.loadPals('p2');

		second.resolve(palsJson({ pals: [{ ...palJson(), ownerUid: 'p2' }] }));
		await secondPromise;

		first.reject(new Error('socket closed'));
		await firstPromise;

		expect(gameState.pals).toEqual([{ ...palJson(), ownerUid: 'p2' }]);
		expect(gameState.palsError).toBeNull();
	});
});

function commandResult(overrides: Partial<GameCommandResultJson> = {}): GameCommandResultJson {
	return {
		commandId: 'server-assigned',
		op: 'player.edit',
		applied: true,
		verified: true,
		retrySafe: false,
		data: {},
		...overrides
	};
}

function sentCommandIds(): string[] {
	return sendAndWait.mock.calls.map((call) => (call[1] as { command_id: string }).command_id);
}

describe('GameState command id reuse', () => {
	it('mints a fresh id after a refusal the mod answered definitively', async () => {
		const gameState = new GameState();
		sendAndWait.mockResolvedValueOnce({ error: 'pal parameter not loaded', code: 'game_error' });
		await expect(gameState.editPlayer('p1', 81, 100)).rejects.toThrow('pal parameter not loaded');

		sendAndWait.mockResolvedValueOnce(commandResult());
		await gameState.editPlayer('p1', 81, 100);

		const [first, second] = sentCommandIds();
		expect(second).not.toBe(first);
	});

	it('reuses the id when the bridge was offline', async () => {
		const gameState = new GameState();
		sendAndWait.mockResolvedValueOnce({ error: 'bridge offline', code: 'bridge_offline' });
		await expect(gameState.editPlayer('p1', 81, 100)).rejects.toThrow('bridge offline');

		sendAndWait.mockResolvedValueOnce(commandResult());
		await gameState.editPlayer('p1', 81, 100);

		const [first, second] = sentCommandIds();
		expect(second).toBe(first);
	});

	it('reuses the id when the mod timed out', async () => {
		const gameState = new GameState();
		sendAndWait.mockResolvedValueOnce({ error: 'command timed out', code: 'timeout' });
		await expect(gameState.editPlayer('p1', 81, 100)).rejects.toThrow('command timed out');

		sendAndWait.mockResolvedValueOnce(commandResult());
		await gameState.editPlayer('p1', 81, 100);

		const [first, second] = sentCommandIds();
		expect(second).toBe(first);
	});

	it('reuses the id when the transport itself threw', async () => {
		const gameState = new GameState();
		sendAndWait.mockRejectedValueOnce(new Error('socket closed'));
		await expect(gameState.setItemSlot('p1', 'c1', 4, 'Wood', 5)).rejects.toThrow('socket closed');

		sendAndWait.mockResolvedValueOnce(commandResult({ op: 'item.setSlot' }));
		await gameState.setItemSlot('p1', 'c1', 4, 'Wood', 5);

		const [first, second] = sentCommandIds();
		expect(second).toBe(first);
	});

	it('mints a fresh base id when every heal target failed definitively', async () => {
		const gameState = new GameState();
		const targets = [{ slot_index: 0 }];
		const failed: GameHealPalsJson = {
			results: [
				{ slot_index: 0,
					ok: false,
					result: null,
					error: { code: 'not_authoritative', message: 'this instance cannot write' }
				}
			]
		};
		sendAndWait.mockResolvedValueOnce(failed);
		await gameState.healPals('p1', targets);

		sendAndWait.mockResolvedValueOnce({ results: [] });
		await gameState.healPals('p1', targets);

		const [first, second] = sentCommandIds();
		expect(second).not.toBe(first);
	});

	it('reuses the base id when a heal target failed with an unknown outcome', async () => {
		const gameState = new GameState();
		const targets = [{ slot_index: 0 }];
		const offline: GameHealPalsJson = {
			results: [
				{ slot_index: 0,
					ok: false,
					result: null,
					error: { code: 'timeout', message: 'command timed out' }
				}
			]
		};
		sendAndWait.mockResolvedValueOnce(offline);
		await gameState.healPals('p1', targets);

		sendAndWait.mockResolvedValueOnce({ results: [] });
		await gameState.healPals('p1', targets);

		const [first, second] = sentCommandIds();
		expect(second).toBe(first);
	});
});

describe('GameState single in-flight write gate', () => {
	it('rejects a second write while one is in flight, without sending it', async () => {
		const gameState = new GameState();
		const first = deferred<GameCommandResultJson>();
		sendAndWait.mockImplementationOnce(() => first.promise);

		const inFlight = gameState.editPlayer('p1', 81, 100);
		expect(gameState.writeBusy).toBe(true);

		await expect(gameState.editPlayer('p2', 81, 100)).rejects.toThrow('another change is still in flight');
		expect(sendAndWait).toHaveBeenCalledTimes(1);

		first.resolve(commandResult());
		await inFlight;
		expect(gameState.writeBusy).toBe(false);
	});

	it('gates writes of different types against each other too', async () => {
		const gameState = new GameState();
		const first = deferred<GameHealPalsJson>();
		sendAndWait.mockImplementationOnce(() => first.promise);

		const inFlight = gameState.healPals('p1', [{ slot_index: 0 }]);
		await expect(gameState.setItemSlot('p1', 'c1', 4, 'Wood', 5)).rejects.toThrow(
			'another change is still in flight'
		);

		first.resolve({ results: [] });
		await inFlight;
		expect(gameState.writeBusy).toBe(false);
	});

	it('clears the gate when the write fails, so the next attempt can run', async () => {
		const gameState = new GameState();
		sendAndWait.mockRejectedValueOnce(new Error('socket closed'));
		await expect(gameState.editPlayer('p1', 81, 100)).rejects.toThrow('socket closed');
		expect(gameState.writeBusy).toBe(false);

		sendAndWait.mockResolvedValueOnce(commandResult());
		await gameState.editPlayer('p1', 81, 100);
		expect(sendAndWait).toHaveBeenCalledTimes(2);
	});

	it('releases the gate when a write never settles, and treats it as an unknown outcome', async () => {
		vi.useFakeTimers();
		try {
			const gameState = new GameState();
			sendAndWait.mockImplementationOnce(() => new Promise(() => {}));

			const stranded = gameState.editPlayer('p1', 81, 100);
			const rejection = expect(stranded).rejects.toThrow('the game did not answer in time');
			expect(gameState.writeBusy).toBe(true);

			await vi.advanceTimersByTimeAsync(15_000);
			await rejection;
			expect(gameState.writeBusy).toBe(false);

			sendAndWait.mockResolvedValueOnce(commandResult());
			await gameState.editPlayer('p1', 81, 100);

			const [first, second] = sentCommandIds();
			expect(second).toBe(first);
		} finally {
			vi.useRealTimers();
		}
	});

	it('leaves reads ungated', async () => {
		const gameState = new GameState();
		const write = deferred<GameCommandResultJson>();
		sendAndWait.mockImplementationOnce(() => write.promise);
		const inFlight = gameState.editPlayer('p1', 81, 100);

		sendAndWait.mockResolvedValueOnce(playersJson());
		await gameState.refreshPlayers();
		expect(gameState.players).toEqual(playersJson().players);

		write.resolve(commandResult());
		await inFlight;
	});
});

describe('GameState setItemSlot', () => {
	type SetSlotPayload = {
		container_id: string;
		slot_index: number;
		static_item_id: string | null;
		count: number;
	};

	function sentSlotPayload(): SetSlotPayload {
		return sendAndWait.mock.calls[0][1] as SetSlotPayload;
	}

	it('addresses the slot by container and index', async () => {
		const gameState = new GameState();
		sendAndWait.mockResolvedValueOnce(commandResult({ op: 'item.setSlot' }));
		await gameState.setItemSlot('p1', 'container-1', 4, 'Wood', 20);

		const payload = sentSlotPayload();
		expect(payload.container_id).toBe('container-1');
		expect(payload.slot_index).toBe(4);
		expect(payload.static_item_id).toBe('Wood');
		expect(payload.count).toBe(20);
	});

	it('sends a null item when the count is zero', async () => {
		const gameState = new GameState();
		sendAndWait.mockResolvedValueOnce(commandResult({ op: 'item.setSlot' }));
		await gameState.setItemSlot('p1', 'container-1', 4, 'Wood', 0);

		const payload = sentSlotPayload();
		expect(payload.static_item_id).toBeNull();
		expect(payload.count).toBe(0);
	});

	it('sends a zero count when the item is null', async () => {
		const gameState = new GameState();
		sendAndWait.mockResolvedValueOnce(commandResult({ op: 'item.setSlot' }));
		await gameState.setItemSlot('p1', 'container-1', 4, null, 12);

		const payload = sentSlotPayload();
		expect(payload.static_item_id).toBeNull();
		expect(payload.count).toBe(0);
	});

	it('truncates a fractional count before it reaches the wire', async () => {
		const gameState = new GameState();
		sendAndWait.mockResolvedValueOnce(commandResult({ op: 'item.setSlot' }));
		await gameState.setItemSlot('p1', 'container-1', 4, 'Wood', 2.7);
		expect(sentSlotPayload().count).toBe(2);
	});

	it('keys the command id by slot', async () => {
		const gameState = new GameState();
		sendAndWait.mockRejectedValueOnce(new Error('socket closed'));
		await expect(gameState.setItemSlot('p1', 'c1', 4, 'Wood', 1)).rejects.toThrow('socket closed');

		sendAndWait.mockResolvedValueOnce(commandResult({ op: 'item.setSlot' }));
		await gameState.setItemSlot('p1', 'c1', 4, 'Stone', 9);

		sendAndWait.mockResolvedValueOnce(commandResult({ op: 'item.setSlot' }));
		await gameState.setItemSlot('p1', 'c1', 5, 'Wood', 1);

		const [first, second, third] = sentCommandIds();
		expect(second).toBe(first);
		expect(third).not.toBe(first);
	});
});

describe('GameState pal slot writes', () => {
	it('addresses a removal by slot', async () => {
		const gameState = new GameState();
		sendAndWait.mockResolvedValueOnce(commandResult({ op: 'pal.remove' }));
		await gameState.removePal('p1', 37);

		const payload = sendAndWait.mock.calls[0][1] as { player_uid: string; slot_index: number };
		expect(payload.player_uid).toBe('p1');
		expect(payload.slot_index).toBe(37);
	});

	it('sends both ends of a move', async () => {
		const gameState = new GameState();
		sendAndWait.mockResolvedValueOnce(commandResult({ op: 'pal.move' }));
		await gameState.movePal('p1', 4, 61);

		const payload = sendAndWait.mock.calls[0][1] as {
			from_slot_index: number;
			to_slot_index: number;
		};
		expect(payload.from_slot_index).toBe(4);
		expect(payload.to_slot_index).toBe(61);
	});

	it('keys a move by direction', async () => {
		const gameState = new GameState();
		sendAndWait.mockRejectedValueOnce(new Error('socket closed'));
		await expect(gameState.movePal('p1', 4, 61)).rejects.toThrow('socket closed');

		sendAndWait.mockResolvedValueOnce(commandResult({ op: 'pal.move' }));
		await gameState.movePal('p1', 4, 61);

		sendAndWait.mockResolvedValueOnce(commandResult({ op: 'pal.move' }));
		await gameState.movePal('p1', 61, 4);

		const [first, second, third] = sentCommandIds();
		expect(second).toBe(first);
		expect(third).not.toBe(first);
	});
});

describe('GameState addPal', () => {
	it('sends the slot and character it was given', async () => {
		const gameState = new GameState();
		sendAndWait.mockResolvedValueOnce(commandResult({ op: 'pal.add' }));
		await gameState.addPal('p1', 41, 'Anubis');

		const payload = sendAndWait.mock.calls[0][1] as {
			slot_index: number;
			character_id: string;
			level: number | null;
			gender: string | null;
		};
		expect(payload.slot_index).toBe(41);
		expect(payload.character_id).toBe('Anubis');
	});

	it('sends null for a level and gender it was not given', async () => {
		const gameState = new GameState();
		sendAndWait.mockResolvedValueOnce(commandResult({ op: 'pal.add' }));
		await gameState.addPal('p1', 41, 'Anubis');

		const payload = sendAndWait.mock.calls[0][1] as {
			level: number | null;
			gender: string | null;
		};
		expect(payload.level).toBeNull();
		expect(payload.gender).toBeNull();
	});

	it('truncates a fractional level before it reaches the wire', async () => {
		const gameState = new GameState();
		sendAndWait.mockResolvedValueOnce(commandResult({ op: 'pal.add' }));
		await gameState.addPal('p1', 41, 'Anubis', 12.9, 'female');

		const payload = sendAndWait.mock.calls[0][1] as { level: number; gender: string };
		expect(payload.level).toBe(12);
		expect(payload.gender).toBe('female');
	});
});

describe('GameState editPal', () => {
	type EditPayload = Record<string, unknown>;

	function sentEdit(): EditPayload {
		return sendAndWait.mock.calls[0][1] as EditPayload;
	}

	it('sends null for every field it was not given', async () => {
		const gameState = new GameState();
		sendAndWait.mockResolvedValueOnce(commandResult({ op: 'pal.edit' }));
		await gameState.editPal('p1', { slotIndex: 4 }, { level: 30 });

		const payload = sentEdit();
		expect(payload.level).toBe(30);
		for (const key of [
			'rank',
			'exp',
			'talent_hp',
			'talent_melee',
			'talent_shot',
			'talent_defense',
			'rank_hp',
			'rank_attack',
			'rank_defense',
			'rank_craft_speed',
			'nickname',
			'active_skills',
			'passive_skills',
			'work_suitability'
		]) {
			expect(payload[key]).toBeNull();
		}
	});

	it('carries the lists and the work map through', async () => {
		const gameState = new GameState();
		sendAndWait.mockResolvedValueOnce(commandResult({ op: 'pal.edit' }));
		await gameState.editPal('p1', { slotIndex: 4 }, {
			activeSkills: ['Waza1'],
			passiveSkills: ['Legend'],
			workSuitability: { Mining: 3 }
		});

		const payload = sentEdit();
		expect(payload.active_skills).toEqual(['Waza1']);
		expect(payload.passive_skills).toEqual(['Legend']);
		expect(payload.work_suitability).toEqual({ Mining: 3 });
	});

	it('carries every editable field the request can hold', async () => {
		const gameState = new GameState();
		sendAndWait.mockResolvedValueOnce(commandResult({ op: 'pal.edit' }));
		await gameState.editPal(
			'p1',
			{ slotIndex: 4 },
			{
				gender: 'female',
				sanity: 80,
				stomach: 150,
				hp: 500,
				friendshipPoint: 3,
				isAwakened: true,
				isLucky: false
			}
		);

		const payload = sentEdit();
		expect(payload.gender).toBe('female');
		expect(payload.sanity).toBe(80);
		expect(payload.stomach).toBe(150);
		expect(payload.hp).toBe(500);
		expect(payload.friendship_point).toBe(3);
		expect(payload.is_awakened).toBe(true);
		expect(payload.is_lucky).toBe(false);
	});

	it('addresses a party pal by its own id rather than by a slot', async () => {
		const gameState = new GameState();
		sendAndWait.mockResolvedValueOnce(commandResult({ op: 'pal.edit' }));
		await gameState.editPal('p1', { instanceId: 'pal-9' }, { level: 30 });

		const payload = sentEdit();
		expect(payload.instance_id).toBe('pal-9');
		expect(payload.slot_index).toBeNull();
	});

	it('keys the command id by slot', async () => {
		const gameState = new GameState();
		sendAndWait.mockRejectedValueOnce(new Error('socket closed'));
		await expect(gameState.editPal('p1', { slotIndex: 4 }, { level: 5 })).rejects.toThrow('socket closed');

		sendAndWait.mockResolvedValueOnce(commandResult({ op: 'pal.edit' }));
		await gameState.editPal('p1', { slotIndex: 4 }, { level: 9 });

		sendAndWait.mockResolvedValueOnce(commandResult({ op: 'pal.edit' }));
		await gameState.editPal('p1', { slotIndex: 5 }, { level: 9 });

		const [first, second, third] = sentCommandIds();
		expect(second).toBe(first);
		expect(third).not.toBe(first);
	});

	it('returns null rather than throwing when a detail read is refused', async () => {
		const gameState = new GameState();
		sendAndWait.mockResolvedValueOnce({ error: 'bridge is offline', code: 'bridge_offline' });
		expect(await gameState.loadPalDetail('p1', { slotIndex: 4 })).toBeNull();
	});
});

function guildJson(): GameGuildJson {
	return {
		guild: {
			id: 'g1',
			name: 'DeBugging',
			groupName: '00000000000000000000000000000001',
			baseCampLevel: 35,
			baseCampIds: ['b1', 'b2'],
			baseCampPointIds: ['p1', 'p2'],
			memberUids: ['00000000-0000-0000-0000-000000000001'],
			bases: null,
			adminUid: '00000000-0000-0000-0000-000000000001',
			roleOptions: null,
			members: null,
			lab: null
		},
		status: 'ok'
	};
}

describe('GameState.loadGuild', () => {
	it('populates the guild on a successful reply', async () => {
		const gameState = new GameState();
		sendAndWait.mockResolvedValueOnce(guildJson());
		await gameState.loadGuild({ playerUid: 'p1' });
		expect(gameState.guild).toEqual(guildJson());
		expect(gameState.guildError).toBeNull();
		expect((sendAndWait.mock.calls[0][1] as { player_uid: string }).player_uid).toBe('p1');
	});

	it('accepts a null guild as a successful answer', async () => {
		const gameState = new GameState();
		sendAndWait.mockResolvedValueOnce({ guild: null, status: 'ok' });
		await gameState.loadGuild({ playerUid: 'p1' });
		expect(gameState.guild).toEqual({ guild: null, status: 'ok' });
		expect(gameState.guildError).toBeNull();
	});

	it('records a refusal without touching a stale guild', async () => {
		const gameState = new GameState();
		const refusal = { error: 'world not loaded', code: 'capability_unavailable' };
		sendAndWait.mockResolvedValueOnce(refusal);
		await gameState.loadGuild({ playerUid: 'p1' });
		expect(gameState.guild).toBeNull();
		expect(gameState.guildError).toEqual(refusal);
	});

	it('discards a stale reply from an earlier call', async () => {
		const gameState = new GameState();
		const first = deferred<GameGuildJson>();
		const second = deferred<GameGuildJson>();

		sendAndWait.mockImplementationOnce(() => first.promise);
		const firstPromise = gameState.loadGuild({ playerUid: 'p1' });
		sendAndWait.mockImplementationOnce(() => second.promise);
		const secondPromise = gameState.loadGuild({ playerUid: 'p2' });

		second.resolve({ ...guildJson(), status: 'ok' });
		await secondPromise;
		first.resolve({ guild: null, status: 'ok' });
		await firstPromise;

		expect(gameState.guild?.guild?.name).toBe('DeBugging');
	});
});

function basePalsJson(): GameBasePalsJson {
	return {
		baseId: 'b1',
		containerId: 'c1',
		slotNum: 15,
		pals: [{ instanceId: 'i1', characterId: 'Anubis', slotIndex: 0, level: 60 } as GamePalJson],
		status: 'ok'
	};
}

describe('GameState.loadBasePals', () => {
	it('populates base pals on a successful reply', async () => {
		const gameState = new GameState();
		sendAndWait.mockResolvedValueOnce(basePalsJson());
		await gameState.loadBasePals('b1');
		expect(gameState.basePals).toEqual(basePalsJson());
		expect(gameState.basePalsError).toBeNull();
		expect((sendAndWait.mock.calls[0][1] as { base_id: string }).base_id).toBe('b1');
	});

	it('records a refusal without touching stale base pals', async () => {
		const gameState = new GameState();
		const refusal = { error: 'unknown baseId', code: 'validation_failed' };
		sendAndWait.mockResolvedValueOnce(refusal);
		await gameState.loadBasePals('b1');
		expect(gameState.basePals).toBeNull();
		expect(gameState.basePalsError).toEqual(refusal);
	});

	it('discards a stale reply from an earlier base', async () => {
		const gameState = new GameState();
		const first = deferred<GameBasePalsJson>();
		const second = deferred<GameBasePalsJson>();

		sendAndWait.mockImplementationOnce(() => first.promise);
		const firstPromise = gameState.loadBasePals('b1');
		sendAndWait.mockImplementationOnce(() => second.promise);
		const secondPromise = gameState.loadBasePals('b2');

		second.resolve({ ...basePalsJson(), baseId: 'b2' });
		await secondPromise;
		first.resolve(basePalsJson());
		await firstPromise;

		expect(gameState.basePals?.baseId).toBe('b2');
	});
});

describe('GameState.loadGuilds', () => {
	it('lists every guild in the world', async () => {
		const gameState = new GameState();
		const reply = {
			guilds: [{ id: 'g1', name: 'Gobblin', adminUid: null, members: [] }],
			status: 'ok'
		};
		sendAndWait.mockResolvedValueOnce(reply);
		await gameState.loadGuilds();
		expect(gameState.guilds).toEqual(reply);
	});

	it('clears a stale list on a refusal', async () => {
		const gameState = new GameState();
		sendAndWait.mockResolvedValueOnce({
			guilds: [{ id: 'g1', name: 'Gobblin', adminUid: null, members: [] }],
			status: 'ok'
		});
		await gameState.loadGuilds();
		sendAndWait.mockResolvedValueOnce({ error: 'world not loaded', code: 'capability_unavailable' });
		await gameState.loadGuilds();
		expect(gameState.guilds).toBeNull();
	});
});

describe('GameState.loadGuild addressing', () => {
	it('asks by guild id when given one', async () => {
		const gameState = new GameState();
		sendAndWait.mockResolvedValueOnce(guildJson());
		await gameState.loadGuild({ guildId: 'g9' });
		expect(sendAndWait.mock.calls[0][1]).toEqual({ guild_id: 'g9' });
	});

	it('asks by player when given a player', async () => {
		const gameState = new GameState();
		sendAndWait.mockResolvedValueOnce(guildJson());
		await gameState.loadGuild({ playerUid: 'p1' });
		expect(sendAndWait.mock.calls[0][1]).toEqual({ player_uid: 'p1' });
	});
});
