import { describe, it, expect } from 'vitest';
import { toSlug, buildSlugIndex, keyFromSlug, stripKeyPrefix, isHiddenRecord } from './wikiSlug';
import itemsJson from '../../../../data/json/items.json';
import itemsEn from '../../../../data/json/l10n/en/items.json';

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

// The wiki filters on this in five places - the category grid, the prerender entry list, the
// slug lookup, the SEO entity list and the pal route - so a record it misses gets a page.
describe('isHiddenRecord', () => {
	it('hides a record the parser could not resolve', () => {
		expect(isHiddenRecord({ disabled: true })).toBe(true);
		expect(isHiddenRecord({ details: { disabled: true } })).toBe(true);
	});

	// The game rewrites a retired id onto its survivor on load, so both would render the same
	// name and stats - one wiki entry describing the other.
	it('hides a record the game retires onto another', () => {
		expect(isHiddenRecord({ redirect_to: 'Accessory_AT_1' })).toBe(true);
		expect(isHiddenRecord({ details: { redirect_to: 'Accessory_AT_1' } })).toBe(true);
	});

	it('keeps a live record', () => {
		expect(isHiddenRecord({ disabled: false })).toBe(false);
		expect(isHiddenRecord({ details: { disabled: false } })).toBe(false);
	});

	it('ignores a non-record', () => {
		expect(isHiddenRecord(null)).toBe(false);
		expect(isHiddenRecord('Accessory_AT_1')).toBe(false);
		expect(isHiddenRecord(undefined)).toBe(false);
	});
});

// The three Attack Pendant tiers share one name because the game retired _2 and _3 onto _1.
// Rarity variants share a name too, but legitimately - they are distinct live items - so the
// guard is against retired ids reaching the wiki, not against duplicate names as such.
describe('wiki item listing', () => {
	const items = itemsJson as Record<string, Record<string, unknown>>;
	const names = itemsEn as Record<string, { localized_name?: string }>;
	const listed = Object.keys(items).filter((key) => !isHiddenRecord(items[key]));

	it('lists exactly one Attack Pendant', () => {
		expect(listed.filter((key) => key.startsWith('Accessory_AT_'))).toEqual(['Accessory_AT_1']);
	});

	it('lists no id the game retires', () => {
		expect(listed.filter((key) => typeof items[key].redirect_to === 'string')).toEqual([]);
	});

	it('still lists every rarity variant of a live item', () => {
		expect(listed.filter((key) => /^ClothArmor(_[2-5])?$/.test(key)).sort()).toEqual([
			'ClothArmor',
			'ClothArmor_2',
			'ClothArmor_3',
			'ClothArmor_4',
			'ClothArmor_5'
		]);
	});

	// Every retired id's survivor must itself be listed, or hiding the duplicate would have
	// removed the only entry describing that item.
	it('lists the survivor of every retired id', () => {
		const orphaned = Object.keys(items)
			.map((key) => items[key].redirect_to)
			.filter((target): target is string => typeof target === 'string')
			.filter((target) => !listed.includes(target));
		expect(orphaned).toEqual([]);
	});

	// Hiding retired ids clears all but three same-name collisions. These three are a different
	// pattern the redirect table does not cover: a legacy row the game left in place, marked
	// bLegalInGame=false and priced out, alongside the live row that replaced it. Pinned rather
	// than asserted empty so the set failing open is caught, and so growth is caught too.
	it('leaves only the three known legacy-row collisions', () => {
		const seen = new Map<string, string>();
		const collisions: string[] = [];
		for (const key of listed) {
			const label = names[key]?.localized_name;
			if (!label) continue;
			const fingerprint = `${label}|${items[key].rarity}`;
			const previous = seen.get(fingerprint);
			if (previous) collisions.push(`${previous} == ${key}`);
			else seen.set(fingerprint, key);
		}
		expect(collisions.sort()).toEqual([
			'Blueprint_Accessory_Avoid_1 == Blueprint_Accessory_Avoid_1_fix',
			'Gunpowder == Gunpowder2',
			'PalEgg_MutationPal == PalEgg_MutationPal_05'
		]);
	});
});
