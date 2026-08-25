import type { PluginCommand } from '$types';

export const WIDGET_KINDS = [
	'entity_select',
	'text_input',
	'number_input',
	'toggle',
	'select',
	'multiselect',
	'table',
	'list',
	'text',
	'button'
] as const;

export type WidgetKind = (typeof WIDGET_KINDS)[number];

export const INPUT_KINDS: readonly WidgetKind[] = [
	'entity_select',
	'text_input',
	'number_input',
	'toggle',
	'select',
	'multiselect'
];

export const ENTITY_KINDS = ['pal', 'player', 'guild', 'base'] as const;
export type EntityKind = (typeof ENTITY_KINDS)[number];

export const MAX_TABLE_ROWS = 500;

export interface ViewWidget {
	type: WidgetKind;
	id: string | null;
	label: string | null;
	entity: EntityKind | null;
	from: string | null;
	path: string | null;
	command: string | null;
	columns: string[];
	selectable: boolean;
	span: 'full' | null;
	args: Record<string, string>;
	text: string | null;
}

export interface ViewSection {
	title: string | null;
	columns: 1 | 2 | 3;
	widgets: ViewWidget[];
}

export interface NormalizedView {
	sections: ViewSection[];
	warnings: string[];
}

export interface TableData {
	columns: string[];
	rows: Record<string, string>[];
	ids: string[];
	total: number;
}

export interface ListData {
	items: string[];
	total: number;
}

export interface ViewRuntimeState {
	inputs: Record<string, unknown>;
	selections: Record<string, string[]>;
}

export interface ViewRunRequest {
	commandId: string;
	args: Record<string, unknown>;
	dryRun: boolean;
}

function asRecord(value: unknown): Record<string, unknown> | null {
	if (value === null || typeof value !== 'object' || Array.isArray(value)) return null;
	return value as Record<string, unknown>;
}

function asString(value: unknown): string | null {
	return typeof value === 'string' ? value : null;
}

function asStringArray(value: unknown): string[] {
	return Array.isArray(value)
		? value.filter((item): item is string => typeof item === 'string')
		: [];
}

function asStringMap(value: unknown): Record<string, string> {
	const record = asRecord(value);
	if (!record) return {};
	const out: Record<string, string> = {};
	for (const [key, item] of Object.entries(record)) {
		if (typeof item === 'string') out[key] = item;
	}
	return out;
}

function normalizeWidget(
	raw: unknown,
	commandIds: readonly string[],
	warnings: string[]
): ViewWidget | null {
	const record = asRecord(raw);
	if (!record) {
		warnings.push('A widget that is not an object was skipped.');
		return null;
	}
	const kind = asString(record.type);
	if (!kind || !(WIDGET_KINDS as readonly string[]).includes(kind)) {
		warnings.push(`Unknown widget type ${JSON.stringify(kind)} was skipped.`);
		return null;
	}
	const type = kind as WidgetKind;

	const entityRaw = asString(record.entity);
	if (
		type === 'entity_select' &&
		(!entityRaw || !(ENTITY_KINDS as readonly string[]).includes(entityRaw))
	) {
		warnings.push(`Unknown entity ${JSON.stringify(entityRaw)} was skipped.`);
		return null;
	}

	const from = asString(record.from);
	if (from !== null && !commandIds.includes(from)) {
		warnings.push(`Widget reads from unknown command ${JSON.stringify(from)}; skipped.`);
		return null;
	}

	const command = asString(record.command);
	if (type === 'button' && (command === null || !commandIds.includes(command))) {
		warnings.push(`Button runs unknown command ${JSON.stringify(command)}; skipped.`);
		return null;
	}

	const span = asString(record.span);
	return {
		type,
		id: asString(record.id),
		label: asString(record.label),
		entity:
			entityRaw !== null && (ENTITY_KINDS as readonly string[]).includes(entityRaw)
				? (entityRaw as EntityKind)
				: null,
		from,
		path: asString(record.path),
		command,
		columns: asStringArray(record.columns),
		selectable: record.selectable === true,
		span: span === 'full' ? 'full' : null,
		args: asStringMap(record.args),
		text: asString(record.text)
	};
}

export function normalizeView(raw: unknown, commandIds: readonly string[]): NormalizedView {
	const warnings: string[] = [];
	if (!Array.isArray(raw)) return { sections: [], warnings };

	const sections: ViewSection[] = [];
	for (const entry of raw) {
		const record = asRecord(entry);
		if (!record) {
			warnings.push('A section that is not an object was skipped.');
			continue;
		}
		const declared = record.columns;
		let columns: 1 | 2 | 3 = 1;
		if (declared === 1 || declared === 2 || declared === 3) {
			columns = declared;
		} else if (declared !== undefined && declared !== null) {
			warnings.push(`Section columns ${JSON.stringify(declared)} is not 1, 2 or 3; using 1.`);
		}
		const widgets: ViewWidget[] = [];
		if (Array.isArray(record.widgets)) {
			for (const widget of record.widgets) {
				const normalized = normalizeWidget(widget, commandIds, warnings);
				if (normalized) widgets.push(normalized);
			}
		}
		sections.push({ title: asString(record.title), columns, widgets });
	}
	return { sections, warnings };
}

