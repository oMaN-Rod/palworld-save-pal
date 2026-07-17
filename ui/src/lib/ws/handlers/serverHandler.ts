import { getServerState, getToastState } from '$states';
import type { Server, ServerMod, ServerApiResponse, ServerStatus, ContainerStats } from '$types';
import { MessageType } from '$types';
import type { WSMessageHandler } from '$ws/types';

export const listServersHandler: WSMessageHandler = {
	type: MessageType.LIST_SERVERS,
	async handle(data: { servers: Server[] }) {
		const state = getServerState();
		state.servers = data.servers;
		state.loading = false;

		// Update selected server if it exists in the new list
		if (state.selectedServer) {
			const updated = data.servers.find((s) => s.id === state.selectedServer?.id);
			if (updated) {
				state.selectedServer = updated;
			}
		}
	}
};

export const getServerHandler: WSMessageHandler = {
	type: MessageType.GET_SERVER,
	async handle(data: Server) {
		const state = getServerState();
		state.selectedServer = data;

		// Also update in the servers list
		const idx = state.servers.findIndex((s) => s.id === data.id);
		if (idx >= 0) {
			state.servers[idx] = data;
		}
	}
};

export const createServerHandler: WSMessageHandler = {
	type: MessageType.CREATE_SERVER,
	async handle(data: Server) {
		const state = getServerState();
		const toast = getToastState();
		state.creationProgress = '';
		state.servers = [...state.servers, data];
		state.selectedServer = data;
		toast.add(`Server "${data.name}" created successfully`, 'Success', 'success');
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

export const updateServerHandler: WSMessageHandler = {
	type: MessageType.UPDATE_SERVER,
	async handle(data: Server) {
		const state = getServerState();
		const toast = getToastState();
		const idx = state.servers.findIndex((s) => s.id === data.id);
		if (idx >= 0) {
			state.servers[idx] = data;
		}
		if (state.selectedServer?.id === data.id) {
			state.selectedServer = data;
		}
		state.saving = false;
		toast.add(`Server "${data.name}" updated`, 'Success', 'success');
	}
};

export const deleteServerHandler: WSMessageHandler = {
	type: MessageType.DELETE_SERVER,
	async handle(data: { server_id: number }) {
		const state = getServerState();
		const toast = getToastState();
		state.servers = state.servers.filter((s) => s.id !== data.server_id);
		if (state.selectedServer?.id === data.server_id) {
			state.selectedServer = null;
		}
		toast.add('Server deleted', 'Success', 'success');
	}
};

export const serverStatusUpdateHandler: WSMessageHandler = {
	type: MessageType.SERVER_STATUS_UPDATE,
	async handle(data: { server_id: number; status: ServerStatus; success: boolean }) {
		const state = getServerState();
		const toast = getToastState();

		const idx = state.servers.findIndex((s) => s.id === data.server_id);
		if (idx >= 0) {
			state.servers[idx] = { ...state.servers[idx], status: data.status };
		}
		if (state.selectedServer?.id === data.server_id) {
			state.selectedServer = { ...state.selectedServer, status: data.status };
		}

		if (data.success) {
			const action = data.status.running ? 'started' : 'stopped';
			toast.add(`Server ${action}`, 'Success', 'success');
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

export const listServerModsHandler: WSMessageHandler = {
	type: MessageType.LIST_SERVER_MODS,
	async handle(data: { server_id: number; mods: ServerMod[] }) {
		const state = getServerState();
		state.mods = data.mods;
	}
};

export const toggleServerModHandler: WSMessageHandler = {
	type: MessageType.TOGGLE_SERVER_MOD,
	async handle(data: { server_id: number; mod_name: string; enabled: boolean }) {
		const state = getServerState();
		const toast = getToastState();
		const mod = state.mods.find((m) => m.mod_name === data.mod_name);
		if (mod) {
			mod.enabled = data.enabled;
			state.mods = [...state.mods];
		}
		toast.add(
			`${data.mod_name} ${data.enabled ? 'enabled' : 'disabled'}`,
			'Success',
			'success'
		);
	}
};

export const installServerModHandler: WSMessageHandler = {
	type: MessageType.INSTALL_SERVER_MOD,
	async handle(data: { server_id: number; mod_name: string; success: boolean }) {
		const state = getServerState();
		const toast = getToastState();
		if (data.success) {
			toast.add(`Mod "${data.mod_name}" installed`, 'Success', 'success');
			// Refresh mods list
			state.loadMods(data.server_id);
		} else {
			toast.add(`Failed to install "${data.mod_name}"`, 'Error', 'error');
		}
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
	deleteServerHandler,
	serverStatusUpdateHandler,
	serverApiResponseHandler,
	listServerModsHandler,
	toggleServerModHandler,
	installServerModHandler,
	detectWorkshopDirHandler,
	getServerStatsHandler,
	serverCreationProgressHandler,
	importServerHandler
];
