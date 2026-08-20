import { describe, expect, it } from 'vitest';
import { slugify } from './pluginId';

describe('slugify', () => {
	it('lowercases and joins words with a single separator', () => {
		expect(slugify('My First Plugin')).toBe('my-first-plugin');
	});

	it('collapses runs of spaces into a single separator', () => {
		expect(slugify('Too   Many    Spaces')).toBe('too-many-spaces');
	});

	it('trims trailing punctuation', () => {
		expect(slugify('Loot Table!!!')).toBe('loot-table');
	});

	it('trims leading punctuation', () => {
		expect(slugify('--Loot Table')).toBe('loot-table');
	});

	it('yields the empty string for a name that is entirely punctuation', () => {
		expect(slugify('!!!')).toBe('');
		expect(slugify('   ')).toBe('');
	});

	it('lowercases mixed-case input', () => {
		expect(slugify('LOUD Plugin Name')).toBe('loud-plugin-name');
	});

	it('truncates a name longer than the id the server accepts', () => {
		expect(slugify('a'.repeat(100))).toBe('a'.repeat(64));
	});

	it('leaves a name of exactly the maximum length alone', () => {
		expect(slugify('a'.repeat(64))).toBe('a'.repeat(64));
	});

	it('trims a separator the truncation lands on', () => {
		expect(slugify(`${'a'.repeat(63)} tail`)).toBe('a'.repeat(63));
	});

	it('trims a separator a collapsed run of punctuation left behind', () => {
		expect(slugify(`${'a'.repeat(63)} !!! tail`)).toBe('a'.repeat(63));
	});

	it('keeps a truncation that lands mid-word', () => {
		expect(slugify(`${'a'.repeat(60)} bbbbbb`)).toBe(`${'a'.repeat(60)}-bbb`);
	});
});
