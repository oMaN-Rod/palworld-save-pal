import type { ActionDescriptor } from '$components/ui/actions/actionDescriptor';
import * as m from '$i18n/messages';
import { p } from '$lib/utils/commonTranslations';

export type PlayerActionContext = {
	group: 'inventory' | 'key_items';
	sortCommonContainer: () => void;
	fillCommonContainer: () => void;
	setCommonContainerCount: () => void;
	clearCommonContainer: () => void;
	setEssentialList: (kind: 'gear' | 'implants' | 'misc') => void;
	clearEssentialContainer: () => void;
	clearWeaponLoadOutContainer: () => void;
	clearEquipmentArmorContainer: () => void;
	clearFoodEquipContainer: () => void;
	clearAll: () => void;
};

export function buildPlayerActions(ctx: PlayerActionContext): ActionDescriptor[] {
	const inInventory = () => ctx.group === 'inventory';
	const inKeyItems = () => ctx.group === 'key_items';

	return [
		{
			id: 'sort-inventory',
			label: m.sort_inventory(),
			icon: 'ph:sort-ascending',
			run: ctx.sortCommonContainer,
			available: inInventory
		},
		{
			id: 'fill-inventory',
			label: m.fill_entity({ entity: m.inventory() }),
			icon: 'tabler:paint',
			run: ctx.fillCommonContainer,
			available: inInventory
		},
		{
			id: 'set-inventory-count',
			label: m.set_inventory_count(),
			icon: 'tabler:hash',
			run: ctx.setCommonContainerCount,
			available: inInventory
		},
		{
			id: 'clear-inventory',
			label: m.clear_entity({ entity: m.inventory() }),
			icon: 'tabler:arrows-move-horizontal',
			run: ctx.clearCommonContainer,
			available: inInventory
		},
		{
			id: 'add-pal-gear',
			label: m.add_all_pal_gear(p.pal),
			icon: 'ph:paw-print',
			run: () => ctx.setEssentialList('gear'),
			available: inKeyItems
		},
		{
			id: 'add-all-implants',
			label: m.add_all_implants(),
			icon: 'tabler:activity',
			run: () => ctx.setEssentialList('implants'),
			available: inKeyItems
		},
		{
			id: 'add-other-key-items',
			label: m.add_other_key_items(),
			icon: 'tabler:key',
			run: () => ctx.setEssentialList('misc'),
			available: inKeyItems
		},
		{
			id: 'clear-key-items',
			label: m.clear_entity({ entity: m.key_items() }),
			icon: 'tabler:arrows-move-horizontal',
			run: ctx.clearEssentialContainer,
			available: inKeyItems
		},
		{
			id: 'clear-weapons',
			label: m.clear_entity({ entity: m.weapon({ count: 2 }) }),
			icon: 'tabler:swords',
			run: ctx.clearWeaponLoadOutContainer
		},
		{
			id: 'clear-armor',
			label: m.clear_entity({ entity: m.armor() }),
			icon: 'tabler:shield',
			run: ctx.clearEquipmentArmorContainer
		},
		{
			id: 'clear-food',
			label: m.clear_entity({ entity: m.food() }),
			icon: 'tabler:pizza',
			run: ctx.clearFoodEquipContainer
		},
		{
			id: 'clear-all',
			label: m.clear_all(),
			icon: 'tabler:bomb',
			run: ctx.clearAll,
			danger: true
		}
	];
}
