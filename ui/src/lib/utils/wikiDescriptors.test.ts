import { describe, it, expect, vi } from 'vitest';

vi.mock('$lib/data', () => ({
	activeSkillsData: { activeSkills: {} },
	buildingsData: { buildings: {} },
	elementsData: { elements: {} },
	itemsData: { items: {} },
	palsData: { pals: {} },
	passiveSkillsData: { passiveSkills: {} },
	technologiesData: { technologies: {} },
	workSuitabilityData: { workSuitability: {} },
	WORK_SUITABILITY_KEYS: []
}));

import { DESCRIPTORS, descriptorFor } from './wikiDescriptors';
import { WIKI_CATEGORIES } from './wikiCategories';

describe('DESCRIPTORS', () => {
	it('has a descriptor for every WikiCategory', () => {
		for (const { id } of WIKI_CATEGORIES) {
			expect(DESCRIPTORS[id]).toBeDefined();
			expect(descriptorFor(id)).toBe(DESCRIPTORS[id]);
		}
	});

	it('declares at least one field, description, or related accessor per category', () => {
		for (const { id } of WIKI_CATEGORIES) {
			const descriptor = DESCRIPTORS[id];
			const hasContent =
				descriptor.fields.length > 0 ||
				typeof descriptor.description === 'function' ||
				typeof descriptor.related === 'function';
			expect(hasContent).toBe(true);
		}
	});

	describe('displayName', () => {
		it('falls back to the key when the record is undefined', () => {
			for (const { id } of WIKI_CATEGORIES) {
				expect(DESCRIPTORS[id].displayName('SomeKey', undefined)).toBe('SomeKey');
			}
		});

		it('falls back to the key when the record lacks a localized name', () => {
			for (const { id } of WIKI_CATEGORIES) {
				expect(DESCRIPTORS[id].displayName('SomeKey', {})).toBe('SomeKey');
			}
		});

		it('uses the localized name when present', () => {
			expect(DESCRIPTORS.buildings.displayName('Altar', { localized_name: 'Altar of Fate' })).toBe(
				'Altar of Fate'
			);
			expect(
				DESCRIPTORS.items.displayName('AIcore', { info: { localized_name: 'Ancient Core' } })
			).toBe('Ancient Core');
		});
	});

	describe('fields[].value', () => {
		it('returns null rather than throwing on an empty record', () => {
			for (const { id } of WIKI_CATEGORIES) {
				for (const wikiField of DESCRIPTORS[id].fields) {
					expect(() => wikiField.value({})).not.toThrow();
					expect(wikiField.value({})).toBeNull();
				}
			}
		});
	});
});
