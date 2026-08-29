import { readFile } from 'node:fs/promises';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';
import { describe, expect, it } from 'vitest';
import { checkFileBudget, checkPageMarkup, checkRedirects } from './check-build-output.mjs';

const LIMITS = { maxFiles: 18000, maxBytes: 25 * 1024 * 1024 };

describe('checkFileBudget', () => {
	it('passes a build inside the budget', () => {
		const result = checkFileBudget([{ path: 'a.html', size: 100 }], LIMITS);
		expect(result.errors).toEqual([]);
	});

	it('fails when the file count exceeds the limit', () => {
		const files = Array.from({ length: 18001 }, (_, i) => ({ path: `f${i}`, size: 1 }));
		const result = checkFileBudget(files, LIMITS);
		expect(result.errors[0]).toContain('18001');
	});

	it('fails when a single file exceeds 25 MiB', () => {
		const result = checkFileBudget([{ path: 'big.wasm', size: 26 * 1024 * 1024 }], LIMITS);
		expect(result.errors[0]).toContain('big.wasm');
	});
});

describe('checkPageMarkup', () => {
	const good =
		'<html lang="en"><head><title>T</title><meta name="description" content="d" />' +
		'<link rel="canonical" href="https://palworldsavepal.app/" /></head><body><h1>H</h1></body></html>';

	it('accepts a fully rendered page', () => {
		expect(checkPageMarkup(good)).toEqual([]);
	});

	it('rejects an unsubstituted lang placeholder', () => {
		expect(checkPageMarkup(good.replace('lang="en"', 'lang="%lang%"'))).toContain(
			'unsubstituted %lang% placeholder'
		);
	});

	it('rejects a page with no title', () => {
		expect(checkPageMarkup(good.replace('<title>T</title>', ''))).toContain('missing <title>');
	});

	it('rejects a page with no h1', () => {
		expect(checkPageMarkup(good.replace('<h1>H</h1>', ''))).toContain('missing <h1>');
	});

	it('rejects a page with no canonical', () => {
		expect(checkPageMarkup(good.replace(/<link rel="canonical"[^>]*>/, ''))).toContain(
			'missing canonical link'
		);
	});
});

describe('checkRedirects', () => {
	it('accepts relative sources, comments and blank lines', () => {
		const text = '# canonical locale aliases\n/zh-hans /zh 301\n\n/zh-hans/* /zh/:splat 301\n';
		expect(checkRedirects(text)).toEqual([]);
	});

	it('accepts an external destination', () => {
		expect(checkRedirects('/discord https://discord.gg/example 302\n')).toEqual([]);
	});

	it('rejects a domain-level source, which Workers assets refuse', () => {
		const text = 'https://www.palworldsavepal.app/* https://palworldsavepal.app/:splat 301\n';
		expect(checkRedirects(text)[0]).toContain('line 1');
		expect(checkRedirects(text)[0]).toContain('relative');
	});

	it('rejects a source that is not a path', () => {
		expect(checkRedirects('zh-hans /zh 301\n')[0]).toContain('line 1');
	});

	it('rejects an unsupported status code', () => {
		expect(checkRedirects('/old /new 418\n')[0]).toContain('418');
	});

	it('rejects a rule missing a destination', () => {
		expect(checkRedirects('/old\n')[0]).toContain('line 1');
	});

	it('rejects carriage returns, which end up inside the status token', () => {
		expect(checkRedirects('/zh-hans /zh 301\r\n')[0]).toContain('carriage return');
	});
});

describe('the shipped _redirects file', () => {
	it('is valid for Cloudflare Workers static assets', async () => {
		const path = join(dirname(fileURLToPath(import.meta.url)), '../static/_redirects');
		expect(checkRedirects(await readFile(path, 'utf8'))).toEqual([]);
	});
});
