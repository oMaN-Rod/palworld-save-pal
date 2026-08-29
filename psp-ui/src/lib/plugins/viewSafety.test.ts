import { readFileSync, readdirSync } from 'node:fs';
import { join } from 'node:path';
import { describe, expect, it } from 'vitest';
import { resolvePath, toText } from './pluginView';

const VIEW_DIR = 'src/routes/plugins/components/view';
const EXTRA_FILES = ['src/routes/plugins/components/ApplyBar.svelte'];

function viewSources(): { name: string; source: string }[] {
	const dirFiles = readdirSync(VIEW_DIR)
		.filter((name) => name.endsWith('.svelte'))
		.map((name) => ({ name, source: readFileSync(join(VIEW_DIR, name), 'utf8') }));
	const extraFiles = EXTRA_FILES.map((path) => ({
		name: path.slice(path.lastIndexOf('/') + 1),
		source: readFileSync(path, 'utf8')
	}));
	return [...dirFiles, ...extraFiles];
}

/// The vocabulary is closed and every kind has a renderer; a kind with none
/// would render as nothing at all, silently.
describe('the view renderer', () => {
	it('has a file for every widget kind to land in', () => {
		const names = viewSources().map((file) => file.name);
		expect(names).toContain('PluginView.svelte');
		expect(names).toContain('ViewSection.svelte');
		expect(names.length).toBeGreaterThanOrEqual(7);
	});

	/// A plugin ships JSON, never code: nothing it supplies may be
	/// interpolated as markup or handed to an evaluator.
	it.each([
		['{@html', 'renders a string as markup'],
		['innerHTML', 'writes a string into the DOM as markup'],
		['outerHTML', 'writes a string into the DOM as markup'],
		['insertAdjacentHTML', 'writes a string into the DOM as markup'],
		['document.write', 'writes a string into the DOM as markup'],
		['eval(', 'evaluates a string as code'],
		['new Function', 'evaluates a string as code'],
		['dangerouslySet', 'renders a string as markup']
	])('never uses %s, which %s', (needle) => {
		for (const { name, source } of viewSources()) {
			expect(source, `${name} must not contain ${needle}`).not.toContain(needle);
		}
	});

	/// A plugin-supplied string reaching a URL attribute is a javascript: URL
	/// away from being code, so no view component builds one at all.
	it('never binds a href or a src', () => {
		for (const { name, source } of viewSources()) {
			expect(source, `${name} must not set href`).not.toMatch(/href[=}]/);
			expect(source, `${name} must not set src`).not.toMatch(/\bsrc=/);
		}
	});

	/// The one place a plugin string could still reach a style attribute is an
	/// interpolated class or style; the column count is the only dynamic class
	/// the renderer has, and it comes from a clamped number, never a string.
	it('never interpolates a plugin value into a style attribute', () => {
		for (const { name, source } of viewSources()) {
			expect(source, `${name} must not build a style attribute`).not.toMatch(/style[:=]/);
		}
	});

	/// The model, not the component, is where a plugin string could smuggle a
	/// non-string out: everything a component prints has been through `toText`.
	it('renders every value as a string, whatever the plugin returned', () => {
		for (const value of [
			{ toString: () => 'x' },
			[1, 2],
			Symbol,
			() => 'x',
			Object.create({ inherited: 'y' })
		]) {
			expect(typeof toText(value)).toBe('string');
		}
	});

	it('never resolves a path onto the prototype chain', () => {
		expect(resolvePath({}, 'constructor.name')).toBeUndefined();
		expect(resolvePath({}, '__proto__.polluted')).toBeUndefined();
		expect(resolvePath([], 'constructor')).toBeUndefined();
	});
});
