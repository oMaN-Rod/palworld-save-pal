import { serverErrorText } from '$components/servers/serverErrors';
import * as m from '$i18n/messages';
import { getModsState, getServerState, getToastState } from '$states';
import type {
	ApplyResult,
	ContainerStats,
	ModError,
	Server,
	ServerApiResponse,
	ServerStatus
} from '$types';
import { MessageType } from '$types';
import type { WSMessageHandler } from '$ws/types';

/** `list_servers` and `get_server` entries say whether the server's mods are still moving. */
type ListedServer = Server & { relocation_pending?: boolean };

function syncRelocation(
	state: ReturnType<typeof getServerState>,
	{ relocation_pending, ...server }: ListedServer
): Server {
	if (relocation_pending === false) state.clearRelocation(server.id);
	else if (relocation_pending) state.markRelocationPending(server.id);
	return server;
}

function closedCleanly(apply: ApplyResult): boolean {
	return !apply.mid_apply && apply.error === undefined;
}

/** Without an apply result, a database failure came before anything moved. */
function modsMoved(apply: ApplyResult | null | undefined, error: ModError | undefined): boolean {
	if (!error) return false;
	if (apply) return closedCleanly(apply);
	return error.code !== 'db' && error.code !== 'not_found';
}

export const listServersHandler: WSMessageHandler = {
	type: MessageType.LIST_SERVERS,
	async handle(data: { servers: ListedServer[] }) {
		const state = getServerState();
		const servers = data.servers.map((entry) => syncRelocation(state, entry));
		state.servers = servers;
		state.loading = false;

		if (state.selectedServer) {
			const updated = servers.find((s) => s.id === state.selectedServer?.id);
			if (updated) {
				state.selectedServer = updated;
			}
		}
	}
};

export const getServerHandler: WSMessageHandler = {
	type: MessageType.GET_SERVER,
	async handle(data: ListedServer) {
		const state = getServerState();
		const server = syncRelocation(state, data);
		state.selectedServer = server;

		const idx = state.servers.findIndex((s) => s.id === server.id);
		if (idx >= 0) {
			state.servers[idx] = server;
		}
	}
};

export const createServerHandler: WSMessageHandler = {
	type: MessageType.CREATE_SERVER,
	async handle(data: Server & { warnings?: string[] }) {
		const state = getServerState();
		const toast = getToastState();
		state.creationProgress = '';
		state.servers = [...state.servers, data];
		state.selectedServer = data;
		toast.add(`Server "${data.name}" created successfully`, 'Success', 'success');
		for (const warning of data.warnings ?? []) {
			toast.add(warning, 'Notice', 'default');
		}
	}
};

export const importServerHandler: WSMessageHandler = {
	type: MessageType.IMPORT_SERVER,
	async handle(data: Server & { notifications?: string[] }) {
		const state = getServerState();
		const toast = getToastState();
		const idx = state.servers.findIndex((s) => s.id === data.id);
		if (idx >= 0) {
			state.servers[idx] = data;
		} else {
			state.servers = [...state.servers, data];
		}
		state.selectedServer = data;
		toast.add(`Server "${data.name}" imported successfully`, 'Success', 'success');
		for (const note of data.notifications ?? []) {
			toast.add(note, 'Notice', 'default');
		}
	}
};

export const serverCreationProgressHandler: WSMessageHandler = {
	type: MessageType.SERVER_CREATION_PROGRESS,
	async handle(data: { message: string }) {
		const state = getServerState();
		state.creationProgress = data.message;
	}
};

/** A refusal carrying only `server_id` has no server fields. */
type UpdateServerReply = (Server | { id?: undefined; server_id: number }) & {
	error?: ModError;
	relocation_pending?: boolean;
	apply?: ApplyResult | null;
};

/** Moving a server's mods rewrites its target's root and folders. */
function reloadModsTarget(serverId: number): void {
	const mods = getModsState();
	const targetId = `server-${serverId}`;
	mods.loadTargets();
	mods.plan(targetId);
	mods.rescan(targetId);
}

export const updateServerHandler: WSMessageHandler = {
	type: MessageType.UPDATE_SERVER,
	async handle(data: UpdateServerReply) {
		const state = getServerState();
		const toast = getToastState();
		state.saving = false;
		const serverId = data.id ?? ('server_id' in data ? data.server_id : undefined);
		if (serverId !== undefined) getModsState().clearTargetProgress(`server-${serverId}`);

		if (data.id !== undefined) {
			const { error, relocation_pending, apply, ...server } = data;
			state.storeServer(server);
			if (!relocation_pending) state.clearRelocation(data.id);
			else if ('apply' in data) state.setRelocation(data.id, apply, error, modsMoved(apply, error));
			else state.markRelocationPending(data.id);
			if ('apply' in data) reloadModsTarget(data.id);
		}

		if (data.error) {
			const saved = data.id !== undefined && data.error.code === 'apply_in_progress';
			toast.add(
				serverErrorText(data.error, 'update'),
				saved ? m.warning() : m.error(),
				saved ? 'warning' : 'error'
			);
		} else if (data.relocation_pending) {
			toast.add(m.servers_relocation_pending(), m.warning(), 'warning');
		} else if (data.id !== undefined) {
			toast.add(m.servers_updated({ name: data.name }), m.success(), 'success');
		}
	}
};