/// `hasOwnProperty` rather than `in`: a path is plugin-supplied, and `in`
/// would let `constructor` or `toString` resolve to something off the
/// prototype chain that nothing in a command's result ever put there.
export function resolvePath(value: unknown, path: string | null): unknown {
	if (!path) return value;
	let current: unknown = value;
	for (const segment of path.split('.')) {
		if (current === null || current === undefined) return undefined;
		if (Array.isArray(current)) {
			const index = Number(segment);
			if (!Number.isInteger(index) || index < 0 || index >= current.length) return undefined;
			current = current[index];
			continue;
		}
		if (typeof current !== 'object') return undefined;
		const record = current as Record<string, unknown>;
		if (!Object.prototype.hasOwnProperty.call(record, segment)) return undefined;
		current = record[segment];
	}
	return current;
}

export function toText(value: unknown): string {
	if (value === null || value === undefined) return '';
	if (typeof value === 'string') return value;
	if (typeof value === 'number' || typeof value === 'boolean') return String(value);
	try {
		return JSON.stringify(value) ?? '';
	} catch {
		return '';
	}
}

const ROW_ID_FIELDS = ['id', 'instance_id', 'uid'];

function rowId(record: Record<string, unknown>, index: number): string {
	for (const field of ROW_ID_FIELDS) {
		const value = record[field];
		if (typeof value === 'string' && value.length > 0) return value;
	}
	return String(index);
}

export function toRows(value: unknown, declared: readonly string[]): TableData {
	const source = Array.isArray(value) ? value : [];
	const rendered = source.slice(0, MAX_TABLE_ROWS);
	const records = rendered.map((entry) => asRecord(entry) ?? {});

	let columns: string[];
	if (declared.length > 0) {
		columns = [...declared];
	} else {
		const seen = new Set<string>();
		columns = [];
		for (const record of records) {
			for (const key of Object.keys(record)) {
				if (!seen.has(key)) {
					seen.add(key);
					columns.push(key);
				}
			}
		}
	}

	const rows: Record<string, string>[] = [];
	const ids: string[] = [];
	records.forEach((record, index) => {
		const row: Record<string, string> = {};
		for (const column of columns) row[column] = toText(record[column]);
		rows.push(row);
		ids.push(rowId(record, index));
	});

	return { columns, rows, ids, total: source.length };
}

export function toList(value: unknown): ListData {
	const source = Array.isArray(value) ? value : [];
	return { items: source.slice(0, MAX_TABLE_ROWS).map(toText), total: source.length };
}

const ARG_REFERENCE = /^([A-Za-z_][A-Za-z0-9_]*)\.(selection|value)$/;

export function parseArgRef(
	reference: string
): { widget: string; kind: 'selection' | 'value' } | null {
	const match = ARG_REFERENCE.exec(reference);
	if (!match) return null;
	return { widget: match[1], kind: match[2] as 'selection' | 'value' };
}

export function buildRunRequest(
	widget: ViewWidget,
	command: PluginCommand,
	state: ViewRuntimeState
): ViewRunRequest {
	const params = new Set(command.params.map((param) => param.id));
	const args: Record<string, unknown> = {};
	for (const param of command.params) {
		if (Object.prototype.hasOwnProperty.call(state.inputs, param.id)) {
			args[param.id] = state.inputs[param.id];
		}
	}
	for (const [key, reference] of Object.entries(widget.args)) {
		if (!params.has(key)) continue;
		const parsed = parseArgRef(reference);
		if (!parsed) continue;
		args[key] =
			parsed.kind === 'selection'
				? (state.selections[parsed.widget] ?? [])
				: (state.inputs[parsed.widget] ?? null);
	}
	// The host owns this, not the view: nothing a plugin declares can run a
	// destructive command past its preview.
	return { commandId: command.id, args, dryRun: command.destructive };
}

export function tablesFedBy(sections: readonly ViewSection[], commandId: string): string[] {
	const ids: string[] = [];
	for (const section of sections) {
		for (const widget of section.widgets) {
			if (
				widget.type === 'table' &&
				widget.selectable &&
				widget.id &&
				widget.from === commandId
			) {
				ids.push(widget.id);
			}
		}
	}
	return ids;
}

export function entityKindsUsed(sections: readonly ViewSection[]): EntityKind[] {
	const kinds = new Set<EntityKind>();
	for (const section of sections) {
		for (const widget of section.widgets) {
			if (widget.type === 'entity_select' && widget.entity) kinds.add(widget.entity);
		}
	}
	return [...kinds];
}

export function defaultInputs(
	sections: readonly ViewSection[],
	commands: readonly PluginCommand[]
): Record<string, unknown> {
	const params = new Map<string, PluginCommand['params'][number]>();
	for (const command of commands) {
		for (const param of command.params) {
			if (!params.has(param.id)) params.set(param.id, param);
		}
	}

	const inputs: Record<string, unknown> = {};
	for (const section of sections) {
		for (const widget of section.widgets) {
			if (!INPUT_KINDS.includes(widget.type) || !widget.id) continue;
			const param = params.get(widget.id);
			if (!param) continue;
			if (param.default !== null && param.default !== undefined) {
				inputs[widget.id] = param.default;
				continue;
			}
			switch (param.type) {
				case 'int':
				case 'float':
					inputs[widget.id] = param.min ?? 0;
					break;
				case 'bool':
					inputs[widget.id] = false;
					break;
				case 'enum':
					inputs[widget.id] = param.options[0] ?? '';
					break;
				case 'multiselect':
					inputs[widget.id] = [];
					break;
				default:
					inputs[widget.id] = '';
			}
		}
	}
	return inputs;
}
