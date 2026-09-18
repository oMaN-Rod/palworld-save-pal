import {
	addPalActionKey,
	CommandIdTracker,
	editGuildActionKey,
	editPalActionKey,
	editPlayerActionKey,
	healActionKey,
	movePalActionKey,
	removePalActionKey,
	RequestGuard,
	setGuildRoleActionKey,
	setItemSlotActionKey,
	type PalAddress,
	type PalAddTarget
} from '$lib/components/live/liveView.utils';
import { MessageType } from '$types';
import { sendAndWait } from '$utils/websocketUtils';

export interface GameStatusJson {
	authoritative: boolean;
	modVersion: string;
	mode: string;
	protocolVersion: number;
	queueDepth: number;
	worldLoaded: boolean;
}

export interface GameInstanceJson {
	id: string;
	source: 'auto' | 'saved';
	name: string;
	host: string;
	port: number;
	live: boolean;
	targetId: string | null;
}

export interface GameInstancesJson {
	instances: GameInstanceJson[];
	activeId: string | null;
}

export interface GameInstanceFields {
	name: string;
	host: string;
	port: number;
	token: string;
}

export interface GameTestInstanceJson {
	ok: boolean;
	modVersion?: string;
	error?: string;
}

export interface GamePlayerJson {
	uid: string;
	nickname?: string;
	level?: number;
	exp?: number;
	guildId?: string;
	x?: number;
	y?: number;
	z?: number;
	yaw?: number;
	status: string;
}

export interface GamePlayersJson {
	players: GamePlayerJson[];
	status: string;
}

export interface GamePalJson {
	characterId: string;
	gender?: string;
	instanceId: string;
	playerUid?: string | null;
	isLucky?: boolean;
	isSick?: boolean;
	isFainted?: boolean;
	isAwakened?: boolean;
	hp?: number;
	maxHp?: number;
	level?: number;
	nickname?: string;
	ownerUid: string;
	slotIndex: number;
}

export interface GamePalDetailJson extends GamePalJson {
	exp?: number;
	rank?: number;
	rankHp?: number;
	rankAttack?: number;
	rankDefense?: number;
	rankCraftSpeed?: number;
	talentHp?: number;
	talentShot?: number;
	talentDefense?: number;
	equipWaza?: string[];
	masteredWaza?: string[];
	passiveSkills?: string[];
	workSuitability?: Record<string, number>;
	sanity?: number;
	stomach?: number;
	maxStomach?: number;
	friendshipPoint?: number;
	status: string;
}

export interface GamePalEditRequest {
	level?: number;
	rank?: number;
	exp?: number;
	talentHp?: number;
	talentMelee?: number;
	talentShot?: number;
	talentDefense?: number;
	rankHp?: number;
	rankAttack?: number;
	rankDefense?: number;
	rankCraftSpeed?: number;
	nickname?: string;
	activeSkills?: string[];
	passiveSkills?: string[];
	workSuitability?: Record<string, number>;
	friendshipPoint?: number;
	sanity?: number;
	stomach?: number;
	hp?: number;
	gender?: string;
	isAwakened?: boolean;
	isLucky?: boolean;
}

const EDIT_PAL_WIRE_NAMES: Record<keyof GamePalEditRequest, string> = {
	level: 'level',
	rank: 'rank',
	exp: 'exp',
	talentHp: 'talent_hp',
	talentMelee: 'talent_melee',
	talentShot: 'talent_shot',
	talentDefense: 'talent_defense',
	rankHp: 'rank_hp',
	rankAttack: 'rank_attack',
	rankDefense: 'rank_defense',
	rankCraftSpeed: 'rank_craft_speed',
	nickname: 'nickname',
	activeSkills: 'active_skills',
	passiveSkills: 'passive_skills',
	workSuitability: 'work_suitability',
	friendshipPoint: 'friendship_point',
	sanity: 'sanity',
	stomach: 'stomach',
	hp: 'hp',
	gender: 'gender',
	isAwakened: 'is_awakened',
	isLucky: 'is_lucky'
};

