// Removes the web-only half of ui_build after a desktop `vite build`.
//
// adapter-static writes ui_build once Vite's hooks have already run, so this
// cannot be a Vite plugin. It runs from `build:desktop` instead, which keeps the
// list in one place for every packaging path (Tauri's beforeBuildCommand, the
// build-desktop scripts) rather than repeating it per script.
//
// One entry is stale rather than merely unwanted: static/data/json is a
// gitignored build product, so a machine that once ran build:web leaves
// ui_build/data behind on every later desktop build.
import { existsSync, readdirSync, rmSync, statSync } from 'node:fs';
import { dirname, join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';
import { localeSlugs } from '../src/lib/i18n/routingConfig.js';

const WEB_ONLY = new Set([
	'sitemap.xml',
	'sitemaps',
	'robots.txt',
	// Cloudflare Workers static-asset config; psp-server sets its own headers.
	'_headers',
	'_redirects',
	// gen-json-manifest.mjs copies data/json here for the wasm worker to fetch.
	// Desktop gets game data over the websocket and bundles what it imports.
	'data'
]);

// English has no slug, so index.html is not a localized root.
const LOCALE_ENTRIES = new Set(
	Object.values(localeSlugs)
		.filter(Boolean)
		.flatMap((slug) => [slug, `${slug}.html`])
);

/** @param {string} name a top-level entry name in ui_build */
export function isWebOnlyEntry(name) {
	return WEB_ONLY.has(name) || LOCALE_ENTRIES.has(name);
}

/** @param {string} path */
function sizeOf(path) {
	const stats = statSync(path);
	if (!stats.isDirectory()) return stats.size;
	let total = 0;
	for (const name of readdirSync(path)) total += sizeOf(join(path, name));
	return total;
}

/**
 * @param {string} buildDir
 * @returns {{ removed: string[], bytes: number }}
 */
export function pruneDesktopBuild(buildDir) {
	const removed = [];
	let bytes = 0;
	for (const name of readdirSync(buildDir)) {
		if (!isWebOnlyEntry(name)) continue;
		const path = join(buildDir, name);
		bytes += sizeOf(path);
		rmSync(path, { recursive: true, force: true });
		removed.push(name);
	}
	return { removed: removed.sort(), bytes };
}

if (process.argv[1] === fileURLToPath(import.meta.url)) {
	const buildDir = resolve(dirname(dirname(fileURLToPath(import.meta.url))), '../ui_build');
	if (!existsSync(buildDir)) {
		console.error(`[prune-desktop-build] ${buildDir} does not exist; run the build first`);
		process.exit(1);
	}
	const { removed, bytes } = pruneDesktopBuild(buildDir);
	const mb = (bytes / 1024 / 1024).toFixed(1);
	console.log(`[prune-desktop-build] removed ${removed.length} entries, freed ${mb} MB`);
}
