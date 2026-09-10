import type {
	GameCapabilitiesJson,
	GameCapabilityOp,
	GameErrorJson,
	GameHealResultEntry,
	GameInventoryContainerJson,
	GameItemDynamicJson,
	GamePalDetailJson,
	GamePalJson,
	GameRefusalJson,
	GameStatusJson,
	HealTargetRequest
} from '$states/gameState.svelte';
import {
	EntryState,
	PalGender,
	type DynamicItem,
	type DynamicItemClass,
	type ItemContainerSlot,
	type Pal,
	type PalData,
	type WorkSuitability
} from '$types';

export const MIN_LEVEL = 1;
export const MAX_LEVEL = 100;

export interface PalAddress {
	slotIndex?: number;
	instanceId?: string;
}

export function deriveCharacterKey(characterId: string): string {
	let key = characterId.toLowerCase();
	if (key.startsWith('boss_')) {
		key = key.slice(5);
	} else if (key.startsWith('predator_')) {
		key = key.slice(9);
	} else if (key.endsWith('_avatar')) {
		key = key.slice(0, -7);
	}
	return key;
}

function toPalGender(gender: string | undefined): PalGender {
	switch (gender?.toLowerCase()) {
		case 'male':
			return PalGender.MALE;
		case 'female':
			return PalGender.FEMALE;
		default:
			return PalGender.NONE;
	}
}

export function toLivePal(
	gamePal: GamePalJson | undefined,
	palData: PalData | undefined,
	detail?: GamePalDetailJson | null
): Pal {
	const character_key = gamePal ? deriveCharacterKey(gamePal.characterId) : 'None';
	const upperId = gamePal?.characterId?.toUpperCase() ?? '';
	const is_lucky = gamePal?.isLucky ?? false;
	return {
		name: palData?.localized_name ?? gamePal?.characterId ?? 'None',
		instance_id: gamePal?.instanceId ?? '',
		owner_uid: gamePal?.ownerUid ?? '',
		character_id: gamePal?.characterId ?? 'None',
		character_key,
		is_lucky,
		is_boss: upperId.startsWith('BOSS_') && !is_lucky,
		is_predator: upperId.startsWith('PREDATOR_'),
		is_awakened: gamePal?.isAwakened ?? false,
		is_imported: false,
		friendship_point: detail?.friendshipPoint ?? 0,
		gender: toPalGender(gamePal?.gender),
		rank_hp: detail?.rankHp ?? 0,
		rank_attack: detail?.rankAttack ?? 0,
		rank_defense: detail?.rankDefense ?? 0,
		rank_craftspeed: detail?.rankCraftSpeed ?? 0,
		talent_hp: detail?.talentHp ?? 0,
		talent_shot: detail?.talentShot ?? 0,
		talent_defense: detail?.talentDefense ?? 0,
		rank: detail?.rank ?? 1,
		level: gamePal?.level ?? 0,
		nickname: gamePal?.nickname,
		is_tower: false,
		stomach: detail?.stomach ?? 0,
		storage_slot: gamePal?.slotIndex ?? 0,
		learned_skills: detail?.masteredWaza ?? [],
		active_skills: detail?.equipWaza ?? [],
		passive_skills: detail?.passiveSkills ?? [],
		work_suitability: (detail?.workSuitability ?? {}) as Record<WorkSuitability, number>,
		hp: gamePal?.hp ?? 0,
		max_hp: gamePal?.maxHp ?? 0,
		elements: palData?.element_types ?? [],
		state: EntryState.NONE,
		sanity: detail?.sanity ?? 0,
		exp: detail?.exp ?? 0,
		is_sick: (gamePal?.isSick ?? false) || (gamePal?.isFainted ?? false)
	};
}

export function opAvailable(
	status: GameStatusJson | null,
	caps: GameCapabilitiesJson | null,
	op: GameCapabilityOp
): { available: boolean; reason: string | null } {
	if (!status?.authoritative) return { available: false, reason: 'live_not_authoritative' };
	if (!status?.worldLoaded) return { available: false, reason: 'live_world_not_loaded' };
	const entry = caps?.ops[op];
	if (!entry) return { available: false, reason: null };
	return { available: entry.available, reason: entry.reason ?? null };
}

export function describeHealResults(results: GameHealResultEntry[]): {
	healed: number;
	unconfirmed: number;
	failed: number;
} {
	let healed = 0;
	let unconfirmed = 0;
	let failed = 0;
	for (const entry of results) {
		if (!entry.ok) failed++;
		else if (entry.result?.applied && entry.result?.verified) healed++;
		else unconfirmed++;
	}
	return { healed, unconfirmed, failed };
}

export function errorDisplayKey(code: string): string | null {
	switch (code) {
		case 'bridge_offline':
			return 'live_offline_hint';
		case 'timeout':
			return 'live_timeout_hint';
		case 'not_authoritative':
			return 'live_not_authoritative';
		case 'capability_unavailable':
			return 'live_capability_unavailable';
		default:
			return null;
	}
}

export function keepsRawMessage(code: string): boolean {
	return code === 'capability_unavailable';
}

