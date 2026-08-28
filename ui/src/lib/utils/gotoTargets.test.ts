import { readdirSync, readFileSync, statSync } from 'node:fs';
import { join, resolve } from 'node:path';
import { describe, expect, it } from 'vitest';

const SRC = resolve(__dirname, '../..');
const ROUTES = join(SRC, 'routes');

function walk(dir: string): string[] {
	return readdirSync(dir).flatMap((name) => {
		const full = join(dir, name);
		if (statSync(full).isDirectory()) return walk(full);
		return /\.(ts|svelte)$/.test(name) && !/\.test\.ts$/.test(name) ? [full] : [];
	});
}

// Literal goto('/x') / goto('/x/y') only. Template literals carry runtime
// params and cannot be resolved against the route tree statically.
const GOTO = /goto\(\s*'(\/[a-z0-9\-/]*)'\s*\)/gi;

function literalGotoTargets() {
	const found = new Map<string, string>();
	for (const file of walk(SRC)) {
		const text = readFileSync(file, 'utf8');
		for (const [, path] of text.matchAll(GOTO)) {
			if (!found.has(path)) found.set(path, file.slice(SRC.length + 1));
		}
	}
	return found;
}

function routeExists(path: string) {
	if (path === '/') return true;
	const dir = join(ROUTES, ...path.split('/').filter(Boolean));
	try {
		return statSync(dir).isDirectory();
	} catch {
		return false;
	}
}

describe('goto targets', () => {
	// A route deleted in a refactor leaves its callers pointing at nothing. On
	// desktop that is a redirect loop, not a 404 page: psp-server rewrites the
	// missing path to /?path=<it>, the shell boots and navigates straight back.
	it('every literal goto target has a route', () => {
		const dead = [...literalGotoTargets()]
			.filter(([path]) => !routeExists(path))
			.map(([path, file]) => `${path} (${file})`);

		expect(dead).toEqual([]);
	});

	it('finds the targets it is meant to be checking', () => {
		const targets = [...literalGotoTargets().keys()];

		expect(targets).toContain('/overview');
		expect(targets.length).toBeGreaterThan(3);
	});
});
