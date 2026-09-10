import { pluginsData } from '$lib/data';
import { lspClient, type LspRequestReply } from '$lib/plugins/lspClient';
import { MessageType, type PluginRunResult, type PluginSummary } from '$types';
import type { WSMessageHandler } from '../types';

export const pluginRunResultHandler: WSMessageHandler = {
	type: MessageType.PLUGIN_RUN_RESULT,
	async handle(data: PluginRunResult) {
		pluginsData.finishRun(data);
	}
};

/**
 * `handle_set_plugin_enabled` also replies under `list_plugins` (see `setEnabled`
 * in plugins.svelte.ts). A pending `sendAndWait(LIST_PLUGINS)` intercepts that
 * response before dispatch, so this only fires for frames nothing is already
 * awaiting -- the two never double-fire.
 */
export const listPluginsHandler: WSMessageHandler = {
	type: MessageType.LIST_PLUGINS,
	async handle(data: PluginSummary[]) {
		pluginsData.plugins = data;
	}
};

export const lspNotificationHandler: WSMessageHandler = {
	type: MessageType.LSP_NOTIFICATION,
	async handle(data: { plugin_id?: string; frame?: unknown; error?: string }) {
		if (data.error) {
			console.error(`lsp notification failed: ${data.error}`);
			return;
		}
		if (!data.frame) return;
		if (data.plugin_id !== undefined && data.plugin_id !== lspClient.pluginId) return;
		lspClient.handleFrame(data.frame);
	}
};

/**
 * `lsp_request` answers are correlated by the `request_id` the client put on
 * the request, not by message type, so several may be in flight at once.
 */
export const lspRequestHandler: WSMessageHandler = {
	type: MessageType.LSP_REQUEST,
	async handle(data: LspRequestReply) {
		lspClient.handleRequestReply(data);
	}
};

export const pluginHandlers = [
	pluginRunResultHandler,
	listPluginsHandler,
	lspNotificationHandler,
	lspRequestHandler
];
