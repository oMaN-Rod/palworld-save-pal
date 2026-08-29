import { send, sendAndWait } from '$lib/utils/websocketUtils';
import { MessageType, type PluginRunResult, type PluginSummary } from '$types';

export interface RunningCommand {
	pluginId: string;
	commandId: string;
	runId: string | null;
	startedAt: number;
}

export interface ExportedPluginFile {
	name: string;
	content: string;
}

export interface ExportedPluginDesktopResult {
	message: string;
	file_path: string;
}

// psp_plugin::sandbox::DEFAULT_WALL_CLOCK_MS -- the server-side limit every run is bounded by.
export const PLUGIN_WALL_CLOCK_LIMIT_SECONDS = 30;

// Past the server's own limit: clears a stuck "running" banner if the connection
// drops mid-run, since the shared transport exposes no `onclose` hook to key off.
const RUN_WATCHDOG_MS = (PLUGIN_WALL_CLOCK_LIMIT_SECONDS + 5) * 1000;

class Plugins {
	plugins: PluginSummary[] = $state([]);
	lastResult: PluginRunResult | null = $state(null);
	running: RunningCommand | null = $state(null);

	#runWatchdog: ReturnType<typeof setTimeout> | undefined;

	async list(): Promise<PluginSummary[]> {
		this.plugins = await sendAndWait<PluginSummary[]>(MessageType.LIST_PLUGINS);
		return this.plugins;
	}

	/** `send`, not `sendAndWait`: the reply comes back as `list_plugins`, not `set_plugin_enabled`, and the `LIST_PLUGINS` handler picks it up instead. */
	setEnabled(id: string, enabled: boolean): void {
		send(MessageType.SET_PLUGIN_ENABLED, { id, enabled });
	}

	async uninstall(id: string): Promise<void> {
		await sendAndWait(MessageType.UNINSTALL_PLUGIN, { id });
		await this.list();
	}

	async install(filename: string, content: string): Promise<PluginSummary> {
		const summary = await sendAndWait<PluginSummary>(MessageType.INSTALL_PLUGIN, {
			filename,
			content
		});
		await this.list();
		return summary;
	}

	async exportPlugin(id: string): Promise<ExportedPluginFile[] | ExportedPluginDesktopResult> {
		return sendAndWait<ExportedPluginFile[] | ExportedPluginDesktopResult>(
			MessageType.EXPORT_PLUGIN,
			{ id }
		);
	}

	async clonePlugin(sourceId: string, targetId: string, targetName: string): Promise<PluginSummary> {
		const summary = await sendAndWait<PluginSummary & { error?: string }>(MessageType.CLONE_PLUGIN, {
			source_id: sourceId,
			target_id: targetId,
			target_name: targetName
		});
		if (summary.error) throw new Error(summary.error);
		await this.list();
		return summary;
	}

	#startRun(pluginId: string, commandId: string): RunningCommand {
		this.lastResult = null;
		const marker: RunningCommand = { pluginId, commandId, runId: null, startedAt: Date.now() };
		this.running = marker;

		if (this.#runWatchdog) clearTimeout(this.#runWatchdog);
		this.#runWatchdog = setTimeout(() => {
			// Only clears state a dropped connection left behind; a real result already replaced `running` by now.
			if (this.running === marker) {
				this.running = null;
			}
		}, RUN_WATCHDOG_MS);

		return marker;
	}

	/** `send`, not `sendAndWait`: the result arrives as a `plugin_run_result` frame, routed back through `pluginHandlers`. */
	run(pluginId: string, commandId: string, args: Record<string, unknown>, dryRun: boolean): void {
		this.#startRun(pluginId, commandId);

		send(MessageType.RUN_PLUGIN_COMMAND, {
			plugin_id: pluginId,
			command_id: commandId,
			args,
			dry_run: dryRun
		});
	}

	/** `send`, not `sendAndWait`, for the same reason as `run`. */
	runDraft(
		pluginId: string,
		commandId: string,
		args: Record<string, unknown>,
		dryRun: boolean,
		sources: Record<string, string>,
		manifest: string | null
	): void {
		this.#startRun(pluginId, commandId);

		send(MessageType.RUN_PLUGIN_DRAFT, {
			plugin_id: pluginId,
			command_id: commandId,
			args,
			dry_run: dryRun,
			sources,
			manifest
		});
	}

	finishRun(result: PluginRunResult): void {
		if (this.#runWatchdog) {
			clearTimeout(this.#runWatchdog);
			this.#runWatchdog = undefined;
		}
		this.lastResult = result;
		this.running = null;
	}

	cancel(): void {
		if (this.running?.runId) {
			send(MessageType.CANCEL_PLUGIN_RUN, { run_id: this.running.runId });
		}
	}
}

export const pluginsData = new Plugins();
