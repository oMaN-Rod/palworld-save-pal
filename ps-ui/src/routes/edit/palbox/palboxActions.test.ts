import { describe, expect, it, vi } from 'vitest';

import { availableActions, sheetActions } from '$components/ui/actions/actionDescriptor';

import { buildPalboxActions, type PalboxActionContext } from './palboxActions';

function context(selectionCount: number): PalboxActionContext {
	return {
		selectionCount,
		addPal: vi.fn(),
		addAllPals: vi.fn(),
		selectAll: vi.fn(),
		healAll: vi.fn(),
		cloneSelected: vi.fn(),
		applyPreset: vi.fn(),
		cloneSelectedToUps: vi.fn(),
		healSelected: vi.fn(),
		maxSelected: vi.fn(),
		deleteSelected: vi.fn(),
		clearSelection: vi.fn()
	};
}

const alwaysOrder = [
	'palbox-add-pal',
	'palbox-add-all',
	'palbox-select-all',
	'palbox-select-all-with-party',
	'palbox-heal-all'
];

const bulkOrder = [
	'palbox-apply-preset',
	'palbox-clone-to-ups',
	'palbox-heal-selected',
	'palbox-max-selected',
	'palbox-delete-selected',
	'palbox-clear-selection'
];

const noSelectionOrder = alwaysOrder;
const oneSelectedOrder = [...alwaysOrder, 'palbox-clone-selected', ...bulkOrder];
const manySelectedOrder = [...alwaysOrder, ...bulkOrder];

const wiring: Record<string, { mock: keyof PalboxActionContext; args: unknown[] }> = {
	'palbox-add-pal': { mock: 'addPal', args: [] },
	'palbox-add-all': { mock: 'addAllPals', args: [] },
	'palbox-select-all': { mock: 'selectAll', args: [false] },
	'palbox-select-all-with-party': { mock: 'selectAll', args: [true] },
	'palbox-heal-all': { mock: 'healAll', args: [] },
	'palbox-clone-selected': { mock: 'cloneSelected', args: [] },
	'palbox-apply-preset': { mock: 'applyPreset', args: [] },
	'palbox-clone-to-ups': { mock: 'cloneSelectedToUps', args: [] },
	'palbox-heal-selected': { mock: 'healSelected', args: [] },
	'palbox-max-selected': { mock: 'maxSelected', args: [] },
	'palbox-delete-selected': { mock: 'deleteSelected', args: [] },
	'palbox-clear-selection': { mock: 'clearSelection', args: [] }
};

function idsAt(selectionCount: number): string[] {
	return availableActions(buildPalboxActions(context(selectionCount))).map((action) => action.id);
}

describe('buildPalboxActions', () => {
	it('describes every toolbar operation exactly once', () => {
		const ids = buildPalboxActions(context(1)).map((action) => action.id);
		expect(ids).toEqual(oneSelectedOrder);
		expect(new Set(ids).size).toBe(ids.length);
	});

	it('offers only the always-available operations with nothing selected', () => {
		expect(idsAt(0)).toEqual(noSelectionOrder);
	});

	it('offers clone-selected only when exactly one pal is selected', () => {
		expect(idsAt(1)).toEqual(oneSelectedOrder);
		expect(idsAt(2)).toEqual(manySelectedOrder);
		expect(idsAt(0)).not.toContain('palbox-clone-selected');
		expect(idsAt(2)).not.toContain('palbox-clone-selected');
	});

	it('offers the bulk operations for any non-empty selection', () => {
		expect(idsAt(2)).toEqual(manySelectedOrder);
		expect(idsAt(7)).toEqual(manySelectedOrder);
	});

	it('wires every descriptor to its own context callback, with its own argument', () => {
		for (const [id, expectation] of Object.entries(wiring)) {
			const ctx = context(1);
			const action = buildPalboxActions(ctx).find((candidate) => candidate.id === id);
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
		const dangerIds = buildPalboxActions(context(2))
			.filter((action) => action.danger)
			.map((action) => action.id);
		expect(dangerIds).toEqual(['palbox-delete-selected']);
	});

	it('sorts the destructive row last in the phone sheet', () => {
		const ids = sheetActions(buildPalboxActions(context(2))).map((action) => action.id);
		expect(ids.at(-1)).toBe('palbox-delete-selected');
		expect(ids).toEqual([
			...manySelectedOrder.filter((id) => id !== 'palbox-delete-selected'),
			'palbox-delete-selected'
		]);
	});

	it('gives every descriptor a non-empty label and icon', () => {
		for (const action of buildPalboxActions(context(2))) {
			expect(action.label.length, `${action.id} has no label`).toBeGreaterThan(0);
			expect(action.icon, `${action.id} has no icon`).toMatch(/^[a-z-]+:[a-z-]+$/);
		}
	});

	it('names the two select-all rows apart, since only Ctrl told them apart before', () => {
		const actions = buildPalboxActions(context(0));
		const box = actions.find((action) => action.id === 'palbox-select-all');
		const withParty = actions.find((action) => action.id === 'palbox-select-all-with-party');
		expect(box?.label).not.toBe(withParty?.label);
	});
});