export function statusModeLabelKey(mode: string): string {
	if (mode === 'solo' || mode === 'coop_host') return 'live_mode_solo_host';
	if (mode === 'dedicated') return 'live_mode_dedicated';
	return 'live_mode_client';
}

export function statusWarningKey(status: GameStatusJson | null): string | null {
	if (!status) return null;
	if (!status.authoritative) return 'live_not_authoritative';
	if (!status.worldLoaded) return 'live_world_not_loaded';
	return null;
}

export type BridgeChipState = 'ok' | 'read_only' | 'offline' | 'error';

export function bridgeChipState(
	status: GameStatusJson | null,
	error: GameRefusalJson | GameErrorJson | null
): BridgeChipState {
	if (error) {
		return error.code === 'bridge_offline' || error.code === 'timeout' ? 'offline' : 'error';
	}
	if (!status) return 'offline';
	return status.authoritative ? 'ok' : 'read_only';
}

export function healActionKey(playerUid: string, targets: HealTargetRequest[]): string {
	const sorted = [...targets].sort((a, b) => a.slot_index - b.slot_index);
	return `heal:${playerUid}:${sorted.map((t) => t.slot_index).join(',')}`;
}

export function removePalActionKey(playerUid: string, slotIndex: number): string {
	return `remove_pal:${playerUid}:${slotIndex}`;
}

export function editPalActionKey(playerUid: string, address: PalAddress): string {
	const target = address.instanceId ?? `slot:${address.slotIndex}`;
	return `edit_pal:${playerUid}:${target}`;
}

export function editPlayerActionKey(playerUid: string, level: number): string {
	return `edit_player:${playerUid}:${level}`;
}

export function editGuildActionKey(guildId: string, baseCampLevel: number): string {
	return `edit_guild:${guildId}:${baseCampLevel}`;
}

export function setGuildRoleActionKey(guildId: string, memberUid: string, role: string): string {
	return `set_guild_role:${guildId}:${memberUid}:${role}`;
}

export interface PalAddTarget {
	baseId?: string;
	party?: boolean;
}

export function addPalActionKey(
	playerUid: string,
	slotIndex: number | null,
	target: PalAddTarget = {}
): string {
	const where = target.party ? 'party' : target.baseId ? `base:${target.baseId}` : 'palbox';
	return `add_pal:${playerUid}:${where}:${slotIndex ?? 'any'}`;
}

export function movePalActionKey(
	playerUid: string,
	fromSlotIndex: number,
	toSlotIndex: number
): string {
	return `move_pal:${playerUid}:${fromSlotIndex}:${toSlotIndex}`;
}

export function setItemSlotActionKey(
	playerUid: string,
	containerId: string,
	slotIndex: number
): string {
	return `set_item_slot:${playerUid}:${containerId}:${slotIndex}`;
}

export class CommandIdTracker {
	#attempts = new Map<string, { commandId: string; unknown: boolean }>();
	#generateId: () => string;

	constructor(generateId: () => string = () => crypto.randomUUID()) {
		this.#generateId = generateId;
	}

	next(key: string): string {
		const existing = this.#attempts.get(key);
		if (existing?.unknown) return existing.commandId;
		const commandId = this.#generateId();
		this.#attempts.set(key, { commandId, unknown: false });
		return commandId;
	}

	recordOutcome(key: string, unknown: boolean): void {
		const existing = this.#attempts.get(key);
		if (existing) existing.unknown = unknown;
	}
}

export class RequestGuard {
	#generation = 0;

	next(): number {
		return ++this.#generation;
	}

	isCurrent(ticket: number): boolean {
		return ticket === this.#generation;
	}
}

const EMPTY_SLOT_ID = 'None';

function toDynamicItem(dynamic: GameItemDynamicJson | null | undefined): DynamicItem | undefined {
	if (!dynamic) return undefined;
	return {
		local_id: dynamic.localId,
		durability: dynamic.durability ?? 0,
		remaining_bullets: dynamic.remainingBullets ?? 0,
		type: dynamic.type as DynamicItemClass,
		character_id: dynamic.characterId ?? undefined,
		gender: '',
		talent_hp: 0,
		talent_shot: 0,
		talent_defense: 0,
		learned_skills: [],
		active_skills: [],
		passive_skills: dynamic.passiveSkills ?? [],
		modified: false
	};
}

export function containerSlots(container: GameInventoryContainerJson): ItemContainerSlot[] {
	const bySlot = new Map(container.slots.map((slot) => [slot.slotIndex, slot]));
	let size = container.slotNum ?? 0;
	for (const slot of container.slots) {
		if (slot.slotIndex + 1 > size) size = slot.slotIndex + 1;
	}
	const slots: ItemContainerSlot[] = [];
	for (let index = 0; index < size; index++) {
		const slot = bySlot.get(index);
		slots.push({
			slot_index: index,
			static_id: slot?.staticItemId ?? EMPTY_SLOT_ID,
			count: slot?.count ?? 0,
			dynamic_item: toDynamicItem(slot?.dynamicItem)
		});
	}
	return slots;
}
