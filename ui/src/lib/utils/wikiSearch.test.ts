import { describe, expect, it } from 'vitest';
import { searchWiki, type WikiSearchEntry } from './wikiSearch';

const entries: WikiSearchEntry[] = [
	{ category: 'pals', key: 'Lamball', name: 'Lamball' },
	{ category: 'items', key: 'LamballMutton', name: 'Lamball Mutton' },
	{ category: 'buildings', key: 'Ranch', name: 'Ranch' },
	{ category: 'pals', key: 'Cattiva', name: 'Cattiva' }
];

describe('searchWiki', () => {
	it('returns nothing for an empty query', () => {
		expect(searchWiki('', entries)).toEqual([]);
		expect(searchWiki('   ', entries)).toEqual([]);
	});

	it('ranks exact match above prefix match', () => {
		const result = searchWiki('lamball', entries);
		expect(result[0].name).toBe('Lamball');
		expect(result[1].name).toBe('Lamball Mutton');
	});

	it('ranks prefix above substring', () => {
		const sample: WikiSearchEntry[] = [
			{ category: 'pals', key: 'A', name: 'Xen Ball' },
			{ category: 'pals', key: 'B', name: 'Ballistic' }
		];
		const result = searchWiki('ball', sample);
		expect(result[0].name).toBe('Ballistic');
	});

	it('is case-insensitive', () => {
		expect(searchWiki('LAMBALL', entries)[0].name).toBe('Lamball');
	});

	it('excludes non-matching entries', () => {
		const names = searchWiki('lamball', entries).map((r) => r.name);
		expect(names).not.toContain('Ranch');
		expect(names).not.toContain('Cattiva');
	});

	it('respects the limit', () => {
		expect(searchWiki('a', entries, 2)).toHaveLength(2);
	});

	it('searches across categories', () => {
		const cats = searchWiki('lamball', entries).map((r) => r.category);
		expect(cats).toContain('pals');
		expect(cats).toContain('items');
	});
});
