import { describe, expect, it } from 'vitest';
import {
	effectiveCapability,
	fieldDoc,
	functionDoc,
	globalByName,
	handleByName,
	isGranted,
	signatureLabel,
	typeName,
	visibleGlobals,
	type ApiDefinition
} from './apiDefinition';

const definition: ApiDefinition = {
	globals: [
		{
			name: 'save',
			capability: 'save.read',
			functions: [
				{
					name: 'players',
					params: [],
					returns: { kind: 'iterator', value: 'player' },
					doc: 'Every player in the save.',
					capability: null
				},
				{
					name: 'delete_player',
					params: [{ name: 'uid', type: { kind: 'string' }, optional: false }],
					returns: { kind: 'boolean' },
					doc: 'Deletes a player.',
					capability: 'save.write'
				}
			],
			fields: []
		},
		{
			name: 'ctx',
			capability: null,
			functions: [],
			fields: [{ name: 'dry_run', type: { kind: 'boolean' }, doc: 'Whether this is a dry run.' }]
		},
		{
			name: 'raw',
			capability: 'save.raw',
			functions: [
				{ name: 'get', params: [], returns: { kind: 'any' }, doc: 'Reads a node.', capability: null }
			],
			fields: []
		}
	],
	handles: [
		{
			name: 'player',
			capability: 'save.read',
			fields: [{ name: 'uid', type: { kind: 'string' }, doc: "The player's UUID." }],
			methods: [
				{ name: 'delete', params: [], returns: { kind: 'boolean' }, doc: 'Deletes.', capability: 'save.write' }
			]
		}
	]
};

describe('typeName', () => {
	it('renders every scalar kind as its Lua name', () => {
		expect(typeName({ kind: 'nil' })).toBe('nil');
		expect(typeName({ kind: 'boolean' })).toBe('boolean');
		expect(typeName({ kind: 'integer' })).toBe('integer');
		expect(typeName({ kind: 'number' })).toBe('number');
		expect(typeName({ kind: 'string' })).toBe('string');
		expect(typeName({ kind: 'table' })).toBe('table');
		expect(typeName({ kind: 'any' })).toBe('any');
	});

	it('renders a handle as its type name', () => {
		expect(typeName({ kind: 'handle', value: 'pal' })).toBe('pal');
	});

	it('renders an iterator the way the generated meta file does', () => {
		expect(typeName({ kind: 'iterator', value: 'pal' })).toBe('fun(): pal|nil');
	});

	it('renders a union as its members joined by a pipe', () => {
		expect(typeName({ kind: 'union', value: [{ kind: 'string' }, { kind: 'nil' }] })).toBe(
			'string|nil'
		);
	});

	it('renders a nested union without losing structure', () => {
		expect(
			typeName({
				kind: 'union',
				value: [{ kind: 'handle', value: 'pal' }, { kind: 'iterator', value: 'player' }]
			})
		).toBe('pal|fun(): player|nil');
	});
});

describe('capability gating', () => {
	it('inherits the owner capability when the member declares none', () => {
		expect(effectiveCapability(null, 'save.read')).toBe('save.read');
	});

	it('prefers the members own capability over the owners', () => {
		expect(effectiveCapability('save.write', 'save.read')).toBe('save.write');
	});

	it('treats an ungated member on an ungated owner as always visible', () => {
		expect(effectiveCapability(null, null)).toBeNull();
		expect(isGranted(null, [])).toBe(true);
	});

	it('grants only what the list contains', () => {
		expect(isGranted('save.read', ['save.read'])).toBe(true);
		expect(isGranted('save.write', ['save.read'])).toBe(false);
	});
});

describe('visibleGlobals', () => {
	it('drops a global whose capability is not granted', () => {
		const names = visibleGlobals(definition, ['save.read']).map((g) => g.name);
		expect(names).toContain('save');
		expect(names).toContain('ctx');
		expect(names).not.toContain('raw');
	});

	it('keeps an ungated global with no capabilities granted at all', () => {
		expect(visibleGlobals(definition, []).map((g) => g.name)).toEqual(['ctx']);
	});

	it('drops a function whose own capability is not granted, keeping its siblings', () => {
		const save = visibleGlobals(definition, ['save.read']).find((g) => g.name === 'save');
		expect(save?.functions.map((f) => f.name)).toEqual(['players']);
	});

	it('keeps a function whose own capability is granted', () => {
		const save = visibleGlobals(definition, ['save.read', 'save.write']).find(
			(g) => g.name === 'save'
		);
		expect(save?.functions.map((f) => f.name)).toEqual(['players', 'delete_player']);
	});

	it('does not mutate the definition it filters', () => {
		visibleGlobals(definition, []);
		expect(definition.globals).toHaveLength(3);
		expect(globalByName(definition, 'save')?.functions).toHaveLength(2);
	});
});

describe('lookups', () => {
	it('finds a global by name', () => {
		expect(globalByName(definition, 'save')?.name).toBe('save');
		expect(globalByName(definition, 'nope')).toBeUndefined();
	});

	it('finds a handle by its lowercase name', () => {
		expect(handleByName(definition, 'player')?.name).toBe('player');
		expect(handleByName(definition, 'Player')).toBeUndefined();
	});
});

describe('rendering', () => {
	it('labels a signature with parameter names and types', () => {
		const fn = globalByName(definition, 'save')!.functions[1];
		expect(signatureLabel(fn)).toBe('delete_player(uid: string): boolean');
	});

	it('marks an optional parameter', () => {
		expect(
			signatureLabel({
				name: 'find',
				params: [{ name: 'needle', type: { kind: 'string' }, optional: true }],
				returns: { kind: 'nil' },
				doc: '',
				capability: null
			})
		).toBe('find(needle?: string): nil');
	});

	it('labels a no-parameter function', () => {
		const fn = globalByName(definition, 'save')!.functions[0];
		expect(signatureLabel(fn)).toBe('players(): fun(): player|nil');
	});

	it('names the effective capability in a function doc', () => {
		const fn = globalByName(definition, 'save')!.functions[1];
		expect(functionDoc(fn, 'save.read')).toContain('save.write');
		expect(functionDoc(fn, 'save.read')).toContain('Deletes a player.');
	});

	it('names the inherited capability when the function declares none', () => {
		const fn = globalByName(definition, 'save')!.functions[0];
		expect(functionDoc(fn, 'save.read')).toContain('save.read');
	});

	it('names no capability for an ungated function', () => {
		const doc = functionDoc(
			{ name: 'f', params: [], returns: { kind: 'nil' }, doc: 'Does nothing.', capability: null },
			null
		);
		expect(doc).toBe('Does nothing.');
	});

	it('renders a field doc with its type', () => {
		const field = globalByName(definition, 'ctx')!.fields[0];
		expect(fieldDoc(field)).toContain('boolean');
		expect(fieldDoc(field)).toContain('Whether this is a dry run.');
	});
});
