import { describe, expect, it } from 'vitest';
import type { PluginCommand } from '$types';
import {
	MAX_TABLE_ROWS,
	buildRunRequest,
	defaultInputs,
	entityKindsUsed,
	normalizeView,
	parseArgRef,
	resolvePath,
	tablesFedBy,
	toList,
	toRows,
	toText
} from './pluginView';

const COMMAND_IDS = ['scan', 'fix'];

function widget(overrides: Record<string, unknown>) {
	return { type: 'text', ...overrides };
}

describe('normalizeView', () => {
	it('reads a section and its widgets', () => {
		const { sections, warnings } = normalizeView(
			[
				{
					title: 'Filters',
					columns: 2,
					widgets: [
						{ type: 'number_input', id: 'min_level', label: 'Minimum level' },
						{ type: 'button', label: 'Scan', command: 'scan', span: 'full' }
					]
				}
			],
			COMMAND_IDS
		);
		expect(warnings).toEqual([]);
		expect(sections).toHaveLength(1);
		expect(sections[0].title).toBe('Filters');
		expect(sections[0].columns).toBe(2);
		expect(sections[0].widgets[0]).toMatchObject({
			type: 'number_input',
			id: 'min_level',
			label: 'Minimum level'
		});
		expect(sections[0].widgets[1].span).toBe('full');
	});

	it('treats anything that is not an array of sections as no view at all', () => {
		for (const raw of [undefined, null, 'nope', 42, {}]) {
			expect(normalizeView(raw, COMMAND_IDS).sections).toEqual([]);
		}
	});

	it('skips an unknown widget type and says so', () => {
		const { sections, warnings } = normalizeView(
			[{ widgets: [widget({ type: 'sparkline', id: 'x' }), widget({ from: 'scan' })] }],
			COMMAND_IDS
		);
		expect(sections[0].widgets).toHaveLength(1);
		expect(warnings.join(' ')).toContain('sparkline');
	});

	it('skips an entity_select whose entity kind it does not know', () => {
		const { sections, warnings } = normalizeView(
			[{ widgets: [widget({ type: 'entity_select', id: 'who', entity: 'dragon' })] }],
			COMMAND_IDS
		);
		expect(sections[0].widgets).toEqual([]);
		expect(warnings.join(' ')).toContain('dragon');
	});

	it('skips a widget whose from names no command', () => {
		const { sections, warnings } = normalizeView(
			[{ widgets: [widget({ type: 'table', id: 'rows', from: 'nonesuch' })] }],
			COMMAND_IDS
		);
		expect(sections[0].widgets).toEqual([]);
		expect(warnings.join(' ')).toContain('nonesuch');
	});

	it('skips a button whose command names nothing, rather than drawing a dead button', () => {
		const { sections, warnings } = normalizeView(
			[{ widgets: [{ type: 'button', label: 'Go', command: 'nonesuch' }] }],
			COMMAND_IDS
		);
		expect(sections[0].widgets).toEqual([]);
		expect(warnings.join(' ')).toContain('nonesuch');
	});

	it('clamps a column count it cannot render to one, and says so', () => {
		for (const columns of [0, 4, -1, 2.5, 'two']) {
			const { sections, warnings } = normalizeView([{ columns, widgets: [] }], COMMAND_IDS);
			expect(sections[0].columns).toBe(1);
			expect(warnings.length).toBeGreaterThan(0);
		}
	});

	it('keeps a valid column count', () => {
		for (const columns of [1, 2, 3]) {
			expect(normalizeView([{ columns, widgets: [] }], COMMAND_IDS).sections[0].columns).toBe(
				columns
			);
		}
	});

	it('drops a span it does not understand instead of failing', () => {
		const { sections } = normalizeView(
			[{ widgets: [widget({ from: 'scan', span: 'half' })] }],
			COMMAND_IDS
		);
		expect(sections[0].widgets[0].span).toBeNull();
	});
});

