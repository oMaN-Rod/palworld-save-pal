import { describe, expect, it, vi } from 'vitest';

import { availableActions, sheetActions } from '$components/ui/actions/actionDescriptor';

import { buildGuildActions, type GuildActionContext } from './guildActions';

const BASE_ID = 'base-1';

function context(selectionCount: number, baseId = BASE_ID): GuildActionContext {
	return {
		selectionCount,
		baseId,
		addPal: vi.fn(),
		selectAll: vi.fn(),
		healAll: vi.fn(),
		applyPreset: vi.fn(),
		healSelected: vi.fn(),
		deleteSelected: vi.fn(),
		clearSelection: vi.fn()
	};
}

const alwaysOrder = ['guild-add-pal', 'guild-select-all', 'guild-heal-all'];

const bulkOrder = [
	'guild-apply-preset',
	'guild-heal-selected',
	'guild-delete-selected',
	'guild-clear-selection'
];

const noSelectionOrder = alwaysOrder;
const selectedOrder = [...alwaysOrder, ...bulkOrder];

// Asserting args catches a row wired to the right callback with the wrong base.
const wiring: Record<string, { mock: keyof GuildActionContext; args: unknown[] }> = {
	'guild-add-pal': { mock: 'addPal', args: [BASE_ID] },
	'guild-select-all': { mock: 'selectAll', args: [] },
	'guild-heal-all': { mock: 'healAll', args: [] },
	'guild-apply-preset': { mock: 'applyPreset', args: [] },
	'guild-heal-selected': { mock: 'healSelected', args: [] },
	'guild-delete-selected': { mock: 'deleteSelected', args: [] },
	'guild-clear-selection': { mock: 'clearSelection', args: [] }
};

function idsAt(selectionCount: number): string[] {
	return availableActions(buildGuildActions(context(selectionCount))).map((action) => action.id);
}

describe('buildGuildActions', () => {
	it('describes every toolbar operation exactly once', () => {
		const ids = buildGuildActions(context(1)).map((action) => action.id);
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
			const action = buildGuildActions(ctx).find((candidate) => candidate.id === id);
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

	it('adds to whichever base the context names, not a remembered one', () => {
		const ctx = context(0, 'base-7');
		buildGuildActions(ctx)
			.find((action) => action.id === 'guild-add-pal')
			?.run();
		expect(ctx.addPal).toHaveBeenCalledWith('base-7');
	});

	it('names the base in the rows that act on the whole base', () => {
		const actions = buildGuildActions(context(0));
		expect(actions.find((action) => action.id === 'guild-add-pal')?.label).toBe(
			'Add a new Pal to your Base'
		);
		expect(actions.find((action) => action.id === 'guild-select-all')?.label).toBe(
			'Select all in current base'
		);
		expect(actions.find((action) => action.id === 'guild-heal-all')?.label).toBe(
			'Heal all in Base'
		);
	});

	it('marks only delete-selected as destructive', () => {
		const dangerIds = buildGuildActions(context(2))
			.filter((action) => action.danger)
			.map((action) => action.id);
		expect(dangerIds).toEqual(['guild-delete-selected']);
	});

	it('sorts the destructive row last in the phone sheet', () => {
		const ids = sheetActions(buildGuildActions(context(2))).map((action) => action.id);
		expect(ids.at(-1)).toBe('guild-delete-selected');
		expect(ids).toEqual([
			...selectedOrder.filter((id) => id !== 'guild-delete-selected'),
			'guild-delete-selected'
		]);
	});

	it('gives every descriptor a non-empty label and icon', () => {
		for (const action of buildGuildActions(context(2))) {
			expect(action.label.length, `${action.id} has no label`).toBeGreaterThan(0);
			expect(action.icon, `${action.id} has no icon`).toMatch(/^[a-z-]+:[a-z-]+$/);
		}
	});

	it('names every row apart, since the rail shows labels rather than tooltips', () => {
		const labels = buildGuildActions(context(2)).map((action) => action.label);
		expect(new Set(labels).size).toBe(labels.length);
	});

	it('counts the selection in the labels that act on it', () => {
		const one = buildGuildActions(context(1));
		const many = buildGuildActions(context(4));
		for (const id of bulkOrder) {
			const singular = one.find((action) => action.id === id)?.label;
			const plural = many.find((action) => action.id === id)?.label;
			expect(singular, `${id} is not described`).toBeTruthy();
			expect(plural, `${id} does not count the selection`).not.toBe(singular);
		}
	});

	it('leaves the whole-base rows alone as the selection changes', () => {
		const none = buildGuildActions(context(0));
		const many = buildGuildActions(context(5));
		for (const id of alwaysOrder) {
			expect(many.find((action) => action.id === id)?.label).toBe(
				none.find((action) => action.id === id)?.label
			);
		}
	});
});