function editPalPayload(changes: GamePalEditRequest): Record<string, unknown> {
	const payload: Record<string, unknown> = {};
	for (const [key, wireName] of Object.entries(EDIT_PAL_WIRE_NAMES)) {
		payload[wireName] = changes[key as keyof GamePalEditRequest] ?? null;
	}
	return payload;
}

export interface GamePalsJson {
	pals: GamePalJson[];
	page: number;
	pageCount: number;
	slotCount: number;
	slotBase: number;
	containerId: string | null;
	party: GamePalJson[];
	partyStatus: string;
	status: string;
}

export interface GameItemDynamicJson {
	characterId: string | null;
	durability: number | null;
	localId: string;
	maxDurability: number | null;
	passiveSkills: string[];
	remainingBullets: number | null;
	type: string;
}

export interface GameItemSlotJson {
	slotIndex: number;
	staticItemId: string;
	count: number | null;
	dynamicItem?: GameItemDynamicJson | null;
}

export interface GameInventoryContainerJson {
	containerId: string | null;
	type: string;
	slotNum: number | null;
	status: string;
	slots: GameItemSlotJson[];
	buildObjectId: string | null;
	baseId: string | null;
}

export interface GameInventoryJson {
	playerUid: string;
	status: string;
	containers: GameInventoryContainerJson[];
}

export interface GameGuildContainersJson {
	guildId: string;
	containers: GameInventoryContainerJson[];
	status: string;
}

export interface GameGuildMemberJson {
	uid: string | null;
	name: string | null;
	role: string | null;
	status: string | null;
	lastOnlineTicks: number | null;
}

export interface GameGuildResearchJson {
	researchId: string | null;
	workAmount: number | null;
	requiredWorkAmount: number | null;
}

export interface GameGuildLabJson {
	currentResearchId: string | null;
	research: GameGuildResearchJson[];
}

export interface GameGuildSummaryJson {
	id: string | null;
	name: string | null;
	adminUid: string | null;
	members: GameGuildMemberJson[] | null;
}

export interface GameGuildsJson {
	guilds: GameGuildSummaryJson[];
	status: string;
}

export interface GameGuildBaseJson {
	id: string | null;
	name: string | null;
	level: number | null;
	buildingNum?: number | null;
	containerId: string | null;
	palSlotNum: number | null;
}

export interface GameGuildDetailJson {
	id: string | null;
	name: string | null;
	groupName: string | null;
	baseCampLevel: number | null;
	baseCampIds: string[] | null;
	baseCampPointIds: string[] | null;
	memberUids: string[] | null;
	bases: GameGuildBaseJson[] | null;
	adminUid: string | null;
	members: GameGuildMemberJson[] | null;
	roleOptions: string[] | null;
	lab: GameGuildLabJson | null;
}

export interface GameBasePalsJson {
	baseId: string;
	containerId: string | null;
	slotNum: number | null;
	pals: GamePalJson[];
	status: string;
}

export interface GameGuildJson {
	guild: GameGuildDetailJson | null;
	status: string;
}

export type GameCapabilityOp =
	| 'pal.heal'
	| 'item.setSlot'
	| 'pal.remove'
	| 'pal.move'
	| 'pal.add'
	| 'pal.edit'
	| 'player.edit'
	| 'guild.edit'
	| 'guild.setRole';

export interface GameCapabilityJson {
	available: boolean;
	reason: string | null;
}

export interface GameCapabilitiesJson {
	version: number;
	ops: Partial<Record<GameCapabilityOp, GameCapabilityJson>>;
}

export interface GameCommandResultJson {
	commandId: string;
	op: string;
	applied: boolean;
	verified: boolean;
	retrySafe: boolean;
	data: unknown;
}

export interface HealTargetRequest {
	slot_index: number;
}

export interface GameErrorJson {
	code: string;
	message: string;
}

export interface GameHealResultEntry {
	slot_index: number;
	ok: boolean;
	result: GameCommandResultJson | null;
	error: GameErrorJson | null;
}

export interface GameHealPalsJson {
	results: GameHealResultEntry[];
}

export interface GameRefusalJson {
	error: string;
	code: string;
}

export class GameCommandError extends Error {
	code: string;
	constructor(message: string, code: string) {
		super(message);
		this.code = code;
	}
}