describe('resolvePath', () => {
	const result = { pals: [{ name: 'a' }, { name: 'b' }], nested: { count: 3 }, zero: 0, no: false };

	it('returns the whole value for an empty path', () => {
		expect(resolvePath(result, null)).toBe(result);
		expect(resolvePath(result, '')).toBe(result);
	});

	it('walks object keys and array indices', () => {
		expect(resolvePath(result, 'nested.count')).toBe(3);
		expect(resolvePath(result, 'pals.1.name')).toBe('b');
	});

	it('returns undefined rather than throwing when the path finds nothing', () => {
		expect(resolvePath(result, 'nope')).toBeUndefined();
		expect(resolvePath(result, 'nested.nope.deeper')).toBeUndefined();
		expect(resolvePath(result, 'pals.9')).toBeUndefined();
		expect(resolvePath(undefined, 'anything')).toBeUndefined();
	});

	it('distinguishes a falsy value from a missing one', () => {
		expect(resolvePath(result, 'zero')).toBe(0);
		expect(resolvePath(result, 'no')).toBe(false);
	});

	it('never walks into the prototype chain', () => {
		expect(resolvePath({}, 'constructor')).toBeUndefined();
		expect(resolvePath({}, '__proto__')).toBeUndefined();
		expect(resolvePath({}, 'toString')).toBeUndefined();
	});
});

describe('toRows', () => {
	it('projects declared columns and reports the total', () => {
		const table = toRows([{ name: 'a', level: 3, extra: 'hidden' }], ['name', 'level']);
		expect(table.columns).toEqual(['name', 'level']);
		expect(table.rows).toEqual([{ name: 'a', level: '3' }]);
		expect(table.total).toBe(1);
	});

	it('derives columns from the rows when none are declared', () => {
		const table = toRows([{ b: 1, a: 2 }, { c: 3 }], []);
		expect(table.columns).toEqual(['b', 'a', 'c']);
	});

	it('caps the rows it renders but reports how many exist', () => {
		const source = Array.from({ length: MAX_TABLE_ROWS + 214 }, (_, i) => ({ id: String(i) }));
		const table = toRows(source, ['id']);
		expect(table.rows).toHaveLength(MAX_TABLE_ROWS);
		expect(table.total).toBe(MAX_TABLE_ROWS + 214);
	});

	it('takes a row id from the first identity field it carries', () => {
		expect(toRows([{ id: 'A' }], ['id']).ids).toEqual(['A']);
		expect(toRows([{ instance_id: 'B' }], []).ids).toEqual(['B']);
		expect(toRows([{ uid: 'C' }], []).ids).toEqual(['C']);
		expect(toRows([{ name: 'D' }], []).ids).toEqual(['0']);
	});

	it('renders an empty table for anything that is not a list of rows', () => {
		for (const value of [undefined, null, 'text', 42, { not: 'a list' }]) {
			expect(toRows(value, ['x'])).toMatchObject({ rows: [], total: 0 });
		}
	});
});

describe('toText and toList', () => {
	it('renders scalars as themselves and absent values as empty', () => {
		expect(toText('a')).toBe('a');
		expect(toText(3)).toBe('3');
		expect(toText(false)).toBe('false');
		expect(toText(null)).toBe('');
		expect(toText(undefined)).toBe('');
	});

	it('renders a structure as JSON rather than [object Object]', () => {
		expect(toText({ a: 1 })).toBe('{"a":1}');
	});

	it('caps a list the way a table is capped, and reports the total', () => {
		const source = Array.from({ length: MAX_TABLE_ROWS + 5 }, (_, i) => i);
		const list = toList(source);
		expect(list.items).toHaveLength(MAX_TABLE_ROWS);
		expect(list.total).toBe(MAX_TABLE_ROWS + 5);
		expect(toList('not a list')).toEqual({ items: [], total: 0 });
	});
});

describe('parseArgRef', () => {
	it('accepts exactly the two forms in the grammar', () => {
		expect(parseArgRef('rows.selection')).toEqual({ widget: 'rows', kind: 'selection' });
		expect(parseArgRef('min_level.value')).toEqual({ widget: 'min_level', kind: 'value' });
	});

	it('rejects everything else, so nothing becomes an expression language', () => {
		for (const reference of [
			'rows',
			'rows.total',
			'rows.selection.first',
			'rows.value()',
			'1 + 1',
			'rows..value',
			'.value'
		]) {
			expect(parseArgRef(reference)).toBeNull();
		}
	});
});

const SCAN: PluginCommand = {
	id: 'scan',
	title: 'Scan',
	description: null,
	destructive: false,
	params: [
		{
			id: 'min_level',
			type: 'int',
			label: 'Min',
			description: null,
			default: 1,
			min: null,
			max: null,
			options: [],
			entity: null
		}
	]
};

