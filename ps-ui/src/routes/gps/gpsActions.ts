import type { ActionDescriptor } from '$components/ui/actions/actionDescriptor';
import * as m from '$i18n/messages';
import { c } from '$lib/utils/commonTranslations';

export type GpsActionContext = {
	selectionCount: number;
	addAllPals: () => void;
	selectAll: () => void;
	applyPreset: () => void;
	cloneSelectedToUps: () => void;
	cloneSelectedToPlayer: () => void;
	deleteSelected: () => void;
	clearSelection: () => void;
};

/** Two clone rows because `run` takes no argument to pick a destination. */
export function buildGpsActions(ctx: GpsActionContext): ActionDescriptor[] {
	const hasSelection = () => ctx.selectionCount > 0;
	const pals = () => m.pal({ count: ctx.selectionCount });

	return [
		{
			id: 'gps-add-all',
			label: m.add_all_pals_to_entity({ entity: m.gps(), pals: c.pals }),
			icon: 'tabler:circle-plus',
			run: ctx.addAllPals
		},
		{
			id: 'gps-select-all',
			label: `${m.select_all_in()} ${m.gps()}`,
			icon: 'tabler:arrows-diff',
			run: ctx.selectAll
		},
		{
			id: 'gps-apply-preset',
			label: m.apply_preset_to_selected({ pals: pals() }),
			icon: 'tabler:player-play',
			run: ctx.applyPreset,
			available: hasSelection
		},
		{
			id: 'gps-clone-to-ups',
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
			id: 'gps-clone-to-player',
			label: m.clone_pals_to_entity({
				count: ctx.selectionCount,
				pals: pals(),
				entity: c.player
			}),
			icon: 'tabler:users',
			run: ctx.cloneSelectedToPlayer,
			available: hasSelection
		},
		{
			id: 'gps-delete-selected',
			label: m.delete_selected_entity({ entity: pals() }),
			icon: 'tabler:trash',
			run: ctx.deleteSelected,
			available: hasSelection,
			danger: true
		},
		{
			id: 'gps-clear-selection',
			label: m.clear_selected_entity({ entity: pals() }),
			icon: 'tabler:x',
			run: ctx.clearSelection,
			available: hasSelection
		}
	];
}
