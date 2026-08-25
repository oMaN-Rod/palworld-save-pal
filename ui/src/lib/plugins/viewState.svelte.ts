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
		return this.sections.length > 0;
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

	setValue(widgetId: string, value: unknown): void {
		this.inputs = { ...this.inputs, [widgetId]: value };
	}

	runtime(): ViewRuntimeState {
		return { inputs: this.inputs, selections: this.selections };
	}

	optionsFor(kind: EntityKind): PluginEntityOptions {
		return this.entities[kind] ?? NO_OPTIONS;
	}

	async loadEntities(): Promise<void> {
		const kinds = entityKindsUsed(this.sections);
		if (kinds.length === 0) return;
		try {
			const reply = await sendAndWait<{ entities?: Record<string, PluginEntityOptions> }>(
				MessageType.LIST_PLUGIN_ENTITIES,
				{ kinds }
			);
			this.entities = reply?.entities ?? {};
		} catch (error) {
			console.warn('[plugin view] entity options could not be loaded', error);
			this.entities = {};
		}
	}
}
