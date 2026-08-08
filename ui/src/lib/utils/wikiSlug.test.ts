import { describe, it, expect } from 'vitest';
import { toSlug, buildSlugIndex, keyFromSlug, stripKeyPrefix } from './wikiSlug';

describe('toSlug', () => {
	it('splits camelCase into kebab', () => {
		expect(toSlug('BluePlatypus')).toBe('blue-platypus');
		expect(toSlug('PowerShot')).toBe('power-shot');
	});

	it('lowercases single words', () => {
		expect(toSlug('Wool')).toBe('wool');
		expect(toSlug('Alpaca')).toBe('alpaca');
	});

	it('collapses underscores and spaces to single hyphens', () => {
		expect(toSlug('Blue_Platypus')).toBe('blue-platypus');
		expect(toSlug('Blue  Platypus')).toBe('blue-platypus');
		expect(toSlug('A__B')).toBe('a-b');
	});

	it('handles digit boundaries without splitting runs', () => {
		expect(toSlug('Tier2Armor')).toBe('tier2-armor');
	});

	it('trims leading and trailing separators', () => {
		expect(toSlug('_Wool_')).toBe('wool');
	});

	it('strips illegal filesystem characters like colons', () => {
		expect(toSlug('EPalWazaID::AcidRain')).toBe('epal-waza-id-acid-rain');
	});

	it('collapses other punctuation to hyphens', () => {
		expect(toSlug("Tier2's Armor (Special)")).toBe('tier2-s-armor-special');
	});
});

describe('buildSlugIndex / keyFromSlug', () => {
	it('round-trips every key', () => {
		const keys = ['BluePlatypus', 'Wool', 'PowerShot', 'Alpaca'];
		const index = buildSlugIndex(keys);
		for (const key of keys) {
			expect(keyFromSlug(toSlug(key), index)).toBe(key);
		}
	});

	it('returns undefined for an unknown slug', () => {
		const index = buildSlugIndex(['Wool']);
		expect(keyFromSlug('not-a-thing', index)).toBeUndefined();
	});

	it('is case-insensitive on lookup', () => {
		const index = buildSlugIndex(['BluePlatypus']);
		expect(keyFromSlug('Blue-Platypus', index)).toBe('BluePlatypus');
	});

	it('throws on a slug collision', () => {
		expect(() => buildSlugIndex(['Blue_Platypus', 'Blue-Platypus'])).toThrow();
	});
});

describe('stripKeyPrefix', () => {
	it('strips a namespace prefix', () => {
		expect(stripKeyPrefix('EPalWazaID::AcidRain')).toBe('AcidRain');
	});

	it('leaves unprefixed keys alone', () => {
		expect(stripKeyPrefix('Wool')).toBe('Wool');
		expect(stripKeyPrefix('AirDash_1')).toBe('AirDash_1');
	});

	it('strips only up to the last separator', () => {
		expect(stripKeyPrefix('A::B::C')).toBe('C');
	});
});
