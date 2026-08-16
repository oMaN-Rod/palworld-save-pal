import { describe, expect, it } from 'vitest';
import { buildSitemapIndex, buildUrlEntry, buildUrlset } from './generate-sitemap.mjs';

describe('generate-sitemap', () => {
	it('emits hreflang alternates for localized paths', () => {
		const xml = buildUrlEntry('/', { localized: true });
		expect(xml).toContain('<loc>https://palworldsavepal.app/</loc>');
		expect(xml).toContain('hreflang="x-default"');
		expect(xml).toContain('hreflang="zh-Hans"');
	});

	it('omits alternates for English-only paths', () => {
		const xml = buildUrlEntry('/wiki/pals/lamball', { localized: false });
		expect(xml).toContain('<loc>https://palworldsavepal.app/wiki/pals/lamball</loc>');
		expect(xml).not.toContain('hreflang');
	});

	it('escapes XML-significant characters in URLs', () => {
		const xml = buildUrlEntry('/wiki/items/a&b', { localized: false });
		expect(xml).toContain('a&amp;b');
		expect(xml).not.toContain('a&b');
	});

	it('wraps entries in a urlset with the xhtml namespace', () => {
		const xml = buildUrlset(['<url></url>']);
		expect(xml).toContain('xmlns:xhtml="http://www.w3.org/1999/xhtml"');
		expect(xml.startsWith('<?xml version="1.0" encoding="UTF-8"?>')).toBe(true);
	});

	it('builds an index referencing each child sitemap', () => {
		const xml = buildSitemapIndex(['sitemaps/pals.xml']);
		expect(xml).toContain('<sitemapindex');
		expect(xml).toContain('https://palworldsavepal.app/sitemaps/pals.xml');
	});
});
