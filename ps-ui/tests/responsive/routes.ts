import { readdirSync } from 'node:fs';
import { join } from 'node:path';
import { fileURLToPath } from 'node:url';

/** In the desktop build these render an empty state; the web build would redirect to `/upload`. */
export const CORE_ROUTES = [
	'/',
	'/map',
	'/edit/player',
	'/edit/palbox',
	'/edit/guild',
	'/edit/technologies',
	'/gps',
	'/ups',
	'/live'
] as const;

export const DOC_ROUTES = [
	'/about',
	'/privacy',
	'/docs',
	'/docs/guides',
	'/wiki',
	'/wiki/pals',
	'/upload',
	'/loading',
	'/error',
	'/signal',
	'/tools',
	'/overview',
	'/edit',
	'/edit/effigies',
	'/plugins'
] as const;

export function discoverRoutes(): string[] {
	const root = fileURLToPath(new URL('../../src/routes', import.meta.url));
	const found: string[] = [];

	const walk = (dir: string, url: string) => {
		const entries = readdirSync(dir, { withFileTypes: true });
		if (entries.some((entry) => entry.isFile() && PAGE_FILES.has(entry.name))) {
			found.push(url === '' ? '/' : url);
		}
		for (const entry of entries) {
			if (!entry.isDirectory() || entry.name.startsWith('__')) continue;
			walk(join(dir, entry.name), `${url}/${entry.name}`);
		}
	};
	walk(root, '');

	return found.map(resolveParams).sort();
}

const PAGE_FILES = new Set(['+page.svelte', '+page.ts', '+page.md']);

/** An unlisted dynamic route throws rather than being skipped. */
const DYNAMIC_SAMPLES: Record<string, string> = {
	'/plugins/[id]': '/plugins/does-not-exist',
	'/wiki/[category]': '/wiki/items',
	'/wiki/[category]/[slug]': '/wiki/items/aicore',
	'/wiki/pals/[slug]': '/wiki/pals/alpaca'
};

function resolveParams(route: string): string {
	if (!route.includes('[')) return route;
	const sample = DYNAMIC_SAMPLES[route];
	if (!sample) throw new Error(`No sample path for the dynamic route ${route}`);
	return sample;
}
