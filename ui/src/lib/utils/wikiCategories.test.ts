import { describe, expect, it } from 'vitest';
import { categoryHref, entityLink, WIKI_CATEGORIES } from './wikiCategories';

describe('WIKI_CATEGORIES', () => {
	it('covers all eight categories', () => {
		expect(WIKI_CATEGORIES).toHaveLength(8);
		expect(WIKI_CATEGORIES.map((c) => c.id)).toContain('pals');
		expect(WIKI_CATEGORIES.map((c) => c.id)).toContain('work-suitability');
	});
});

describe('categoryHref', () => {
	it('points every category at /wiki', () => {
		expect(categoryHref('pals')).toBe('/wiki/pals');
		expect(categoryHref('items')).toBe('/wiki/items');
		expect(categoryHref('elements')).toBe('/wiki/elements');
	});
});

describe('entityLink', () => {
	it('links entities to their wiki page', () => {
		expect(entityLink('pals', 'BluePlatypus')).toEqual({
			href: '/wiki/pals/blue-platypus'
		});
	});

	it('strips namespaced key prefixes before slugging', () => {
		expect(entityLink('active-skills', 'EPalWazaID::AcidRain')).toEqual({
			href: '/wiki/active-skills/acid-rain'
		});
	});
});