export const ensureGamedataLaunchArgHandler: WSMessageHandler = {
	type: MessageType.ENSURE_GAMEDATA_LAUNCH_ARG,
	async handle(data: Server & { error?: string | ModError }) {
		const { error, ...server } = data;
		if (typeof error === 'string') return;
		getServerState().storeServer(server);
	}
};

export const deleteServerHandler: WSMessageHandler = {
	type: MessageType.DELETE_SERVER,
	async handle(data: { server_id: number; error?: ModError }) {
		const state = getServerState();
		const toast = getToastState();
		if (data.error) {
			toast.add(serverErrorText(data.error, 'delete'), m.error(), 'error');
			return;
		}
		state.servers = state.servers.filter((s) => s.id !== data.server_id);
		if (state.selectedServer?.id === data.server_id) {
			state.selectedServer = null;
		}
		toast.add('Server deleted', 'Success', 'success');
	}
};

export const serverStatusUpdateHandler: WSMessageHandler = {
	type: MessageType.SERVER_STATUS_UPDATE,
	async handle(data: {
		server_id: number;
		status: ServerStatus | null;
		success: boolean;
		error?: ModError;
	}) {
		const state = getServerState();
		const toast = getToastState();
		state.finishStart(data.server_id);

		const error = data.error;
		const started = data.success && data.status?.running === true;
		const wasMoving = state.relocationPending[data.server_id] !== undefined;
		if (error && (error.code === 'relocation_pending' || 'apply' in error)) {
			const apply = error.apply as ApplyResult | null | undefined;
			const moved = error.code !== 'relocation_pending';
			const cause = moved ? error : (error.cause as ModError | undefined);
			state.setRelocation(data.server_id, apply, cause, moved);
			if (apply) reloadModsTarget(data.server_id);
		} else if ((error && error.code !== 'db') || started) {
			state.clearRelocation(data.server_id);
			if (wasMoving && started) reloadModsTarget(data.server_id);
		}

		const withStatus = (entry: Server): Server => ({
			...entry,
			status: data.status ?? undefined,
			...(started && entry.container_needs_recreate ? { container_needs_recreate: false } : {})
		});
		const idx = state.servers.findIndex((s) => s.id === data.server_id);
		if (idx >= 0) {
			state.servers[idx] = withStatus(state.servers[idx]);
		}
		if (state.selectedServer?.id === data.server_id) {
			state.selectedServer = withStatus(state.selectedServer);
		}

		getModsState().clearTargetProgress(`server-${data.server_id}`);

		if (data.success) {
			toast.add(started ? m.servers_started() : m.servers_stopped(), m.success(), 'success');
		} else if (error) {
			toast.add(serverErrorText(error, 'start'), m.error(), 'error');
		} else {
			toast.add(
				data.status?.running ? m.server_stop_failed() : m.server_start_failed(),
				m.error(),
				'error'
			);
		}
	}
};

export const serverApiResponseHandler: WSMessageHandler = {
	type: MessageType.SERVER_API_RESPONSE,
	async handle(data: ServerApiResponse) {
		const state = getServerState();
		state.apiResponse = data;
	}
};

export const detectWorkshopDirHandler: WSMessageHandler = {
	type: MessageType.DETECT_WORKSHOP_DIR,
	async handle(data: { workshop_dir: string }) {
		const state = getServerState();
		state.detectedWorkshopDir = data.workshop_dir;
	}
};

export const getServerStatsHandler: WSMessageHandler = {
	type: MessageType.GET_SERVER_STATS,
	async handle(data: { server_id: number; stats: ContainerStats | null }) {
		const state = getServerState();
		if (state.selectedServer?.id === data.server_id) {
			state.containerStats = data.stats;
		}
	}
};

export const serverHandlers = [
	listServersHandler,
	getServerHandler,
	createServerHandler,
	updateServerHandler,
	ensureGamedataLaunchArgHandler,
	deleteServerHandler,
	serverStatusUpdateHandler,
	serverApiResponseHandler,
	detectWorkshopDirHandler,
	getServerStatsHandler,
	serverCreationProgressHandler,
	importServerHandler
];
