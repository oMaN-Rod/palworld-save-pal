import { pluginsData } from '$lib/data';
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

export const pluginHandlers = [pluginRunResultHandler, listPluginsHandler];
