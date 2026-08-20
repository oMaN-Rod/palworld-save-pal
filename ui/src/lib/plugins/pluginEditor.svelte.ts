import { pluginsData } from '$lib/data/plugins.svelte';
import { sendAndWait } from '$lib/utils/websocketUtils';
import { MessageType } from '$types';
import type { ApiDefinition, PluginCapability } from './apiDefinition';
import { commandAgreement, type AgreementWarning } from './pluginLint';

export const MANIFEST_PATH = 'manifest.json';

export interface PluginSyntaxError {
	line: number | null;
	message: string;
}

export interface DraftCommand {
	id: string;
	destructive: boolean;
}

/** Snapshot of the draft a destructive preview ran; Apply re-runs this, not whatever the buffers hold by then. */
interface PendingDraft {
	commandId: string;
	args: Record<string, unknown>;
	sources: Record<string, string>;
	manifest: string | null;
}

interface GetPluginResponse {
	id: string;
	manifest: { entry: string; commands: { id: string }[] } & Record<string, unknown>;
	sources: Record<string, string>;
	enabled: boolean;
	bundled: boolean;
	granted_capabilities: PluginCapability[];
}

class PluginEditorStore {
	pluginId = $state<string | null>(null);
	bundled = $state(false);
	enabled = $state(true);
	granted = $state<PluginCapability[]>([]);
	files = $state<Record<string, string>>({});
	savedFiles = $state<Record<string, string>>({});
	activePath = $state(MANIFEST_PATH);
	definition = $state<ApiDefinition | null>(null);
	syntaxError = $state<PluginSyntaxError | null>(null);
	manifestError = $state<string | null>(null);
	loading = $state(false);
	saving = $state(false);
	pendingApply = $state<PendingDraft | null>(null);

	#entry = $state('main.lua');
	#generation = 0;

	get paths(): string[] {
		return Object.keys(this.files).sort((a, b) => {
			if (a === MANIFEST_PATH) return -1;
			if (b === MANIFEST_PATH) return 1;
			return a.localeCompare(b);
		});
	}

	get activeText(): string {
		return this.files[this.activePath] ?? '';
	}

	isDirty(path: string): boolean {
		return this.files[path] !== this.savedFiles[path];
	}

	get dirty(): boolean {
		return this.paths.some((path) => this.isDirty(path));
	}

	get commands(): DraftCommand[] {
		try {
			const parsed = JSON.parse(this.files[MANIFEST_PATH] ?? '');
			const commands = parsed?.commands;
			if (!Array.isArray(commands)) return [];
			return commands
				.filter((command) => typeof command?.id === 'string')
				.map((command) => ({
					id: command.id as string,
					destructive: command.destructive === true
				}));
		} catch {
			return [];
		}
	}

	get commandIds(): string[] {
		return this.commands.map((command) => command.id);
	}

	get warnings(): AgreementWarning[] {
		const entry = this.files[this.#entry];
		if (entry === undefined) return [];
		return commandAgreement(this.commandIds, entry);
	}

	async loadDefinition(): Promise<void> {
		if (this.definition) return;
		const generation = this.#generation;
		const definition = await sendAndWait<ApiDefinition>(MessageType.GET_API_DEFINITION);
		if (generation !== this.#generation) return;
		this.definition = definition;
	}

	async open(id: string): Promise<void> {
		const generation = ++this.#generation;
		this.loading = true;
		try {
			const response = await sendAndWait<GetPluginResponse>(MessageType.GET_PLUGIN, { id });
			if (generation !== this.#generation) return;
			const files: Record<string, string> = {
				[MANIFEST_PATH]: JSON.stringify(response.manifest, null, 2),
				...response.sources
			};
			this.pluginId = response.id;
			this.bundled = response.bundled;
			this.enabled = response.enabled;
			this.granted = response.granted_capabilities ?? [];
			this.files = files;
			this.savedFiles = { ...files };
			this.#entry = response.manifest.entry;
			this.activePath = files[this.#entry] === undefined ? MANIFEST_PATH : this.#entry;
			this.syntaxError = null;
			this.manifestError = null;
			this.pendingApply = null;
		} finally {
			if (generation === this.#generation) this.loading = false;
		}
	}

	async create(id: string, name: string): Promise<string> {
		const summary = await sendAndWait<{ id: string; error?: string | null }>(
			MessageType.CREATE_PLUGIN,
			{ id, name }
		);
		if (summary.error) throw new Error(summary.error);
		return summary.id;
	}

	setSource(path: string, text: string): void {
		this.files = { ...this.files, [path]: text };
	}

	selectPath(path: string): void {
		this.activePath = path;
		this.syntaxError = null;
		this.manifestError = null;
	}

	async checkActive(): Promise<void> {
		if (this.activePath === MANIFEST_PATH) {
			const response = await sendAndWait<{ error: string | null }>(
				MessageType.CHECK_PLUGIN_MANIFEST,
				{ id: this.pluginId, manifest: this.files[MANIFEST_PATH] ?? '' }
			);
			this.manifestError = response.error;
			return;
		}
		const response = await sendAndWait<{ error: PluginSyntaxError | null }>(
			MessageType.CHECK_PLUGIN_SYNTAX,
			{ source: this.activeText }
		);
		this.syntaxError = response.error;
	}

	async saveActive(): Promise<void> {
		if (!this.pluginId) return;
		this.saving = true;
		try {
			const path = this.activePath;
			const source = this.files[path];
			const response = await sendAndWait<{ error?: string | null }>(
				MessageType.SAVE_PLUGIN_SOURCE,
				{ id: this.pluginId, path, source }
			);
			if (response.error) throw new Error(response.error);
			this.savedFiles = { ...this.savedFiles, [path]: source };
		} finally {
			this.saving = false;
		}
	}

	isDestructive(commandId: string): boolean {
		return this.commands.some((command) => command.id === commandId && command.destructive);
	}

	runDraft(commandId: string, args: Record<string, unknown>, dryRun: boolean): void {
		if (!this.pluginId || this.bundled) return;
		const { [MANIFEST_PATH]: manifest, ...sources } = this.files;
		const draft: PendingDraft = { commandId, args, sources, manifest: manifest ?? null };
		const destructive = this.isDestructive(commandId);
		this.pendingApply = destructive ? draft : null;
		this.#send(draft, destructive || dryRun);
	}

	applyPending(): void {
		const draft = this.pendingApply;
		if (!draft) return;
		this.pendingApply = null;
		this.#send(draft, false);
	}

	cancelPending(): void {
		this.pendingApply = null;
		pluginsData.lastResult = null;
	}

	#send(draft: PendingDraft, dryRun: boolean): void {
		if (!this.pluginId) return;
		pluginsData.runDraft(
			this.pluginId,
			draft.commandId,
			draft.args,
			dryRun,
			draft.sources,
			draft.manifest
		);
	}

	reset(): void {
		this.#generation++;
		this.pluginId = null;
		this.bundled = false;
		this.enabled = true;
		this.granted = [];
		this.files = {};
		this.savedFiles = {};
		this.activePath = MANIFEST_PATH;
		this.definition = null;
		this.syntaxError = null;
		this.manifestError = null;
		this.loading = false;
		this.saving = false;
		this.pendingApply = null;
		this.#entry = 'main.lua';
	}
}

export const pluginEditor = new PluginEditorStore();