const UNKNOWN_OUTCOME_CODES = new Set(['bridge_offline', 'timeout']);

const WRITE_TIMEOUT_MS = 15_000;

export function isOutcomeUnknown(error: unknown): boolean {
	if (!(error instanceof GameCommandError)) return true;
	return UNKNOWN_OUTCOME_CODES.has(error.code);
}

function healOutcomeUnknown(results: GameHealResultEntry[]): boolean {
	return results.some((entry) => !entry.ok && UNKNOWN_OUTCOME_CODES.has(entry.error?.code ?? ''));
}

type GameReply<T> = T | GameRefusalJson;

function transportRefusal(error: unknown): GameRefusalJson {
	return { error: error instanceof Error ? error.message : String(error), code: 'bridge_offline' };
}

export class GameState {
	status = $state<GameStatusJson | null>(null);
	statusError = $state<GameRefusalJson | null>(null);
	capabilities = $state<GameCapabilitiesJson | null>(null);
	players = $state<GamePlayerJson[]>([]);
	playersError = $state<GameRefusalJson | null>(null);
	pals = $state<GamePalJson[]>([]);
	palsPage = $state(0);
	palsPageCount = $state(0);
	palsSlotCount = $state(0);
	palsSlotBase = $state(0);
	party = $state<GamePalJson[]>([]);
	partyReadable = $state(false);
	palsError = $state<GameRefusalJson | null>(null);
	inventory = $state<GameInventoryJson | null>(null);
	inventoryError = $state<GameRefusalJson | null>(null);
	guild = $state<GameGuildJson | null>(null);
	guildError = $state<GameRefusalJson | null>(null);
	guilds = $state<GameGuildsJson | null>(null);
	basePals = $state<GameBasePalsJson | null>(null);
	basePalsError = $state<GameRefusalJson | null>(null);
	guildContainers = $state<GameGuildContainersJson | null>(null);
	guildContainersError = $state<GameRefusalJson | null>(null);
	instances = $state<GameInstanceJson[]>([]);
	activeInstanceId = $state<string | null>(null);

	writeBusy = $state(false);

	#statusGuard = new RequestGuard();
	#capabilitiesGuard = new RequestGuard();
	#playersGuard = new RequestGuard();
	#palsGuard = new RequestGuard();
	#inventoryGuard = new RequestGuard();
	#guildGuard = new RequestGuard();
	#guildsGuard = new RequestGuard();
	#basePalsGuard = new RequestGuard();
	#guildContainersGuard = new RequestGuard();
	#instancesGuard = new RequestGuard();
	#healTracker = new CommandIdTracker();
	#setItemSlotTracker = new CommandIdTracker();
	#removePalTracker = new CommandIdTracker();
	#movePalTracker = new CommandIdTracker();
	#addPalTracker = new CommandIdTracker();
	#editPalTracker = new CommandIdTracker();
	#editPlayerTracker = new CommandIdTracker();
	#editGuildTracker = new CommandIdTracker();
	#setGuildRoleTracker = new CommandIdTracker();

