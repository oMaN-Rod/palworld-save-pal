import { describe, expect, it, vi } from 'vitest';

import { availableActions, sheetActions } from '$components/ui/actions/actionDescriptor';

import { buildDpsActions, type DpsActionContext } from './dpsActions';

function context(selectionCount: number): DpsActionContext {
	return {
		selectionCount,
		addAllPals: vi.fn(),
		selectAll: vi.fn(),
		applyPreset: vi.fn(),
		cloneSelectedToUps: vi.fn(),
		deleteSelected: vi.fn(),
		clearSelection: vi.fn()
	};
}

const alwaysOrder = ['dps-add-all', 'dps-select-all'];

const bulkOrder = [
	'dps-apply-preset',
	'dps-clone-to-ups',
	'dps-delete-selected',
	'dps-clear-selection'
];

const noSelectionOrder = alwaysOrder;
const selectedOrder = [...alwaysOrder, ...bulkOrder];

const wiring: Record<string, { mock: keyof DpsActionContext; args: unknown[] }> = {
	'dps-add-all': { mock: 'addAllPals', args: [] },
	'dps-select-all': { mock: 'selectAll', args: [] },
	'dps-apply-preset': { mock: 'applyPreset', args: [] },
	'dps-clone-to-ups': { mock: 'cloneSelectedToUps', args: [] },
	'dps-delete-selected': { mock: 'deleteSelected', args: [] },
	'dps-clear-selection': { mock: 'clearSelection', args: [] }
};

function idsAt(selectionCount: number): string[] {
	return availableActions(buildDpsActions(context(selectionCount))).map((action) => action.id);
}

describe('buildDpsActions', () => {
	it('describes every toolbar operation exactly once', () => {
		const ids = buildDpsActions(context(1)).map((action) => action.id);
		expect(ids).toEqual(selectedOrder);
		expect(new Set(ids).size).toBe(ids.length);
	});

	it('offers only the always-available operations with nothing selected', () => {
		expect(idsAt(0)).toEqual(noSelectionOrder);
	});

	it('offers the bulk operations for any non-empty selection', () => {
		expect(idsAt(1)).toEqual(selectedOrder);
		expect(idsAt(2)).toEqual(selectedOrder);
		expect(idsAt(9)).toEqual(selectedOrder);
	});

	it('wires every descriptor to its own context callback, with its own argument', () => {
		for (const [id, expectation] of Object.entries(wiring)) {
			const ctx = context(1);
			const action = buildDpsActions(ctx).find((candidate) => candidate.id === id);
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

	it('marks only delete-selected as destructive', () => {
		const dangerIds = buildDpsActions(context(2))
			.filter((action) => action.danger)
			.map((action) => action.id);
		expect(dangerIds).toEqual(['dps-delete-selected']);
	});

	it('sorts the destructive row last in the phone sheet', () => {
		const ids = sheetActions(buildDpsActions(context(2))).map((action) => action.id);
		expect(ids.at(-1)).toBe('dps-delete-selected');
		expect(ids).toEqual([
			...selectedOrder.filter((id) => id !== 'dps-delete-selected'),
			'dps-delete-selected'
		]);
	});

	it('gives every descriptor a non-empty label and icon', () => {
		for (const action of buildDpsActions(context(2))) {
			expect(action.label.length, `${action.id} has no label`).toBeGreaterThan(0);
			expect(action.icon, `${action.id} has no icon`).toMatch(/^[a-z-]+:[a-z-]+$/);
		}
	});

	it('names every row apart, since the rail shows labels rather than tooltips', () => {
		const labels = buildDpsActions(context(2)).map((action) => action.label);
		expect(new Set(labels).size).toBe(labels.length);
	});

	it('counts the selection in the labels that act on it', () => {
		const one = buildDpsActions(context(1));
		const many = buildDpsActions(context(4));
		for (const id of bulkOrder) {
			const singular = one.find((action) => action.id === id)?.label;
			const plural = many.find((action) => action.id === id)?.label;
			expect(singular, `${id} is not described`).toBeTruthy();
			expect(plural, `${id} does not count the selection`).not.toBe(singular);
		}
	});
});
