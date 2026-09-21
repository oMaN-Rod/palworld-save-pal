import { describe, expect, it, vi } from 'vitest';

import { availableActions, sheetActions } from '$components/ui/actions/actionDescriptor';

import { buildUpsActions, type UpsActionContext } from './upsActions';

const PAGE_COUNT = 56;
const TOTAL_COUNT = 112;

function context(
	selectionCount: number,
	overrides: Partial<UpsActionContext> = {}
): UpsActionContext {
	return {
		selectionCount,
		pageCount: PAGE_COUNT,
		totalCount: TOTAL_COUNT,
		filtered: false,
		selectAllOnPage: vi.fn(),
		selectAllMatching: vi.fn(),
		editTags: vi.fn(),
		addToCollection: vi.fn(),
		exportSelected: vi.fn(),
		deleteSelected: vi.fn(),
		clearSelection: vi.fn(),
		...overrides
	};
}

const alwaysOrder = ['ups-select-page', 'ups-select-all'];

const bulkOrder = [
	'ups-edit-tags',
	'ups-add-to-collection',
	'ups-export-selected',
	'ups-delete-selected',
	'ups-clear-selection'
];

const noSelectionOrder = alwaysOrder;
const selectedOrder = [...alwaysOrder, ...bulkOrder];

const wiring: Record<string, { mock: keyof UpsActionContext; args: unknown[] }> = {
	'ups-select-page': { mock: 'selectAllOnPage', args: [] },
	'ups-select-all': { mock: 'selectAllMatching', args: [] },
	'ups-edit-tags': { mock: 'editTags', args: [] },
	'ups-add-to-collection': { mock: 'addToCollection', args: [] },
	'ups-export-selected': { mock: 'exportSelected', args: [] },
	'ups-delete-selected': { mock: 'deleteSelected', args: [] },
	'ups-clear-selection': { mock: 'clearSelection', args: [] }
};

function idsAt(selectionCount: number): string[] {
	return availableActions(buildUpsActions(context(selectionCount))).map((action) => action.id);
}

function labelOf(actions: ReturnType<typeof buildUpsActions>, id: string): string | undefined {
	return actions.find((action) => action.id === id)?.label;
}

describe('buildUpsActions', () => {
	it('describes every toolbar operation exactly once', () => {
		const ids = buildUpsActions(context(1)).map((action) => action.id);
		expect(ids).toEqual(selectedOrder);
		expect(new Set(ids).size).toBe(ids.length);
	});

	it('offers only the always-available operations with nothing selected', () => {
		expect(idsAt(0)).toEqual(noSelectionOrder);
	});

	it('offers the bulk operations for any non-empty selection', () => {
		expect(idsAt(1)).toEqual(selectedOrder);
		expect(idsAt(2)).toEqual(selectedOrder);
		expect(idsAt(56)).toEqual(selectedOrder);
	});

	it('wires every descriptor to its own context callback, with its own argument', () => {
		for (const [id, expectation] of Object.entries(wiring)) {
			const ctx = context(1);
			const action = buildUpsActions(ctx).find((candidate) => candidate.id === id);
			expect(action, `${id} is not described`).toBeDefined();
			action?.run();

			for (const [name, value] of Object.entries(ctx)) {
				if (typeof value !== 'function') continue;
				const mockFn = value as ReturnType<typeof vi.fn>;
				if (name === expectation.mock) {
					expect(mockFn, `${id} did not invoke ${name}`).toHaveBeenCalledTimes(1);
					expect(mockFn, `${id} invoked ${name} with the wrong argument`).toHaveBeenCalledWith(
						...expectation.args
					);
				} else {
					expect(mockFn, `${id} also invoked ${name}`).not.toHaveBeenCalled();
				}
			}
		}
	});

	it('keeps the two select-alls apart, by count as well as by callback', () => {
		const actions = buildUpsActions(context(0, { pageCount: 56, totalCount: 112 }));
		expect(labelOf(actions, 'ups-select-page')).toBe('Select all Pals on current page (56)');
		expect(labelOf(actions, 'ups-select-all')).toBe('Select all Pals in UPS (112)');
	});

	it('says the selection is of the filtered set when something narrows it', () => {
		const actions = buildUpsActions(context(0, { filtered: true, totalCount: 9 }));
		expect(labelOf(actions, 'ups-select-all')).toBe('Select all filtered Pals (9)');
	});

	it('keeps one stable id for that row whether or not a filter is active', () => {
		expect(buildUpsActions(context(0, { filtered: false })).map((a) => a.id)).toEqual(
			buildUpsActions(context(0, { filtered: true })).map((a) => a.id)
		);
	});

	it('marks only delete-selected as destructive', () => {
		const dangerIds = buildUpsActions(context(2))
			.filter((action) => action.danger)
			.map((action) => action.id);
		expect(dangerIds).toEqual(['ups-delete-selected']);
	});

	it('sorts the destructive row last in the phone sheet', () => {
		const ids = sheetActions(buildUpsActions(context(2))).map((action) => action.id);
		expect(ids.at(-1)).toBe('ups-delete-selected');
		expect(ids).toEqual([
			...selectedOrder.filter((id) => id !== 'ups-delete-selected'),
			'ups-delete-selected'
		]);
	});

	it('gives every descriptor a non-empty label and icon', () => {
		for (const action of buildUpsActions(context(2))) {
			expect(action.label.length, `${action.id} has no label`).toBeGreaterThan(0);
			expect(action.icon, `${action.id} has no icon`).toMatch(/^[a-z-]+:[a-z-]+$/);
		}
	});

	it('names every row apart, since the rail shows labels rather than tooltips', () => {
		const labels = buildUpsActions(context(2)).map((action) => action.label);
		expect(new Set(labels).size).toBe(labels.length);
	});

	it('counts the selection in the labels that act on it', () => {
		const one = buildUpsActions(context(1));
		const many = buildUpsActions(context(4));
		for (const id of bulkOrder) {
			const singular = labelOf(one, id);
			const plural = labelOf(many, id);
			expect(singular, `${id} is not described`).toBeTruthy();
			expect(plural, `${id} does not count the selection`).not.toBe(singular);
		}
	});

	it('leaves the select-all counts alone as the selection grows', () => {
		const empty = buildUpsActions(context(0));
		const some = buildUpsActions(context(7));
		for (const id of alwaysOrder) {
			expect(labelOf(some, id), `${id} counted the selection`).toBe(labelOf(empty, id));
		}
	});
});