const FIX: PluginCommand = {
	id: 'fix',
	title: 'Fix',
	description: null,
	destructive: true,
	params: [
		{
			id: 'ids',
			type: 'multiselect',
			label: 'Ids',
			description: null,
			default: [],
			min: null,
			max: null,
			options: [],
			entity: null
		}
	]
};

describe('buildRunRequest', () => {
	const view = normalizeView(
		[
			{
				widgets: [
					{ type: 'number_input', id: 'min_level', label: 'Min' },
					{ type: 'table', id: 'rows', from: 'scan', selectable: true },
					{ type: 'button', label: 'Scan', command: 'scan' },
					{ type: 'button', label: 'Fix', command: 'fix', args: { ids: 'rows.selection' } }
				]
			}
		],
		COMMAND_IDS
	).sections;

	const scanButton = view[0].widgets[2];
	const fixButton = view[0].widgets[3];

	it('feeds a command the input widget whose id matches its param', () => {
		const request = buildRunRequest(scanButton, SCAN, {
			inputs: { min_level: 42 },
			selections: {}
		});
		expect(request).toEqual({ commandId: 'scan', args: { min_level: 42 }, dryRun: false });
	});

	it('omits a param no widget feeds, leaving the host to apply its default', () => {
		const request = buildRunRequest(scanButton, SCAN, { inputs: {}, selections: {} });
		expect(request.args).toEqual({});
	});

	it('pulls a value out of another widget through args', () => {
		const request = buildRunRequest(fixButton, FIX, {
			inputs: {},
			selections: { rows: ['a', 'b'] }
		});
		expect(request.args).toEqual({ ids: ['a', 'b'] });
	});

	it('sends an empty selection rather than nothing', () => {
		const request = buildRunRequest(fixButton, FIX, { inputs: {}, selections: {} });
		expect(request.args).toEqual({ ids: [] });
	});

	/// A destructive command previews first, and nothing a plugin
	/// can put in its view opts out.
	it('always previews a destructive command, whatever the widget asks for', () => {
		const smuggler = {
			...fixButton,
			args: { ids: 'rows.selection', dry_run: 'rows.value', destructive: 'rows.value' }
		};
		const request = buildRunRequest(smuggler, FIX, { inputs: {}, selections: { rows: ['x'] } });
		expect(request.dryRun).toBe(true);
		expect(request.args).not.toHaveProperty('dry_run');
		expect(request.args).not.toHaveProperty('destructive');
	});

	it('never previews a non-destructive command', () => {
		expect(buildRunRequest(scanButton, SCAN, { inputs: {}, selections: {} }).dryRun).toBe(false);
	});
});

describe('tablesFedBy', () => {
	it('names every selectable table a command refills', () => {
		const { sections } = normalizeView(
			[
				{
					widgets: [
						{ type: 'table', id: 'a', from: 'scan', selectable: true },
						{ type: 'table', id: 'b', from: 'scan' },
						{ type: 'table', id: 'c', from: 'fix', selectable: true }
					]
				}
			],
			COMMAND_IDS
		);
		expect(tablesFedBy(sections, 'scan')).toEqual(['a']);
		expect(tablesFedBy(sections, 'fix')).toEqual(['c']);
		expect(tablesFedBy(sections, 'nonesuch')).toEqual([]);
	});
});

describe('entityKindsUsed', () => {
	it('lists each kind once, so one request can fetch them all', () => {
		const { sections } = normalizeView(
			[
				{
					widgets: [
						{ type: 'entity_select', id: 'a', entity: 'player' },
						{ type: 'entity_select', id: 'b', entity: 'player' },
						{ type: 'entity_select', id: 'c', entity: 'guild' }
					]
				}
			],
			COMMAND_IDS
		);
		expect(entityKindsUsed(sections).sort()).toEqual(['guild', 'player']);
	});
});

describe('defaultInputs', () => {
	it('seeds each input widget from the param it feeds', () => {
		const { sections } = normalizeView(
			[{ widgets: [{ type: 'number_input', id: 'min_level', label: 'Min' }] }],
			COMMAND_IDS
		);
		expect(defaultInputs(sections, [SCAN, FIX])).toEqual({ min_level: 1 });
	});
});