	async refreshStatus(): Promise<void> {
		const ticket = this.#statusGuard.next();
		try {
			const response = await sendAndWait<GameReply<GameStatusJson>>(MessageType.GAME_STATUS);
			if (!this.#statusGuard.isCurrent(ticket)) return;
			if ('error' in response) {
				this.statusError = response;
				this.status = null;
			} else {
				this.status = response;
				this.statusError = null;
			}
		} catch (error) {
			console.error('game_status failed', error);
		}
	}

	async refreshCapabilities(): Promise<void> {
		const ticket = this.#capabilitiesGuard.next();
		try {
			const response = await sendAndWait<GameReply<GameCapabilitiesJson>>(
				MessageType.GAME_CAPABILITIES
			);
			if (!this.#capabilitiesGuard.isCurrent(ticket)) return;
			this.capabilities = 'error' in response ? null : response;
		} catch (error) {
			console.error('game_capabilities failed', error);
		}
	}

	async refreshPlayers(): Promise<void> {
		const ticket = this.#playersGuard.next();
		try {
			const response = await sendAndWait<GameReply<GamePlayersJson>>(MessageType.GAME_PLAYERS);
			if (!this.#playersGuard.isCurrent(ticket)) return;
			if ('error' in response) {
				this.playersError = response;
				this.players = [];
			} else {
				this.players = response.players;
				this.playersError = null;
			}
		} catch (error) {
			console.error('game_players failed', error);
		}
	}

	async loadPals(playerUid: string, page = 0): Promise<void> {
		const ticket = this.#palsGuard.next();
		try {
			const response = await sendAndWait<GameReply<GamePalsJson>>(MessageType.GAME_PALS, {
				player_uid: playerUid,
				page
			});
			if (!this.#palsGuard.isCurrent(ticket)) return;
			if ('error' in response) {
				this.palsError = response;
				this.pals = [];
				this.palsPage = 0;
				this.palsPageCount = 0;
				this.palsSlotCount = 0;
				this.palsSlotBase = 0;
				this.party = [];
				this.partyReadable = false;
			} else {
				this.pals = response.pals;
				this.palsPage = response.page;
				this.palsPageCount = response.pageCount;
				this.palsSlotCount = response.slotCount ?? 0;
				this.palsSlotBase = response.slotBase ?? 0;
				this.party = response.party ?? [];
				this.partyReadable = response.partyStatus === 'ok';
				this.palsError = null;
			}
		} catch (error) {
			console.error('game_pals failed', error);
			if (!this.#palsGuard.isCurrent(ticket)) return;
			this.palsError = transportRefusal(error);
			this.pals = [];
			this.palsPage = 0;
			this.palsPageCount = 0;
			this.palsSlotCount = 0;
			this.palsSlotBase = 0;
			this.party = [];
			this.partyReadable = false;
		}
	}

	async loadInventory(playerUid: string): Promise<void> {
		const ticket = this.#inventoryGuard.next();
		try {
			const response = await sendAndWait<GameReply<GameInventoryJson>>(MessageType.GAME_INVENTORY, {
				player_uid: playerUid
			});
			if (!this.#inventoryGuard.isCurrent(ticket)) return;
			if ('error' in response) {
				this.inventoryError = response;
				this.inventory = null;
			} else {
				this.inventory = response;
				this.inventoryError = null;
			}
		} catch (error) {
			console.error('game_inventory failed', error);
			if (!this.#inventoryGuard.isCurrent(ticket)) return;
			this.inventoryError = transportRefusal(error);
			this.inventory = null;
		}
	}

	async loadGuild(target: { guildId?: string; playerUid?: string }): Promise<void> {
		const ticket = this.#guildGuard.next();
		try {
			const payload = target.guildId
				? { guild_id: target.guildId }
				: { player_uid: target.playerUid };
			const response = await sendAndWait<GameReply<GameGuildJson>>(MessageType.GAME_GUILD, payload);
			if (!this.#guildGuard.isCurrent(ticket)) return;
			if ('error' in response) {
				this.guildError = response;
				this.guild = null;
			} else {
				this.guild = response;
				this.guildError = null;
			}
		} catch (error) {
			console.error('game_guild failed', error);
			if (!this.#guildGuard.isCurrent(ticket)) return;
			this.guildError = transportRefusal(error);
			this.guild = null;
		}
	}

	async loadGuilds(): Promise<void> {
		const ticket = this.#guildsGuard.next();
		try {
			const response = await sendAndWait<GameReply<GameGuildsJson>>(MessageType.GAME_GUILDS, {});
			if (!this.#guildsGuard.isCurrent(ticket)) return;
			this.guilds = 'error' in response ? null : response;
		} catch (error) {
			console.error('game_guilds failed', error);
			if (!this.#guildsGuard.isCurrent(ticket)) return;
			this.guilds = null;
		}
	}

	async loadBasePals(baseId: string): Promise<void> {
		const ticket = this.#basePalsGuard.next();
		try {
			const response = await sendAndWait<GameReply<GameBasePalsJson>>(MessageType.GAME_BASE_PALS, {
				base_id: baseId
			});
			if (!this.#basePalsGuard.isCurrent(ticket)) return;
			if ('error' in response) {
				this.basePalsError = response;
				this.basePals = null;
			} else {
				this.basePals = response;
				this.basePalsError = null;
			}
		} catch (error) {
			console.error('game_base_pals failed', error);
			if (!this.#basePalsGuard.isCurrent(ticket)) return;
			this.basePalsError = transportRefusal(error);
			this.basePals = null;
		}
	}

	async loadGuildContainers(guildId: string): Promise<void> {
		const ticket = this.#guildContainersGuard.next();
		try {
			const response = await sendAndWait<GameReply<GameGuildContainersJson>>(
				MessageType.GAME_GUILD_CONTAINERS,
				{ guild_id: guildId }
			);
			if (!this.#guildContainersGuard.isCurrent(ticket)) return;
			if ('error' in response) {
				this.guildContainersError = response;
				this.guildContainers = null;
			} else {
				this.guildContainers = response;
				this.guildContainersError = null;
			}
		} catch (error) {
			console.error('game_guild_containers failed', error);
			if (!this.#guildContainersGuard.isCurrent(ticket)) return;
			this.guildContainersError = transportRefusal(error);
			this.guildContainers = null;
		}
	}

	#adoptInstances(ticket: number, response: GameReply<GameInstancesJson>): void {
		if (!this.#instancesGuard.isCurrent(ticket)) return;
		if ('error' in response || !Array.isArray(response.instances)) return;
		this.instances = response.instances;
		this.activeInstanceId = response.activeId;
	}

	async refreshInstances(): Promise<void> {
		const ticket = this.#instancesGuard.next();
		try {
			const response = await sendAndWait<GameReply<GameInstancesJson>>(MessageType.GAME_INSTANCES);
			this.#adoptInstances(ticket, response);
		} catch (error) {
			console.error('game_instances failed', error);
		}
	}

	async addInstance(fields: GameInstanceFields): Promise<void> {
		const ticket = this.#instancesGuard.next();
		const response = await sendAndWait<GameReply<GameInstancesJson>>(
			MessageType.GAME_ADD_INSTANCE,
			{ ...fields }
		);
		if ('error' in response) throw new GameCommandError(response.error, response.code);
		this.#adoptInstances(ticket, response);
	}

	async updateInstance(id: string, fields: GameInstanceFields): Promise<void> {
		const ticket = this.#instancesGuard.next();
		const response = await sendAndWait<GameReply<GameInstancesJson>>(
			MessageType.GAME_UPDATE_INSTANCE,
			{ id, ...fields }
		);
		if ('error' in response) throw new GameCommandError(response.error, response.code);
		this.#adoptInstances(ticket, response);
	}

	async deleteInstance(id: string): Promise<void> {
		const ticket = this.#instancesGuard.next();
		const response = await sendAndWait<GameReply<GameInstancesJson>>(
			MessageType.GAME_DELETE_INSTANCE,
			{ id }
		);
		if ('error' in response) throw new GameCommandError(response.error, response.code);
		this.#adoptInstances(ticket, response);
	}

	async selectInstance(id: string): Promise<void> {
		const ticket = this.#instancesGuard.next();
		const response = await sendAndWait<GameReply<GameInstancesJson>>(
			MessageType.GAME_SELECT_INSTANCE,
			{ id }
		);
		if ('error' in response) throw new GameCommandError(response.error, response.code);
		this.#adoptInstances(ticket, response);
	}

	async setInstanceTarget(id: string, targetId: string | null): Promise<void> {
		const ticket = this.#instancesGuard.next();
		const response = await sendAndWait<GameReply<GameInstancesJson>>(
			MessageType.GAME_INSTANCE_SET_TARGET,
			{ id, targetId }
		);
		if ('error' in response) throw new GameCommandError(response.error, response.code);
		this.#adoptInstances(ticket, response);
	}

	async testInstance(fields: GameInstanceFields): Promise<GameTestInstanceJson> {
		try {
			const response = await sendAndWait<GameReply<GameTestInstanceJson>>(
				MessageType.GAME_TEST_INSTANCE,
				{ ...fields }
			);
			if ('error' in response) {
				return { ok: false, error: String(response.error) };
			}
			return response;
		} catch (error) {
			console.error('game_test_instance failed', error);
			return { ok: false, error: transportRefusal(error).error };
		}
	}

	async editGuild(guildId: string, baseCampLevel: number): Promise<GameCommandResultJson> {
		const key = editGuildActionKey(guildId, baseCampLevel);
		return this.#write(this.#editGuildTracker, key, async (commandId) => {
			const response = await sendAndWait<GameReply<GameCommandResultJson>>(
				MessageType.GAME_EDIT_GUILD,
				{ guild_id: guildId, base_camp_level: baseCampLevel, command_id: commandId }
			);
			if ('error' in response) throw new GameCommandError(response.error, response.code);
			this.#editGuildTracker.recordOutcome(key, false);
			return response;
		});
	}

	async setGuildRole(
		guildId: string,
		memberUid: string,
		role: string
	): Promise<GameCommandResultJson> {
		const key = setGuildRoleActionKey(guildId, memberUid, role);
		return this.#write(this.#setGuildRoleTracker, key, async (commandId) => {
			const response = await sendAndWait<GameReply<GameCommandResultJson>>(
				MessageType.GAME_SET_GUILD_ROLE,
				{ guild_id: guildId, member_uid: memberUid, role, command_id: commandId }
			);
			if ('error' in response) throw new GameCommandError(response.error, response.code);
			this.#setGuildRoleTracker.recordOutcome(key, false);
			return response;
		});
	}

	async #write<T>(
		tracker: CommandIdTracker,
		key: string,
		run: (commandId: string) => Promise<T>
	): Promise<T> {
		if (this.writeBusy) {
			throw new GameCommandError('another change is still in flight', 'write_in_flight');
		}
		this.writeBusy = true;
		const commandId = tracker.next(key);
		let expiry: ReturnType<typeof setTimeout> | undefined;
		try {
			return await Promise.race([
				run(commandId),
				new Promise<never>((_, reject) => {
					expiry = setTimeout(
						() => reject(new GameCommandError('the game did not answer in time', 'timeout')),
						WRITE_TIMEOUT_MS
					);
				})
			]);
		} catch (error) {
			tracker.recordOutcome(key, isOutcomeUnknown(error));
			throw error;
		} finally {
			if (expiry !== undefined) clearTimeout(expiry);
			this.writeBusy = false;
		}
	}

	async healPals(playerUid: string, targets: HealTargetRequest[]): Promise<GameHealPalsJson> {
		const key = healActionKey(playerUid, targets);
		return this.#write(this.#healTracker, key, async (commandId) => {
			const response = await sendAndWait<GameReply<GameHealPalsJson>>(MessageType.GAME_HEAL_PALS, {
				player_uid: playerUid,
				targets,
				command_id: commandId
			});
			if ('error' in response) throw new GameCommandError(response.error, response.code);
			this.#healTracker.recordOutcome(key, healOutcomeUnknown(response.results));
			return response;
		});
	}

	async setItemSlot(
		playerUid: string,
		containerId: string,
		slotIndex: number,
		staticItemId: string | null,
		count: number
	): Promise<GameCommandResultJson> {
		const amount = Math.max(0, Math.trunc(count));
		const item = staticItemId && amount > 0 ? staticItemId : null;
		const key = setItemSlotActionKey(playerUid, containerId, slotIndex);
		return this.#write(this.#setItemSlotTracker, key, async (commandId) => {
			const response = await sendAndWait<GameReply<GameCommandResultJson>>(
				MessageType.GAME_SET_ITEM_SLOT,
				{
					player_uid: playerUid,
					container_id: containerId,
					slot_index: slotIndex,
					static_item_id: item,
					count: item ? amount : 0,
					command_id: commandId
				}
			);
			if ('error' in response) throw new GameCommandError(response.error, response.code);
			this.#setItemSlotTracker.recordOutcome(key, false);
			return response;
		});
	}

	async removePal(playerUid: string, slotIndex: number): Promise<GameCommandResultJson> {
		const key = removePalActionKey(playerUid, slotIndex);
		return this.#write(this.#removePalTracker, key, async (commandId) => {
			const response = await sendAndWait<GameReply<GameCommandResultJson>>(
				MessageType.GAME_REMOVE_PAL,
				{ player_uid: playerUid, slot_index: slotIndex, command_id: commandId }
			);
			if ('error' in response) throw new GameCommandError(response.error, response.code);
			this.#removePalTracker.recordOutcome(key, false);
			return response;
		});
	}

	async movePal(
		playerUid: string,
		fromSlotIndex: number,
		toSlotIndex: number
	): Promise<GameCommandResultJson> {
		const key = movePalActionKey(playerUid, fromSlotIndex, toSlotIndex);
		return this.#write(this.#movePalTracker, key, async (commandId) => {
			const response = await sendAndWait<GameReply<GameCommandResultJson>>(
				MessageType.GAME_MOVE_PAL,
				{
					player_uid: playerUid,
					from_slot_index: fromSlotIndex,
					to_slot_index: toSlotIndex,
					command_id: commandId
				}
			);
			if ('error' in response) throw new GameCommandError(response.error, response.code);
			this.#movePalTracker.recordOutcome(key, false);
			return response;
		});
	}

	async addPal(
		playerUid: string,
		slotIndex: number | null,
		characterId: string,
		level?: number,
		gender?: string,
		target: PalAddTarget = {}
	): Promise<GameCommandResultJson> {
		const key = addPalActionKey(playerUid, slotIndex, target);
		return this.#write(this.#addPalTracker, key, async (commandId) => {
			const response = await sendAndWait<GameReply<GameCommandResultJson>>(
				MessageType.GAME_ADD_PAL,
				{
					player_uid: playerUid,
					slot_index: slotIndex,
					character_id: characterId,
					level: level === undefined ? null : Math.trunc(level),
					gender: gender ?? null,
					base_id: target.baseId ?? null,
					party: target.party ?? null,
					command_id: commandId
				}
			);
			if ('error' in response) throw new GameCommandError(response.error, response.code);
			this.#addPalTracker.recordOutcome(key, false);
			return response;
		});
	}

