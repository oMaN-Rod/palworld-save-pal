import { describe, expect, it } from 'vitest';

import { isSelectableItem } from './itemUtils';
import { ItemTypeA, ItemTypeB, Rarity, type Item } from '$types/game';

function item(details: Partial<Item['details']>): Item {
	return {
		id: 'Test',
		details: {
			group: 'Accessory',
			weight: 1,
			type_a: ItemTypeA.Accessory,
			type_b: ItemTypeB.Accessory,
			price: 1,
			icon: 'icon',
			rank: 1,
			rarity: Rarity.Common,
			max_stack_count: 1,
			sort_id: 1,
			...details
		},
		info: { localized_name: 'Test', description: '' }
	} as Item;
}

// The game rewrites a retired id onto its survivor when it loads a save, so offering one hands
// the player a different item than the one they picked. Accessory_AT_2/_3 are the retired tiers
// of the Attack Pendant and redirect to Accessory_AT_1.
describe('retired ids', () => {
	it('hides an item that redirects to another', () => {
		expect(isSelectableItem(item({ redirect_to: 'Accessory_AT_1' }))).toBe(false);
	});

	it('keeps the survivor the retired ids point at', () => {
		expect(isSelectableItem(item({}))).toBe(true);
	});
});

// Rarity variants are a separate mechanism: ClothArmor_2..5 are the Uncommon..Legendary tiers
// of a live item, they carry no redirect_to, and every one of them must stay selectable.
describe('rarity variants', () => {
	it.each([
		Rarity.Common,
		Rarity.Uncommon,
		Rarity.Rare,
		Rarity.Epic,
		Rarity.Legendary
	])('keeps the rarity %i variant selectable', (rarity) => {
		expect(isSelectableItem(item({ rarity, type_a: ItemTypeA.Armor }))).toBe(true);
	});
});

describe('existing exclusions', () => {
	it('hides an item with no type', () => {
		expect(isSelectableItem(item({ type_a: ItemTypeA.None }))).toBe(false);
	});

	it('hides pal-only equipment', () => {
		expect(isSelectableItem(item({ type_a: ItemTypeA.MonsterEquipWeapon }))).toBe(false);
	});

	it('hides an item the parser could not resolve an icon for', () => {
		expect(isSelectableItem(item({ disabled: true }))).toBe(false);
	});
});

// The stale EPalItemTypeB enum resolved these to NONE, which sent the row to the unroutable
// group "None"; they are live content and must reach a real group.
describe('newly recognised item types', () => {
	it.each([
		[ItemTypeB.WeaponFishingRod, 'Weapon'],
		[ItemTypeB.WeaponMetalDetector, 'Weapon'],
		[ItemTypeB.ConsumeFishingBait, 'Common'],
		[ItemTypeB.ConsumePalAwakening, 'Common'],
		[ItemTypeB.Essential_BossReward, 'KeyItem']
	])('keeps %s selectable', (type_b, group) => {
		expect(
			isSelectableItem(item({ type_a: ItemTypeA.Consume, type_b, group: group as never }))
		).toBe(true);
	});
});
