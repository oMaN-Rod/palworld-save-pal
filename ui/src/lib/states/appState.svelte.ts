import { send } from '$lib/utils/websocketUtils';
import type {
	AppSettings,
	GamepassSave,
	Guild,
	GuildSummary,
	ItemContainerSlot,
	PlayerSummary
} from '$types';
import { MessageType, type Pal, type Player, type SaveFile } from '$types';
import {
	addNewUpspal,
	processGuilds,
	processPlayers,
	saveState,
	saveUpspalChanges,
	writeSave
} from './saveOperations.svelte';

export class AppState {
	players: Record<string, Player> = $state({});
	guilds: Record<string, Guild> = $state({});
	selectedPlayerUid: string = $state('');
	selectedPlayer: Player | undefined = $state(undefined);
	selectedPal: Pal | undefined = $state(undefined);
	saveFile: SaveFile | undefined = $state(undefined);
	playerSaveFiles: SaveFile[] = $state([]);
	clipboardItem: ItemContainerSlot | null = $state(null);
	progressMessage: string = $state('');
	version: string = $state('');
	settings: AppSettings = $state({ language: 'en' });
	gamepassSaves: Record<string, GamepassSave> = $state({});
	autoSave: boolean = $state(false);
	gps: Record<number, Pal> = $state({});

	playerSummaries: Record<string, PlayerSummary> = $state({});
	guildSummaries: Record<string, GuildSummary> = $state({});
	loadingPlayer: boolean = $state(false);
	loadingGuild: boolean = $state(false);
	loadingGps: boolean = $state(false);
	gpsLoaded: boolean = $state(false);
	hasGpsAvailable: boolean = $state(false);
	bulkDetailPlayer: Player | undefined = $state(undefined);
	bulkDetailGuild: Guild | undefined = $state(undefined);
	/** Set while a guild-details request originated from the bulk panel is in flight, so the
	 *  response handler knows whether to populate bulkDetailGuild instead of other guild loads
	 *  (e.g. /edit/guild, worldmap) stomping on it. */
	bulkGuildRequestPending: boolean = $state(false);

	resetState() {
		this.players = {};
		this.guilds = {};
		this.selectedPlayerUid = '';
		this.selectedPlayer = undefined;
		this.selectedPal = undefined;
		this.saveFile = undefined;
		this.playerSaveFiles = [];
		this.gps = {};
		this.playerSummaries = {};
		this.guildSummaries = {};
		this.loadingPlayer = false;
		this.loadingGuild = false;
		this.loadingGps = false;
		this.gpsLoaded = false;
		this.hasGpsAvailable = false;
		this.bulkDetailPlayer = undefined;
		this.bulkDetailGuild = undefined;
		this.bulkGuildRequestPending = false;
	}

	async selectPlayerLazy(playerId: string, origin: string = 'edit') {
		if (this.players?.[playerId]) {
			this.selectedPlayer = this.players[playerId];
			this.selectedPlayerUid = playerId;
			return;
		}

		this.loadingPlayer = true;
		send(MessageType.REQUEST_PLAYER_DETAILS, { player_id: playerId, origin });
	}

	loadPlayerDetailsForBulk(playerId: string) {
		const cached = this.players?.[playerId];
		if (cached) {
			this.bulkDetailPlayer = cached;
			return;
		}
		this.loadingPlayer = true;
		send(MessageType.REQUEST_PLAYER_DETAILS, { player_id: playerId, origin: 'bulk' });
	}

	loadGuildDetailsForBulk(guildId: string) {
		const cached = this.guilds?.[guildId];
		if (cached) {
			this.bulkDetailGuild = cached;
			return;
		}
		this.loadingGuild = true;
		this.bulkGuildRequestPending = true;
		send(MessageType.REQUEST_GUILD_DETAILS, guildId);
	}

	async loadGuildLazy(guildId: string) {
		if (this.guilds?.[guildId]) {
			return this.guilds[guildId];
		}

		if (this.loadingGuild) {
			return;
		}

		this.loadingGuild = true;
		send(MessageType.REQUEST_GUILD_DETAILS, guildId);
	}

	async loadGpsLazy(): Promise<boolean> {
		if (this.gpsLoaded && Object.keys(this.gps ?? {}).length > 0) {
			return true;
		}

		if (!this.hasGpsAvailable) {
			return false;
		}

		if (this.loadingGps) {
			return false;
		}

		this.loadingGps = true;
		send(MessageType.REQUEST_GPS);
		return false;
	}

	isPlayerLoaded(playerId: string): boolean {
		return this.players ? playerId in this.players : false;
	}

	isGuildLoaded(guildId: string): boolean {
		return this.guilds ? guildId in this.guilds : false;
	}

	get playerSummariesArray(): PlayerSummary[] {
		return Object.values(this.playerSummaries ?? {});
	}

	get guildSummariesArray(): GuildSummary[] {
		return Object.values(this.guildSummaries ?? {});
	}

	initData() {}

	async addNewUpspal(pal: Pal) {
		return addNewUpspal(this, pal);
	}

	async saveUpspalChanges(pal: Pal) {
		return saveUpspalChanges(pal);
	}

	processPlayers() {
		return processPlayers(this);
	}

	processGuilds() {
		return processGuilds(this);
	}

	async saveState() {
		return saveState(this);
	}

	async writeSave() {
		return writeSave(this);
	}
}

const appState = new AppState();
export const getAppState = () => appState;