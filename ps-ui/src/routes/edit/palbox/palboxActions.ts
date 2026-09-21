import type { ActionDescriptor } from '$components/ui/actions/actionDescriptor';
import * as m from '$i18n/messages';
import { c, p } from '$lib/utils/commonTranslations';

export type PalboxActionContext = {
	selectionCount: number;
	addPal: () => void;
	addAllPals: () => void;
	selectAll: (includeParty: boolean) => void;
	healAll: () => void;
	cloneSelected: () => void;
	applyPreset: () => void;
	cloneSelectedToUps: () => void;
	healSelected: () => void;
	maxSelected: () => void;
	deleteSelected: () => void;
	clearSelection: () => void;
};

/** Box and box+party select-all are separate rows: `run` gets no event to tell them apart. */
export function buildPalboxActions(ctx: PalboxActionContext): ActionDescriptor[] {
	const hasSelection = () => ctx.selectionCount > 0;
	const hasOneSelected = () => ctx.selectionCount === 1;
	const pals = () => m.pal({ count: ctx.selectionCount });

	return [
		{
			id: 'palbox-add-pal',
			label: m.add_new_pal_to_entity({ entity: m.palbox() }),
			icon: 'tabler:plus',
			run: ctx.addPal
		},
		{
			id: 'palbox-add-all',
			label: m.add_all_pals_to_entity({ entity: m.palbox(), pals: c.pals }),
			icon: 'tabler:circle-plus',
			run: ctx.addAllPals
		},
		{
			id: 'palbox-select-all',
			label: `${m.select_all_in()} ${m.palbox()}`,
			icon: 'tabler:arrows-diff',
			run: () => ctx.selectAll(false)
		},
		{
			id: 'palbox-select-all-with-party',
			label: `${m.select_all_in()} ${m.pal_box_party()}`,
			icon: 'tabler:users',
			run: () => ctx.selectAll(true)
		},
		{
			id: 'palbox-heal-all',
			label: m.heal_all_in_entity({ entity: m.palbox() }),
			icon: 'tabler:bandage',
			run: ctx.healAll
		},
		{
			id: 'palbox-clone-selected',
			label: m.clone_selected_pal(p.pal),
			icon: 'tabler:copy',
			run: ctx.cloneSelected,
			available: hasOneSelected
		},
		{
			id: 'palbox-apply-preset',
			label: m.apply_preset_to_selected({ pals: pals() }),
			icon: 'tabler:player-play',
			run: ctx.applyPreset,
			available: hasSelection
		},
		{
			id: 'palbox-clone-to-ups',
			label: m.clone_pals_to_entity({
				count: ctx.selectionCount,
				pals: pals(),
				entity: c.universalPalStorage
			}),
			icon: 'tabler:upload',
			run: ctx.cloneSelectedToUps,
			available: hasSelection
		},
		{
			id: 'palbox-heal-selected',
			label: m.heal_selected_pals({ pals: pals() }),
			icon: 'tabler:ambulance',
			run: ctx.healSelected,
			available: hasSelection
		},
		{
			id: 'palbox-max-selected',
			label: m.max_out_selected_pals({ pals: pals() }),
			icon: 'ph:hand-fist',
			run: ctx.maxSelected,
			available: hasSelection
		},
		{
			id: 'palbox-delete-selected',
			label: m.delete_selected_entity({ entity: pals() }),
			icon: 'tabler:trash',
			run: ctx.deleteSelected,
			available: hasSelection,
			danger: true
		},
		{
			id: 'palbox-clear-selection',
			label: m.clear_selected_entity({ entity: pals() }),
			icon: 'tabler:x',
			run: ctx.clearSelection,
			available: hasSelection
		}
	];
}