	async loadPalDetail(playerUid: string, address: PalAddress): Promise<GamePalDetailJson | null> {
		try {
			const response = await sendAndWait<GameReply<GamePalDetailJson>>(
				MessageType.GAME_PAL_DETAIL,
				{
					player_uid: playerUid,
					slot_index: address.slotIndex ?? null,
					instance_id: address.instanceId ?? null
				}
			);
			return 'error' in response ? null : response;
		} catch (error) {
			console.error('game_pal_detail failed', error);
			return null;
		}
	}

	async editPlayer(playerUid: string, level: number, exp: number): Promise<GameCommandResultJson> {
		const key = editPlayerActionKey(playerUid, level);
		return this.#write(this.#editPlayerTracker, key, async (commandId) => {
			const response = await sendAndWait<GameReply<GameCommandResultJson>>(
				MessageType.GAME_EDIT_PLAYER,
				{ player_uid: playerUid, level, exp, command_id: commandId }
			);
			if ('error' in response) throw new GameCommandError(response.error, response.code);
			this.#editPlayerTracker.recordOutcome(key, false);
			return response;
		});
	}

	async editPal(
		playerUid: string,
		address: PalAddress,
		changes: GamePalEditRequest
	): Promise<GameCommandResultJson> {
		const key = editPalActionKey(playerUid, address);
		return this.#write(this.#editPalTracker, key, async (commandId) => {
			const response = await sendAndWait<GameReply<GameCommandResultJson>>(
				MessageType.GAME_EDIT_PAL,
				{
					player_uid: playerUid,
					slot_index: address.slotIndex ?? null,
					instance_id: address.instanceId ?? null,
					...editPalPayload(changes),
					command_id: commandId
				}
			);
			if ('error' in response) throw new GameCommandError(response.error, response.code);
			this.#editPalTracker.recordOutcome(key, false);
			return response;
		});
	}
}

let gameStateInstance: GameState | undefined;

export function getGameState(): GameState {
	if (!gameStateInstance) {
		gameStateInstance = new GameState();
	}
	return gameStateInstance;
}
