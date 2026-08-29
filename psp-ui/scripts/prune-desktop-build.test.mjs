import { existsSync, mkdirSync, mkdtempSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { afterEach, beforeEach, describe, expect, it } from 'vitest';
import { isWebOnlyEntry, pruneDesktopBuild } from './prune-desktop-build.mjs';

describe('isWebOnlyEntry', () => {
	it('drops the crawler files', () => {
		for (const name of ['sitemap.xml', 'sitemaps', 'robots.txt']) {
			expect(isWebOnlyEntry(name)).toBe(true);
		}
	});

	it('drops the Cloudflare headers and redirects', () => {
		expect(isWebOnlyEntry('_headers')).toBe(true);
		expect(isWebOnlyEntry('_redirects')).toBe(true);
	});

	it('drops the game data copied for the wasm worker', () => {
		expect(isWebOnlyEntry('data')).toBe(true);
	});

	it('drops localized roots and their directories', () => {
		expect(isWebOnlyEntry('fr.html')).toBe(true);
		expect(isWebOnlyEntry('fr')).toBe(true);
		expect(isWebOnlyEntry('pt-br.html')).toBe(true);
		expect(isWebOnlyEntry('zh')).toBe(true);
		expect(isWebOnlyEntry('zh-hant')).toBe(true);
	});

	it('keeps the english root, which has no locale slug', () => {
		expect(isWebOnlyEntry('index.html')).toBe(false);
		expect(isWebOnlyEntry('en')).toBe(false);
		expect(isWebOnlyEntry('en.html')).toBe(false);
	});

	it('keeps the assets the desktop app serves', () => {
		for (const name of ['_app', 'models', 'maps', 'guides', 'draco', 'vs', 'wiki', 'docs']) {
			expect(isWebOnlyEntry(name)).toBe(false);
		}
	});

	it('keeps routes whose names resemble a locale slug', () => {
		expect(isWebOnlyEntry('editor')).toBe(false);
		expect(isWebOnlyEntry('presets')).toBe(false);
	});
});

describe('pruneDesktopBuild', () => {
	let dir;

	beforeEach(() => {
		dir = mkdtempSync(join(tmpdir(), 'psp-prune-'));
	});

	afterEach(() => {
		rmSync(dir, { recursive: true, force: true });
	});

	function seed(files) {
		for (const [path, contents] of Object.entries(files)) {
			const full = join(dir, path);
			mkdirSync(join(full, '..'), { recursive: true });
			writeFileSync(full, contents);
		}
	}

	it('removes web-only entries and leaves the rest', () => {
		seed({
			'index.html': 'home',
			'robots.txt': 'User-agent: *',
			'sitemaps/pages.xml': '<urlset/>',
			'data/json/pals.json': '{}',
			'fr.html': 'bonjour',
			'fr/map.html': 'carte',
			'_app/immutable/app.js': 'app',
			'models/pal.glb': 'mesh'
		});

		pruneDesktopBuild(dir);

		expect(existsSync(join(dir, 'robots.txt'))).toBe(false);
		expect(existsSync(join(dir, 'sitemaps'))).toBe(false);
		expect(existsSync(join(dir, 'data'))).toBe(false);
		expect(existsSync(join(dir, 'fr.html'))).toBe(false);
		expect(existsSync(join(dir, 'fr'))).toBe(false);
		expect(existsSync(join(dir, 'index.html'))).toBe(true);
		expect(existsSync(join(dir, '_app/immutable/app.js'))).toBe(true);
		expect(existsSync(join(dir, 'models/pal.glb'))).toBe(true);
	});

	it('reports what it freed', () => {
		seed({ 'robots.txt': 'abcde', 'index.html': 'home' });

		const result = pruneDesktopBuild(dir);

		expect(result.removed).toEqual(['robots.txt']);
		expect(result.bytes).toBe(5);
	});

	it('is idempotent on an already-pruned build', () => {
		seed({ 'index.html': 'home' });

		pruneDesktopBuild(dir);
		const result = pruneDesktopBuild(dir);

		expect(result.removed).toEqual([]);
		expect(result.bytes).toBe(0);
		expect(existsSync(join(dir, 'index.html'))).toBe(true);
	});
});
