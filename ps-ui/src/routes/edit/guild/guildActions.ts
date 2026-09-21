import type { ActionDescriptor } from '$components/ui/actions/actionDescriptor';
import * as m from '$i18n/messages';
import { c } from '$lib/utils/commonTranslations';

export type GuildActionContext = {
	selectionCount: number;
	/** The base on screen; `run` takes no argument, so rows close over it. */
	baseId: string;
	addPal: (baseId: string) => void;
	selectAll: () => void;
	healAll: () => void;
	applyPreset: () => void;
	healSelected: () => void;
	deleteSelected: () => void;
	clearSelection: () => void;
};

export function buildGuildActions(ctx: GuildActionContext): ActionDescriptor[] {
	const hasSelection = () => ctx.selectionCount > 0;
	const pals = () => m.pal({ count: ctx.selectionCount });

	return [
		{
			id: 'guild-add-pal',
			label: m.add_new_pal_to_entity({ entity: c.base }),
			icon: 'tabler:plus',
			run: () => ctx.addPal(ctx.baseId)
		},
		{
			id: 'guild-select-all',
			label: m.select_all_current_base(),
			icon: 'tabler:arrows-diff',
			run: ctx.selectAll
		},
		{
			id: 'guild-heal-all',
			label: m.heal_all_in_entity({ entity: c.base }),
			icon: 'tabler:bandage',
			run: ctx.healAll
		},
		{
			id: 'guild-apply-preset',
			label: m.apply_preset_to_selected({ pals: pals() }),
			icon: 'tabler:player-play',
			run: ctx.applyPreset,
			available: hasSelection
		},
		{
			id: 'guild-heal-selected',
			label: m.heal_selected_pals({ pals: pals() }),
			icon: 'tabler:ambulance',
			run: ctx.healSelected,
			available: hasSelection
		},
		{
			id: 'guild-delete-selected',
			label: m.delete_selected_entity({ entity: pals() }),
			icon: 'tabler:trash',
			run: ctx.deleteSelected,
			available: hasSelection,
			danger: true
		},
		{
			id: 'guild-clear-selection',
			label: m.clear_selected_entity({ entity: pals() }),
			icon: 'tabler:x',
			run: ctx.clearSelection,
			available: hasSelection
		}
	];
}
