import { describe, expect, it, vi } from 'vitest';

import { availableActions, sheetActions } from '$components/ui/actions/actionDescriptor';

import { buildGpsActions, type GpsActionContext } from './gpsActions';

function context(selectionCount: number): GpsActionContext {
	return {
		selectionCount,
		addAllPals: vi.fn(),
		selectAll: vi.fn(),
		applyPreset: vi.fn(),
		cloneSelectedToUps: vi.fn(),
		cloneSelectedToPlayer: vi.fn(),
		deleteSelected: vi.fn(),
		clearSelection: vi.fn()
	};
}

const alwaysOrder = ['gps-add-all', 'gps-select-all'];

const bulkOrder = [
	'gps-apply-preset',
	'gps-clone-to-ups',
	'gps-clone-to-player',
	'gps-delete-selected',
	'gps-clear-selection'
];

const noSelectionOrder = alwaysOrder;
const selectedOrder = [...alwaysOrder, ...bulkOrder];

const wiring: Record<string, { mock: keyof GpsActionContext; args: unknown[] }> = {
	'gps-add-all': { mock: 'addAllPals', args: [] },
	'gps-select-all': { mock: 'selectAll', args: [] },
	'gps-apply-preset': { mock: 'applyPreset', args: [] },
	'gps-clone-to-ups': { mock: 'cloneSelectedToUps', args: [] },
	'gps-clone-to-player': { mock: 'cloneSelectedToPlayer', args: [] },
	'gps-delete-selected': { mock: 'deleteSelected', args: [] },
	'gps-clear-selection': { mock: 'clearSelection', args: [] }
};

function idsAt(selectionCount: number): string[] {
	return availableActions(buildGpsActions(context(selectionCount))).map((action) => action.id);
}

describe('buildGpsActions', () => {
	it('describes every toolbar operation exactly once', () => {
		const ids = buildGpsActions(context(1)).map((action) => action.id);
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
			const action = buildGpsActions(ctx).find((candidate) => candidate.id === id);
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

	it('keeps the two clone destinations apart', () => {
		const actions = buildGpsActions(context(3));
		const ups = actions.find((action) => action.id === 'gps-clone-to-ups');
		const player = actions.find((action) => action.id === 'gps-clone-to-player');
		expect(ups?.label).toBe('Clone 3 Pals to Universal Pal Storage');
		expect(player?.label).toBe('Clone 3 Pals to Player');
	});

	it('marks only delete-selected as destructive', () => {
		const dangerIds = buildGpsActions(context(2))
			.filter((action) => action.danger)
			.map((action) => action.id);
		expect(dangerIds).toEqual(['gps-delete-selected']);
	});

	it('sorts the destructive row last in the phone sheet', () => {
		const ids = sheetActions(buildGpsActions(context(2))).map((action) => action.id);
		expect(ids.at(-1)).toBe('gps-delete-selected');
		expect(ids).toEqual([
			...selectedOrder.filter((id) => id !== 'gps-delete-selected'),
			'gps-delete-selected'
		]);
	});

	it('gives every descriptor a non-empty label and icon', () => {
		for (const action of buildGpsActions(context(2))) {
			expect(action.label.length, `${action.id} has no label`).toBeGreaterThan(0);
			expect(action.icon, `${action.id} has no icon`).toMatch(/^[a-z-]+:[a-z-]+$/);
		}
	});

	it('names every row apart, since the rail shows labels rather than tooltips', () => {
		const labels = buildGpsActions(context(2)).map((action) => action.label);
		expect(new Set(labels).size).toBe(labels.length);
	});

	it('counts the selection in the labels that act on it', () => {
		const one = buildGpsActions(context(1));
		const many = buildGpsActions(context(4));
		for (const id of bulkOrder) {
			const singular = one.find((action) => action.id === id)?.label;
			const plural = many.find((action) => action.id === id)?.label;
			expect(singular, `${id} is not described`).toBeTruthy();
			expect(plural, `${id} does not count the selection`).not.toBe(singular);
		}
	});
});
