import type { ActionDescriptor } from '$components/ui/actions/actionDescriptor';
import * as m from '$i18n/messages';
import { c } from '$lib/utils/commonTranslations';

export type DpsActionContext = {
	selectionCount: number;
	addAllPals: () => void;
	selectAll: () => void;
	applyPreset: () => void;
	cloneSelectedToUps: () => void;
	deleteSelected: () => void;
	clearSelection: () => void;
};

/** No move rows: a DPS slot has nowhere to move to. */
export function buildDpsActions(ctx: DpsActionContext): ActionDescriptor[] {
	const hasSelection = () => ctx.selectionCount > 0;
	const pals = () => m.pal({ count: ctx.selectionCount });

	return [
		{
			id: 'dps-add-all',
			label: m.add_all_pals_to_entity({ entity: m.dps(), pals: c.pals }),
			icon: 'tabler:circle-plus',
			run: ctx.addAllPals
		},
		{
			id: 'dps-select-all',
			label: `${m.select_all_in()} ${m.dps()}`,
			icon: 'tabler:arrows-diff',
			run: ctx.selectAll
		},
		{
			id: 'dps-apply-preset',
			label: m.apply_preset_to_selected({ pals: pals() }),
			icon: 'tabler:player-play',
			run: ctx.applyPreset,
			available: hasSelection
		},
		{
			id: 'dps-clone-to-ups',
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
			id: 'dps-delete-selected',
			label: m.delete_selected_entity({ entity: pals() }),
			icon: 'tabler:trash',
			run: ctx.deleteSelected,
			available: hasSelection,
			danger: true
		},
		{
			id: 'dps-clear-selection',
			label: m.clear_selected_entity({ entity: pals() }),
			icon: 'tabler:x',
			run: ctx.clearSelection,
			available: hasSelection
		}
	];
}
