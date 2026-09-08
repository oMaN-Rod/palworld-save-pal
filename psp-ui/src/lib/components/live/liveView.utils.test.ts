import type {
	GameCapabilitiesJson,
	GameCommandResultJson,
	GameHealResultEntry,
	GameInventoryContainerJson,
	GamePalDetailJson,
	GamePalJson,
	GameStatusJson
} from '$states/gameState.svelte';
import { describe, expect, it } from 'vitest';
import {
	addPalActionKey,
	bridgeChipState,
	CommandIdTracker,
	containerSlots,
	describeHealResults,
	errorDisplayKey,
	healActionKey,
	keepsRawMessage,
	opAvailable,
	RequestGuard,
	toLivePal,
	statusModeLabelKey,
	statusWarningKey
} from './liveView.utils';

function status(overrides: Partial<GameStatusJson> = {}): GameStatusJson {
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

function caps(overrides: Partial<GameCapabilitiesJson['ops']> = {}): GameCapabilitiesJson {
	return {
		version: 1,
		ops: {
			'pal.heal': { available: true, reason: null },
			'player.edit': { available: true, reason: null },
			'item.setSlot': { available: false, reason: 'item database not loaded' },
			...overrides
		}
	};
}

describe('opAvailable', () => {
	it('returns the op entry when present and the connection can write', () => {
		expect(opAvailable(status(), caps(), 'item.setSlot')).toEqual({
			available: false,
			reason: 'item database not loaded'
		});
	});

	it('defaults reason to null when the entry omits it', () => {
		expect(opAvailable(status(), caps(), 'pal.heal')).toEqual({ available: true, reason: null });
	});

	it('treats missing capabilities as unavailable', () => {
		expect(opAvailable(status(), null, 'pal.heal')).toEqual({ available: false, reason: null });
	});

	it('treats an op absent from the payload as unavailable', () => {
		const partial = { version: 1, ops: {} } as GameCapabilitiesJson;
		expect(opAvailable(status(), partial, 'pal.heal')).toEqual({
			available: false,
			reason: null
		});
	});

	it('is unavailable with live_not_authoritative when the connection is not authoritative, regardless of caps', () => {
		expect(opAvailable(status({ authoritative: false }), caps(), 'pal.heal')).toEqual({
			available: false,
			reason: 'live_not_authoritative'
		});
	});

	it('is unavailable with live_world_not_loaded when the world is not loaded, regardless of caps', () => {
		expect(opAvailable(status({ worldLoaded: false }), caps(), 'pal.heal')).toEqual({
			available: false,
			reason: 'live_world_not_loaded'
		});
	});

	it('checks authoritative before world-loaded when both are false', () => {
		expect(
			opAvailable(status({ authoritative: false, worldLoaded: false }), caps(), 'pal.heal')
		).toEqual({ available: false, reason: 'live_not_authoritative' });
	});

	it('treats missing status the same as not authoritative', () => {
		expect(opAvailable(null, caps(), 'pal.heal')).toEqual({
			available: false,
			reason: 'live_not_authoritative'
		});
	});
});

describe('describeHealResults', () => {
	function commandResult(overrides: Partial<GameCommandResultJson> = {}): GameCommandResultJson {
		return {
			commandId: 'c1',
			op: 'pal.heal',
			applied: true,
			verified: true,
			retrySafe: true,
			data: {},
			...overrides
		};
	}

	function entry(
		ok: boolean,
		result: GameCommandResultJson | null = ok ? commandResult() : null
	): GameHealResultEntry {
		return { slot_index: 0, ok, result, error: null };
	}

	it('counts an empty array as nothing healed, unconfirmed or failed', () => {
		expect(describeHealResults([])).toEqual({ healed: 0, unconfirmed: 0, failed: 0 });
	});

	it('counts an applied and verified entry as healed', () => {
		expect(describeHealResults([entry(true), entry(false)])).toEqual({
			healed: 1,
			unconfirmed: 0,
			failed: 1
		});
	});

	it('does not count an unverified entry as healed', () => {
		expect(describeHealResults([entry(true, commandResult({ verified: false }))])).toEqual({
			healed: 0,
			unconfirmed: 1,
			failed: 0
		});
	});

	it('does not count an unapplied entry as healed', () => {
		expect(describeHealResults([entry(true, commandResult({ applied: false }))])).toEqual({
			healed: 0,
			unconfirmed: 1,
			failed: 0
		});
	});

	it('treats an ok entry with no result payload as unconfirmed, not healed', () => {
		expect(describeHealResults([entry(true, null)])).toEqual({
			healed: 0,
			unconfirmed: 1,
			failed: 0
		});
	});
});

describe('errorDisplayKey', () => {
	it('maps bridge_offline to the offline hint', () => {
		expect(errorDisplayKey('bridge_offline')).toBe('live_offline_hint');
	});

	it('gives a timeout its own key rather than the offline hint', () => {
		expect(errorDisplayKey('timeout')).toBe('live_timeout_hint');
	});

	it('maps not_authoritative', () => {
		expect(errorDisplayKey('not_authoritative')).toBe('live_not_authoritative');
	});

	it('maps capability_unavailable', () => {
		expect(errorDisplayKey('capability_unavailable')).toBe('live_capability_unavailable');
	});

	it('leaves validation_failed and game_error to show their own message', () => {
		expect(errorDisplayKey('validation_failed')).toBeNull();
		expect(errorDisplayKey('game_error')).toBeNull();
	});

	it('leaves an unknown code to show its own message', () => {
		expect(errorDisplayKey('something_new')).toBeNull();
	});
});

describe('keepsRawMessage', () => {
	it('keeps the mod reason behind capability_unavailable', () => {
		expect(keepsRawMessage('capability_unavailable')).toBe(true);
	});

	it('does not append the raw message to codes whose key says it all', () => {
		expect(keepsRawMessage('bridge_offline')).toBe(false);
		expect(keepsRawMessage('timeout')).toBe(false);
		expect(keepsRawMessage('not_authoritative')).toBe(false);
	});
});

describe('statusModeLabelKey', () => {
	it('labels solo as Solo/Host', () => {
		expect(statusModeLabelKey('solo')).toBe('live_mode_solo_host');
	});

	it('labels coop_host as Solo/Host', () => {
		expect(statusModeLabelKey('coop_host')).toBe('live_mode_solo_host');
	});

	it('labels dedicated', () => {
		expect(statusModeLabelKey('dedicated')).toBe('live_mode_dedicated');
	});

	it('labels coop_client (and anything else) as client', () => {
		expect(statusModeLabelKey('coop_client')).toBe('live_mode_client');
		expect(statusModeLabelKey('unknown')).toBe('live_mode_client');
	});
});

describe('statusWarningKey', () => {
	it('returns null with no status yet', () => {
		expect(statusWarningKey(null)).toBeNull();
	});

	it('flags a non-authoritative connection first', () => {
		expect(statusWarningKey(status({ authoritative: false, worldLoaded: false }))).toBe(
			'live_not_authoritative'
		);
	});

	it('flags an unloaded world once authoritative', () => {
		expect(statusWarningKey(status({ worldLoaded: false }))).toBe('live_world_not_loaded');
	});

	it('returns null once authoritative and loaded', () => {
		expect(statusWarningKey(status())).toBeNull();
	});
});

describe('bridgeChipState', () => {
	it('is offline with no status and no error', () => {
		expect(bridgeChipState(null, null)).toBe('offline');
	});

	it('is ok when authoritative', () => {
		expect(bridgeChipState(status(), null)).toBe('ok');
	});

	it('is read_only when not authoritative', () => {
		expect(bridgeChipState(status({ authoritative: false }), null)).toBe('read_only');
	});

	it('is offline for a bridge_offline or timeout error, regardless of stale status', () => {
		expect(bridgeChipState(status(), { error: 'down', code: 'bridge_offline' })).toBe('offline');
		expect(bridgeChipState(status(), { error: 'slow', code: 'timeout' })).toBe('offline');
	});

	it('is error for any other refusal code', () => {
		expect(bridgeChipState(status(), { error: 'bad', code: 'validation_failed' })).toBe('error');
	});
});

describe('action keys', () => {
	it('heal key is stable regardless of target order', () => {
		const a = healActionKey('p1', [{ slot_index: 32 }, { slot_index: 0 }]);
		const b = healActionKey('p1', [{ slot_index: 0 }, { slot_index: 32 }]);
		expect(a).toBe(b);
	});

	it('heal key differs for a different target set', () => {
		const a = healActionKey('p1', [{ slot_index: 0 }]);
		const b = healActionKey('p1', [{ slot_index: 1 }]);
		expect(a).not.toBe(b);
	});

	it('add key separates the same slot in different places', () => {
		const palbox = addPalActionKey('p1', 3);
		const base = addPalActionKey('p1', 3, { baseId: 'b1' });
		const otherBase = addPalActionKey('p1', 3, { baseId: 'b2' });
		expect(new Set([palbox, base, otherBase]).size).toBe(3);
	});

	it('add key is stable for the party, which names no slot', () => {
		expect(addPalActionKey('p1', null, { party: true })).toBe(
			addPalActionKey('p1', null, { party: true })
		);
	});
});

describe('CommandIdTracker', () => {
	it('generates a fresh id for a key seen for the first time', () => {
		let n = 0;
		const tracker = new CommandIdTracker(() => `id-${++n}`);
		expect(tracker.next('a')).toBe('id-1');
	});

	it('reuses the id when the outcome of the last attempt is unknown', () => {
		let n = 0;
		const tracker = new CommandIdTracker(() => `id-${++n}`);
		const first = tracker.next('a');
		tracker.recordOutcome('a', true);
		expect(tracker.next('a')).toBe(first);
	});

	it('issues a new id once the key has a definite outcome', () => {
		let n = 0;
		const tracker = new CommandIdTracker(() => `id-${++n}`);
		const first = tracker.next('a');
		tracker.recordOutcome('a', false);
		const second = tracker.next('a');
		expect(second).not.toBe(first);
	});

	it('keeps each key independent', () => {
		let n = 0;
		const tracker = new CommandIdTracker(() => `id-${++n}`);
		const a = tracker.next('a');
		tracker.recordOutcome('a', true);
		const b = tracker.next('b');
		expect(b).not.toBe(a);
		expect(tracker.next('a')).toBe(a);
	});

	it('never resolved (no recordOutcome) still issues a new id on the next click', () => {
		let n = 0;
		const tracker = new CommandIdTracker(() => `id-${++n}`);
		const first = tracker.next('a');
		const second = tracker.next('a');
		expect(second).not.toBe(first);
	});
});

describe('RequestGuard', () => {
	it('accepts the only ticket issued so far', () => {
		const guard = new RequestGuard();
		const ticket = guard.next();
		expect(guard.isCurrent(ticket)).toBe(true);
	});

	it('rejects a ticket once a newer one has been issued on the same guard', () => {
		const guard = new RequestGuard();
		const first = guard.next();
		guard.next();
		expect(guard.isCurrent(first)).toBe(false);
	});

	it('accepts the newest ticket regardless of resolution order', () => {
		const guard = new RequestGuard();
		guard.next();
		const second = guard.next();
		expect(guard.isCurrent(second)).toBe(true);
	});

	it('keeps independent guards independent, unlike a single shared counter', () => {
		const statusGuard = new RequestGuard();
		const playersGuard = new RequestGuard();

		const statusTicket = statusGuard.next();
		playersGuard.next();

		expect(statusGuard.isCurrent(statusTicket)).toBe(true);
	});
});

describe('containerSlots', () => {
	function container(
		overrides: Partial<GameInventoryContainerJson> = {}
	): GameInventoryContainerJson {
		return {
			containerId: 'container-1',
			type: 'common',
			slotNum: 4,
			status: 'ok',
			slots: [],
			buildObjectId: null,
			baseId: null,
			...overrides
		};
	}

	it('fills the gaps a sparse payload leaves with the empty-slot sentinel', () => {
		const slots = containerSlots(
			container({ slots: [{ slotIndex: 1, staticItemId: 'Wood', count: 3 }] })
		);

		expect(slots.map((slot) => slot.static_id)).toEqual(['None', 'Wood', 'None', 'None']);
		expect(slots.map((slot) => slot.slot_index)).toEqual([0, 1, 2, 3]);
		expect(slots[1].count).toBe(3);
	});

	it('grows past the declared size for a slot index that reaches beyond it', () => {
		const slots = containerSlots(
			container({ slotNum: 2, slots: [{ slotIndex: 5, staticItemId: 'Shield', count: 1 }] })
		);

		expect(slots).toHaveLength(6);
		expect(slots[5].static_id).toBe('Shield');
	});

	it('renders a container that could not be read as an empty grid', () => {
		expect(containerSlots(container({ slotNum: null }))).toEqual([]);
	});

	it('carries a dynamic item across without inventing pal data for it', () => {
		const [slot] = containerSlots(
			container({
				slotNum: 1,
				slots: [
					{
						slotIndex: 0,
						staticItemId: 'AssaultRifle',
						count: 1,
						dynamicItem: {
							characterId: null,
							durability: 42,
							localId: 'local-1',
							maxDurability: 100,
							passiveSkills: ['Legend'],
							remainingBullets: 7,
							type: 'weapon'
						}
					}
				]
			})
		);

		expect(slot.dynamic_item?.durability).toBe(42);
		expect(slot.dynamic_item?.remaining_bullets).toBe(7);
		expect(slot.dynamic_item?.character_id).toBeUndefined();
	});
});

describe('toLivePal', () => {
	const base: GamePalJson = {
		characterId: 'Foxparks',
		instanceId: 'abc',
		ownerUid: 'owner',
		slotIndex: 3
	};

	it('marks a pal the game reports as ill or downed so the badge can show it', () => {
		expect(toLivePal({ ...base, isSick: true }, undefined).is_sick).toBe(true);
		expect(toLivePal({ ...base, isFainted: true }, undefined).is_sick).toBe(true);
		expect(toLivePal({ ...base, isSick: false, isFainted: false }, undefined).is_sick).toBe(false);
	});

	it('reads the gender the game reports into the spelling the editor uses', () => {
		expect(toLivePal({ ...base, gender: 'male' }, undefined).gender).toBe('Male');
		expect(toLivePal({ ...base, gender: 'female' }, undefined).gender).toBe('Female');
		expect(toLivePal({ ...base, gender: 'none' }, undefined).gender).toBe('None');
		expect(toLivePal({ ...base, gender: undefined }, undefined).gender).toBe('None');
	});

	it('carries awakening and health through', () => {
		const pal = toLivePal({ ...base, isAwakened: true, hp: 1200, maxHp: 5000 }, undefined);
		expect(pal.is_awakened).toBe(true);
		expect(pal.hp).toBe(1200);
		expect(pal.max_hp).toBe(5000);
	});

	it('reads alpha and predator off the character id, and never calls a lucky pal an alpha', () => {
		expect(toLivePal({ ...base, characterId: 'BOSS_Foxparks' }, undefined).is_boss).toBe(true);
		expect(
			toLivePal({ ...base, characterId: 'BOSS_Foxparks', isLucky: true }, undefined).is_boss
		).toBe(false);
		expect(toLivePal({ ...base, characterId: 'PREDATOR_Foxparks' }, undefined).is_predator).toBe(
			true
		);
	});

	it('fills the stats from the detail read, and zeroes them without one', () => {
		const detail = {
			...base,
			status: 'ok',
			exp: 4200,
			rank: 3,
			rankHp: 10,
			talentHp: 77,
			equipWaza: ['EPalWazaID::FireBall'],
			passiveSkills: ['Legend'],
			workSuitability: { Handcraft: 2 },
			sanity: 88,
			stomach: 120,
			friendshipPoint: 900
		} as GamePalDetailJson;

		const filled = toLivePal(base, undefined, detail);
		expect(filled.exp).toBe(4200);
		expect(filled.rank).toBe(3);
		expect(filled.rank_hp).toBe(10);
		expect(filled.talent_hp).toBe(77);
		expect(filled.active_skills).toEqual(['EPalWazaID::FireBall']);
		expect(filled.passive_skills).toEqual(['Legend']);
		expect(filled.work_suitability).toEqual({ Handcraft: 2 });
		expect(filled.sanity).toBe(88);
		expect(filled.stomach).toBe(120);
		expect(filled.friendship_point).toBe(900);

		const bare = toLivePal(base, undefined);
		expect(bare.exp).toBe(0);
		expect(bare.rank).toBe(1);
		expect(bare.active_skills).toEqual([]);
		expect(bare.work_suitability).toEqual({});
	});
});
