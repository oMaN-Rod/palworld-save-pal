import { describe, expect, it, vi } from 'vitest';

import { availableActions } from '$components/ui/actions/actionDescriptor';

import { buildPlayerActions, type PlayerActionContext } from './playerActions';

function context(group: 'inventory' | 'key_items'): PlayerActionContext {
	return {
		group,
		sortCommonContainer: vi.fn(),
		fillCommonContainer: vi.fn(),
		setCommonContainerCount: vi.fn(),
		clearCommonContainer: vi.fn(),
		setEssentialList: vi.fn(),
		clearEssentialContainer: vi.fn(),
		clearWeaponLoadOutContainer: vi.fn(),
		clearEquipmentArmorContainer: vi.fn(),
		clearFoodEquipContainer: vi.fn(),
		clearAll: vi.fn()
	};
}

const inventoryOrder = [
	'sort-inventory',
	'fill-inventory',
	'set-inventory-count',
	'clear-inventory',
	'clear-weapons',
	'clear-armor',
	'clear-food',
	'clear-all'
];

const keyItemsOrder = [
	'add-pal-gear',
	'add-all-implants',
	'add-other-key-items',
	'clear-key-items',
	'clear-weapons',
	'clear-armor',
	'clear-food',
	'clear-all'
];

const wiring: Record<string, { mock: keyof PlayerActionContext; args: unknown[] }> = {
	'sort-inventory': { mock: 'sortCommonContainer', args: [] },
	'fill-inventory': { mock: 'fillCommonContainer', args: [] },
	'set-inventory-count': { mock: 'setCommonContainerCount', args: [] },
	'clear-inventory': { mock: 'clearCommonContainer', args: [] },
	'add-pal-gear': { mock: 'setEssentialList', args: ['gear'] },
	'add-all-implants': { mock: 'setEssentialList', args: ['implants'] },
	'add-other-key-items': { mock: 'setEssentialList', args: ['misc'] },
	'clear-key-items': { mock: 'clearEssentialContainer', args: [] },
	'clear-weapons': { mock: 'clearWeaponLoadOutContainer', args: [] },
	'clear-armor': { mock: 'clearEquipmentArmorContainer', args: [] },
	'clear-food': { mock: 'clearFoodEquipContainer', args: [] },
	'clear-all': { mock: 'clearAll', args: [] }
};

describe('buildPlayerActions', () => {
	it('offers the inventory actions in the inventory group', () => {
		const ids = availableActions(buildPlayerActions(context('inventory'))).map((a) => a.id);
		expect(ids).toContain('sort-inventory');
		expect(ids).toContain('fill-inventory');
		expect(ids).toContain('set-inventory-count');
		expect(ids).not.toContain('add-pal-gear');
	});

	it('offers the key-item actions in the key-items group', () => {
		const ids = availableActions(buildPlayerActions(context('key_items'))).map((a) => a.id);
		expect(ids).toContain('add-pal-gear');
		expect(ids).toContain('add-all-implants');
		expect(ids).not.toContain('sort-inventory');
	});

	it('always offers the clear actions in both groups', () => {
		for (const group of ['inventory', 'key_items'] as const) {
			const ids = availableActions(buildPlayerActions(context(group))).map((a) => a.id);
			expect(ids).toContain('clear-weapons');
			expect(ids).toContain('clear-armor');
			expect(ids).toContain('clear-food');
			expect(ids).toContain('clear-all');
		}
	});

	it('renders the inventory group in the original rail order', () => {
		const ids = availableActions(buildPlayerActions(context('inventory'))).map((a) => a.id);
		expect(ids).toEqual(inventoryOrder);
	});

	it('renders the key-items group in the original rail order', () => {
		const ids = availableActions(buildPlayerActions(context('key_items'))).map((a) => a.id);
		expect(ids).toEqual(keyItemsOrder);
	});

	it('gives every action a non-empty label', () => {
		for (const action of buildPlayerActions(context('inventory'))) {
			expect(action.label.length).toBeGreaterThan(0);
		}
	});

	it('marks only clear-all as destructive', () => {
		const actions = buildPlayerActions(context('inventory'));
		const dangerIds = actions.filter((action) => action.danger).map((action) => action.id);
		expect(dangerIds).toEqual(['clear-all']);
	});

	it('wires every descriptor to its expected context callback', () => {
		for (const [id, expectation] of Object.entries(wiring)) {
			const ctx = context('inventory');
			const action = buildPlayerActions(ctx).find((a) => a.id === id);
			action?.run();

			const mockFn = ctx[expectation.mock] as ReturnType<typeof vi.fn>;
			expect(mockFn, `${id} did not invoke ${expectation.mock}`).toHaveBeenCalledTimes(1);
			expect(mockFn).toHaveBeenCalledWith(...expectation.args);
		}
	});
});
