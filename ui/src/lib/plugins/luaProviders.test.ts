import { describe, expect, it } from 'vitest';
import type { ApiDefinition } from './apiDefinition';
import {
	completionItems,
	hoverFor,
	ownerBeforeCursor,
	signatureFor,
	type ApiSnapshot
} from './luaProviders';

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
					params: [
						{ name: 'uid', type: { kind: 'string' }, optional: false },
						{ name: 'force', type: { kind: 'boolean' }, optional: true }
					],
					returns: { kind: 'boolean' },
					doc: 'Deletes a player.',
					capability: 'save.write'
				}
			],
			fields: []
		},
		{
			name: 'raw',
			capability: 'save.raw',
			functions: [
				{
					name: 'delete',
					params: [],
					returns: { kind: 'boolean' },
					doc: 'Deletes a node.',
					capability: null
				}
			],
			fields: []
		},
		{
			name: 'ctx',
			capability: null,
			functions: [],
			fields: [{ name: 'dry_run', type: { kind: 'boolean' }, doc: 'Whether this is a dry run.' }]
		}
	],
	handles: [
		{
			name: 'player',
			capability: 'save.read',
			fields: [{ name: 'uid', type: { kind: 'string' }, doc: "The player's UUID." }],
			methods: [
				{
					name: 'delete',
					params: [],
					returns: { kind: 'boolean' },
					doc: 'Deletes.',
					capability: 'save.write'
				}
			]
		}
	]
};

const readOnly: ApiSnapshot = { definition, granted: ['save.read'] };
const full: ApiSnapshot = { definition, granted: ['save.read', 'save.write', 'save.raw'] };

describe('ownerBeforeCursor', () => {
	it('names the table when a member is being typed', () => {
		expect(ownerBeforeCursor('  save.pl')).toBe('save');
	});

	it('names the table immediately after the dot', () => {
		expect(ownerBeforeCursor('  save.')).toBe('save');
	});

	it('is null at the start of a bare identifier', () => {
		expect(ownerBeforeCursor('  sav')).toBeNull();
	});

	it('is null on an empty line', () => {
		expect(ownerBeforeCursor('')).toBeNull();
	});

	it('takes the nearest table when a call precedes it', () => {
		expect(ownerBeforeCursor('local n = count(save.pl')).toBe('save');
	});

	it('is null when the dot belongs to a number', () => {
		expect(ownerBeforeCursor('local x = 1.')).toBeNull();
	});
});

describe('completionItems at the top level', () => {
	it('offers only the globals the grant allows', () => {
		expect(
			completionItems(readOnly, null)
				.map((c) => c.label)
				.sort()
		).toEqual(['ctx', 'save']);
	});

	it('offers every global under a full grant', () => {
		expect(
			completionItems(full, null)
				.map((c) => c.label)
				.sort()
		).toEqual(['ctx', 'raw', 'save']);
	});

	it('marks a global as a module', () => {
		expect(completionItems(readOnly, null).every((c) => c.kind === 'module')).toBe(true);
	});
});

describe('completionItems for a globals members', () => {
	it('offers the functions the grant allows', () => {
		expect(completionItems(readOnly, 'save').map((c) => c.label)).toEqual(['players']);
	});

	it('offers a function gated by a capability once it is granted', () => {
		expect(completionItems(full, 'save').map((c) => c.label)).toEqual(['players', 'delete_player']);
	});

	it('offers nothing for a global the grant hides', () => {
		expect(completionItems(readOnly, 'raw')).toEqual([]);
	});

	it('offers nothing for a name that is not a global', () => {
		expect(completionItems(full, 'nonsense')).toEqual([]);
	});

	it('offers a globals fields as fields', () => {
		const items = completionItems(readOnly, 'ctx');
		expect(items.map((c) => c.label)).toEqual(['dry_run']);
		expect(items[0].kind).toBe('field');
	});

	it('carries the signature as the detail and the doc as documentation', () => {
		const item = completionItems(full, 'save').find((c) => c.label === 'delete_player')!;
		expect(item.detail).toBe('delete_player(uid: string, force?: boolean): boolean');
		expect(item.documentation).toContain('Deletes a player.');
		expect(item.documentation).toContain('save.write');
	});

	it('inserts a function name without parentheses', () => {
		const item = completionItems(full, 'save').find((c) => c.label === 'players')!;
		expect(item.insertText).toBe('players');
	});
});

describe('hoverFor', () => {
	it('describes a global', () => {
		expect(hoverFor(readOnly, null, 'save')).toContain('save');
	});

	it('describes a globals function', () => {
		const hover = hoverFor(full, 'save', 'delete_player')!;
		expect(hover).toContain('delete_player(uid: string, force?: boolean): boolean');
		expect(hover).toContain('Deletes a player.');
	});

	it('describes a globals field', () => {
		expect(hoverFor(readOnly, 'ctx', 'dry_run')).toContain('Whether this is a dry run.');
	});

	it('is null for a member the grant hides', () => {
		expect(hoverFor(readOnly, 'save', 'delete_player')).toBeNull();
	});

	it('is null for an unknown name', () => {
		expect(hoverFor(full, 'save', 'nope')).toBeNull();
		expect(hoverFor(full, null, 'nope')).toBeNull();
	});
});

describe('signatureFor', () => {
	it('lists each parameter with its own label', () => {
		const signature = signatureFor(full, 'save', 'delete_player')!;
		expect(signature.label).toBe('delete_player(uid: string, force?: boolean): boolean');
		expect(signature.parameters.map((p) => p.label)).toEqual(['uid: string', 'force?: boolean']);
	});

	it('is null for a member the grant hides', () => {
		expect(signatureFor(readOnly, 'save', 'delete_player')).toBeNull();
	});

	it('is null for a field rather than a function', () => {
		expect(signatureFor(readOnly, 'ctx', 'dry_run')).toBeNull();
	});
});

describe('tier arbitration', () => {
	it('offers baseline completions when the full tier is not live', () => {
		expect(completionItems(readOnly, 'save', false).length).toBeGreaterThan(0);
	});

	it('offers no baseline completions when the full tier is live', () => {
		expect(completionItems(readOnly, 'save', true)).toEqual([]);
	});

	it('offers no baseline hover when the full tier is live', () => {
		expect(hoverFor(readOnly, null, 'save', true)).toBeNull();
	});

	it('still offers signature help when the full tier is live', () => {
		expect(signatureFor(readOnly, 'save', 'players', true)).not.toBeNull();
	});
});
