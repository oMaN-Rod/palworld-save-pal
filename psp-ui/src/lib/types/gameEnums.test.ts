import { describe, expect, it } from 'vitest';

import itemsJson from '../../../../data/json/items.json';
import elementsJson from '../../../../data/json/elements.json';
import { ItemTypeA, ItemTypeB } from './game';

const items = Object.values(itemsJson as Record<string, { type_a: string; type_b: string }>);

function distinct(field: 'type_a' | 'type_b'): string[] {
	return [...new Set(items.map((i) => i[field]))].sort();
}

// A member the pak carries but the enum does not is silent: the parser's fallback stamps
// "None"/"NONE" on the row, which sends it to the unroutable group and drops it out of the
// item picker. That is how fishing rods, bait, Pal awakening items and the rest went missing,
// so the enums are pinned against the shipped artifact rather than trusted to stay current.
describe('item type enums cover the shipped data', () => {
	it('declares every type_a present in items.json', () => {
		const declared = new Set<string>(Object.values(ItemTypeA));
		expect(distinct('type_a').filter((v) => !declared.has(v))).toEqual([]);
	});

	it('declares every type_b present in items.json', () => {
		const declared = new Set<string>(Object.values(ItemTypeB));
		expect(distinct('type_b').filter((v) => !declared.has(v))).toEqual([]);
	});

	// The two enums disagree on how the fallback spells itself, and the artifact carries both
	// spellings. Comparing against the wrong one silently never matches.
	it('spells each fallback the way the artifact does', () => {
		expect(ItemTypeA.None).toBe('None');
		expect(ItemTypeB.None).toBe('NONE');
	});

	// SphereModule is not an EPalItemTypeA/B member at all - the parser short-circuits any id
	// containing it and stamps the literal on type_a, type_b and group alike.
	it('declares the SphereModule literal the parser stamps', () => {
		expect(distinct('type_a')).toContain(ItemTypeA.SphereModule);
		expect(distinct('type_b')).toContain(ItemTypeB.SphereModule);
	});
});

// elements.json is keyed by the game's internal element names, and pals.json spells
// element_types the same way. The union previously carried display names (Ground, Neutral,
// Grass, Electric) that appear nowhere in the data or the code.
describe('ElementType matches the element data', () => {
	it('has a key in elements.json for every declared element', () => {
		const keys = Object.keys(elementsJson as Record<string, unknown>).sort();
		expect(keys).toEqual(['Dark', 'Dragon', 'Earth', 'Electricity', 'Fire', 'Ice', 'Leaf', 'Normal', 'Water']);
	});
});
