import type {
	ApplyResult,
	ContainerStats,
	CreateServerData,
	ImportServerData,
	ModError,
	Server,
	ServerApiResponse
} from '$types';
import { MessageType } from '$types';
import { send } from '$utils/websocketUtils';
import { untrack } from 'svelte';

function without<T>(record: Record<number, T>, key: number): Record<number, T> {
	return Object.fromEntries(
		Object.entries(record).filter(([entry]) => Number(entry) !== key)
	) as Record<number, T>;
}

function withKnownRecreate(incoming: Server, stored: Server | null | undefined): Server {
	if (
		incoming.container_needs_recreate !== undefined ||
		stored?.container_needs_recreate === undefined
	) {
		return incoming;
	}
	return { ...incoming, container_needs_recreate: stored.container_needs_recreate };
}

export class ServerState {
	servers = $state<Server[]>([]);
	selectedServer = $state<Server | null>(null);
	loading = $state(false);
	apiResponse = $state<ServerApiResponse | null>(null);
	containerStats = $state<ContainerStats | null>(null);
	saving = $state(false);
	creationProgress = $state('');
	detectedWorkshopDir = $state('');
	/**
	 * Cleared by that server's `server_status_update`, which can wait behind a relocation, or when
	 * the connection that would carry it goes away.
	 */
	starting = $state<Record<number, boolean>>({});
	/** The apply result of the unfinished move, or `true` when there is none to show. */
	relocationPending = $state<Record<number, ApplyResult | true>>({});
	relocationError = $state<Record<number, ModError>>({});
	/** The mods reached their new folders, but setting the server up afterwards failed. */
	relocationMoved = $state<Record<number, boolean>>({});

	#pollInterval: ReturnType<typeof setInterval> | null = null;
	#connected: boolean | undefined;
	#wasConnected = false;
	#transport: object | null | undefined;

	/**
	 * A request sent before a drop is never answered, and the remote transport rejects one sent
	 * during the gap, so a drop and a remote reconnect both release starts and saves.
	 */
	connectionChanged(connected: boolean, transport: 'socket' | 'remote' = 'socket'): void {
		untrack(() => {
			const dropped = this.#connected === true && !connected;
			const reconnected = this.#connected === false && connected && this.#wasConnected;
			this.#connected = connected;
			if (connected) this.#wasConnected = true;
			if (dropped || (reconnected && transport === 'remote')) this.#forgetInFlight();
		});
	}

	/** The first transport seen is the one in use; any other will never answer earlier requests. */
	transportChanged(transport: object | null): void {
		const changed = this.#transport !== undefined && transport !== this.#transport;
		this.#transport = transport;
		if (changed) this.#forgetInFlight();
	}

	#forgetInFlight(): void {
		this.starting = {};
		this.saving = false;
	}

	storeServer(incoming: Server): void {
		const idx = this.servers.findIndex((s) => s.id === incoming.id);
		if (idx >= 0) {
			this.servers[idx] = withKnownRecreate(incoming, this.servers[idx]);
		}
		if (this.selectedServer?.id === incoming.id) {
			this.selectedServer = withKnownRecreate(incoming, this.selectedServer);
		}
	}

	finishStart(serverId: number): void {
		this.starting = without(
			untrack(() => this.starting),
			serverId
		);
	}

	setRelocation(
		serverId: number,
		apply: ApplyResult | null | undefined,
		error?: ModError,
		moved = false
	): void {
		this.relocationPending = { ...this.relocationPending, [serverId]: apply ?? true };
		this.relocationError = error
			? { ...this.relocationError, [serverId]: error }
			: without(this.relocationError, serverId);
		this.relocationMoved = moved
			? { ...this.relocationMoved, [serverId]: true }
			: without(this.relocationMoved, serverId);
	}

	/** Keeps whatever detail is already known about the move. */
	markRelocationPending(serverId: number): void {
		if (!untrack(() => this.relocationPending[serverId])) this.setRelocation(serverId, null);
	}

	clearRelocation(serverId: number): void {
		this.relocationPending = without(this.relocationPending, serverId);
		this.relocationError = without(this.relocationError, serverId);
		this.relocationMoved = without(this.relocationMoved, serverId);
	}

	async loadServers(): Promise<void> {
		this.loading = true;
		send(MessageType.LIST_SERVERS);
	}

	async selectServer(serverId: number): Promise<void> {
		send(MessageType.GET_SERVER, { server_id: serverId });
	}

	async createServer(data: CreateServerData): Promise<void> {
		send(MessageType.CREATE_SERVER, data);
	}

	async importServer(data: ImportServerData): Promise<void> {
		send(MessageType.IMPORT_SERVER, data);
	}

	async updateServer(serverId: number, updates: Record<string, any>): Promise<void> {
		this.saving = true;
		send(MessageType.UPDATE_SERVER, { server_id: serverId, updates });
	}

	async deleteServer(serverId: number): Promise<void> {
		send(MessageType.DELETE_SERVER, { server_id: serverId });
	}

	async startServer(serverId: number): Promise<void> {
		if (untrack(() => this.starting[serverId])) return;
		this.starting = { ...untrack(() => this.starting), [serverId]: true };
		send(MessageType.START_SERVER, { server_id: serverId });
	}

	async stopServer(serverId: number): Promise<void> {
		send(MessageType.STOP_SERVER, { server_id: serverId });
	}

	async callApi(
		serverId: number,
		endpoint: string,
		method: string = 'GET',
		payload?: Record<string, any>
	): Promise<void> {
		send(MessageType.SERVER_API_CALL, {
			server_id: serverId,
			endpoint,
			method,
			payload
		});
	}

	async detectWorkshopDir(): Promise<void> {
		send(MessageType.DETECT_WORKSHOP_DIR);
	}

	async loadServerSave(serverId: number): Promise<void> {
		send(MessageType.LOAD_SERVER_SAVE, { server_id: serverId });
	}

	async loadStats(serverId: number): Promise<void> {
		send(MessageType.GET_SERVER_STATS, { server_id: serverId });
	}

	startPolling(intervalMs: number = 15000): void {
		this.stopPolling();
		this.#pollInterval = setInterval(() => {
			this.loadServers();
		}, intervalMs);
	}

	stopPolling(): void {
		if (this.#pollInterval) {
			clearInterval(this.#pollInterval);
			this.#pollInterval = null;
		}
	}
}

let serverStateInstance: ServerState | undefined;

export function getServerState(): ServerState {
	if (!serverStateInstance) {
		serverStateInstance = new ServerState();
	}
	return serverStateInstance;
}
