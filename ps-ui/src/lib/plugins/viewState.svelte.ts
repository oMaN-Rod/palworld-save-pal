import { sendAndWait } from '$lib/utils/websocketUtils';
import { MessageType, type PluginCommand, type PluginEntityOptions } from '$types';
import {
	defaultInputs,
	entityKindsUsed,
	normalizeView,
	tablesFedBy,
	type EntityKind,
	type ViewRuntimeState,
	type ViewSection
} from './pluginView';

const NO_OPTIONS: PluginEntityOptions = { options: [], total: 0 };

export class PluginViewState {
	sections: ViewSection[] = $state([]);
	warnings: string[] = $state([]);
	inputs: Record<string, unknown> = $state({});
	selections: Record<string, string[]> = $state({});
	results: Record<string, unknown> = $state({});
	entities: Record<string, PluginEntityOptions> = $state({});

	#loading: Promise<void> | null = null;

	constructor(rawUi: unknown, commands: readonly PluginCommand[]) {
		const { sections, warnings } = normalizeView(
			rawUi,
			commands.map((command) => command.id)
		);
		this.sections = sections;
		this.warnings = warnings;
		this.inputs = defaultInputs(sections, commands);
		for (const warning of warnings) console.warn(`[plugin view] ${warning}`);
	}

	get hasView(): boolean {
		return this.sections.some((section) => section.widgets.length > 0);
	}

	recordResult(commandId: string, result: unknown): void {
		this.results = { ...this.results, [commandId]: result };
		const cleared = tablesFedBy(this.sections, commandId);
		if (cleared.length === 0) return;
		const selections = { ...this.selections };
		for (const id of cleared) selections[id] = [];
		this.selections = selections;
	}

	setSelection(widgetId: string, rowIds: string[]): void {
		this.selections = { ...this.selections, [widgetId]: [...rowIds] };
	}

	toggleRow(widgetId: string, rowId: string): void {
		const current = this.selections[widgetId] ?? [];
		this.setSelection(
			widgetId,
			current.includes(rowId) ? current.filter((id) => id !== rowId) : [...current, rowId]
		);
	}

	valueFor(widgetId: string): unknown {
		return this.inputs[widgetId];
	}

	/// Returns early on an unchanged value rather than replacing `inputs`
	/// anyway. A widget that re-emits what it was given would otherwise
	/// re-render itself, re-mint the change handler it passed down, and fire
	/// that handler again -- a cycle Svelte only stops by aborting the update.
	setValue(widgetId: string, value: unknown): void {
		if (
			Object.prototype.hasOwnProperty.call(this.inputs, widgetId) &&
			this.inputs[widgetId] === value
		) {
			return;
		}
		this.inputs = { ...this.inputs, [widgetId]: value };
	}

	runtime(): ViewRuntimeState {
		return { inputs: this.inputs, selections: this.selections };
	}

	optionsFor(kind: EntityKind): PluginEntityOptions {
		return this.entities[kind] ?? NO_OPTIONS;
	}

	/// Callers share one in-flight request. `sendAndWait` correlates replies by
	/// message type through a single pending slot, so a second request of the
	/// same type overwrites the first's resolver: one caller would wait forever
	/// and the stray reply would reach no one.
	loadEntities(): Promise<void> {
		const kinds = entityKindsUsed(this.sections);
		if (kinds.length === 0) return Promise.resolve();
		if (this.#loading) return this.#loading;

		const request = (async () => {
			try {
				const reply = await sendAndWait<{ entities?: Record<string, PluginEntityOptions> }>(
					MessageType.LIST_PLUGIN_ENTITIES,
					{ kinds }
				);
				this.entities = reply?.entities ?? {};
			} catch (error) {
				console.warn('[plugin view] entity options could not be loaded', error);
				this.entities = {};
			} finally {
				this.#loading = null;
			}
		})();
		this.#loading = request;
		return request;
	}
}
