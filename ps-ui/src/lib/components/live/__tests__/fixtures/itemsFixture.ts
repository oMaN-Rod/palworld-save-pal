import { itemsData } from '$lib/data';
import { ItemTypeA, ItemTypeB, Rarity, type Item } from '$types';
import { normalizeKeys } from '$utils';

export function seedItems(staticIds: string[], maxStackCount = 1): void {
	const items: Record<string, Item> = {};
	for (const id of staticIds) {
		items[id] = {
			id,
			details: {
				group: 'Common',
				weight: 0,
				type_a: ItemTypeA.Material,
				type_b: ItemTypeB.None,
				price: 0,
				icon: id,
				rank: 0,
				rarity: Rarity.Common,
				max_stack_count: maxStackCount,
				sort_id: 0
			},
			info: { localized_name: id, description: '' }
		};
	}
	itemsData.items = items;
	(itemsData as unknown as { keyMap: Record<string, string> }).keyMap = normalizeKeys(
		Object.keys(items)
	);
}

export function clearItems(): void {
	itemsData.items = {};
	(itemsData as unknown as { keyMap: Record<string, string> }).keyMap = {};
}
