import { describe, expect, it } from 'vitest';
import { commandAgreement, globalFunctionNames } from './pluginLint';

describe('globalFunctionNames', () => {
	it('finds a plain global function declaration', () => {
		expect(globalFunctionNames('function run()\nend\n')).toEqual(['run']);
	});

	it('finds a function assigned to a global name', () => {
		expect(globalFunctionNames('run = function()\nend\n')).toEqual(['run']);
	});

	it('tolerates whitespace around the declaration', () => {
		expect(globalFunctionNames('   function   run  ()\nend\n')).toEqual(['run']);
	});

	it('finds several declarations in one source', () => {
		expect(globalFunctionNames('function a()\nend\nfunction b()\nend\n')).toEqual(['a', 'b']);
	});

	it('ignores a local function', () => {
		expect(globalFunctionNames('local function helper()\nend\n')).toEqual([]);
	});

	it('ignores a function assigned to a local', () => {
		expect(globalFunctionNames('local helper = function()\nend\n')).toEqual([]);
	});

	it('ignores a method on a table', () => {
		expect(globalFunctionNames('function M.run()\nend\n')).toEqual([]);
		expect(globalFunctionNames('function M:run()\nend\n')).toEqual([]);
	});

	it('ignores a whole-line comment', () => {
		expect(globalFunctionNames('-- function run()\nfunction real()\nend\n')).toEqual(['real']);
	});

	it('reports each name once even when redeclared', () => {
		expect(globalFunctionNames('function run()\nend\nfunction run()\nend\n')).toEqual(['run']);
	});

	it('returns an empty list for an empty source', () => {
		expect(globalFunctionNames('')).toEqual([]);
	});
});

describe('commandAgreement', () => {
	it('is silent when every command has a function and vice versa', () => {
		expect(commandAgreement(['run'], 'function run()\nend\n')).toEqual([]);
	});

	it('warns about a command with no function', () => {
		const warnings = commandAgreement(['run', 'clean'], 'function run()\nend\n');
		expect(warnings).toHaveLength(1);
		expect(warnings[0].kind).toBe('command-without-function');
		expect(warnings[0].name).toBe('clean');
		expect(warnings[0].message).toContain('clean');
	});

	it('warns about a global function with no command', () => {
		const warnings = commandAgreement(['run'], 'function run()\nend\nfunction stray()\nend\n');
		expect(warnings).toHaveLength(1);
		expect(warnings[0].kind).toBe('function-without-command');
		expect(warnings[0].name).toBe('stray');
	});

	it('reports both directions at once, commands first', () => {
		const warnings = commandAgreement(['missing'], 'function stray()\nend\n');
		expect(warnings.map((w) => w.kind)).toEqual([
			'command-without-function',
			'function-without-command'
		]);
	});

	it('does not warn about a local helper', () => {
		expect(commandAgreement(['run'], 'local function helper()\nend\nfunction run()\nend\n')).toEqual(
			[]
		);
	});
});
