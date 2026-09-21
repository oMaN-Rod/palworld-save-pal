import type { ActionDescriptor } from '$components/ui/actions/actionDescriptor';
import * as m from '$i18n/messages';
import { c } from '$lib/utils/commonTranslations';

export type UpsActionContext = {
	selectionCount: number;
	/** Pals on the current server page only. */
	pageCount: number;
	/** Every pal the filters match, across all pages. */
	totalCount: number;
	filtered: boolean;
	selectAllOnPage: () => void;
	selectAllMatching: () => void;
	editTags: () => void;
	addToCollection: () => void;
	exportSelected: () => void;
	deleteSelected: () => void;
	clearSelection: () => void;
};

/** Two select-all rows because UPS pages on the server: this page vs. every match. */
export function buildUpsActions(ctx: UpsActionContext): ActionDescriptor[] {
	const hasSelection = () => ctx.selectionCount > 0;
	const pals = () => m.pal({ count: ctx.selectionCount });

	return [
		{
			id: 'ups-select-page',
			label: m.select_all_page_pals({ pals: c.pals, count: ctx.pageCount }),
			icon: 'tabler:note',
			run: ctx.selectAllOnPage
		},
		{
			id: 'ups-select-all',
			label: ctx.filtered
				? m.select_all_filtered_pals({ pals: c.pals, count: ctx.totalCount })
				: m.select_all_ups_pals({ pals: c.pals, count: ctx.totalCount }),
			icon: 'tabler:arrows-diff',
			run: ctx.selectAllMatching
		},
		{
			id: 'ups-edit-tags',
			label: m.edit_tags_for_pals({ count: ctx.selectionCount, pals: pals() }),
			icon: 'tabler:tag',
			run: ctx.editTags,
			available: hasSelection
		},
		{
			id: 'ups-add-to-collection',
			label: m.manage_collection_for_pals({ count: ctx.selectionCount, pals: pals() }),
			icon: 'tabler:folder',
			run: ctx.addToCollection,
			available: hasSelection
		},
		{
			id: 'ups-export-selected',
			label: m.export_pals({ count: ctx.selectionCount, pals: pals() }),
			icon: 'tabler:upload',
			run: ctx.exportSelected,
			available: hasSelection
		},
		{
			id: 'ups-delete-selected',
			label: m.delete_selected_entity({ entity: pals() }),
			icon: 'tabler:trash',
			run: ctx.deleteSelected,
			available: hasSelection,
			danger: true
		},
		{
			id: 'ups-clear-selection',
			label: m.clear_selected_entity({ entity: pals() }),
			icon: 'tabler:x',
			run: ctx.clearSelection,
			available: hasSelection
		}
	];
}
