// @vitest-environment jsdom
import { getModalState } from '$states';
import type {
	GameInventoryContainerJson,
	GameInventoryJson,
	GameItemSlotJson
} from '$states/gameState.svelte';
import { fireEvent, render } from '@testing-library/svelte';
import { afterEach, describe, expect, it, vi } from 'vitest';
import '../../pal/__tests__/fixtures/matchMediaPolyfill';
import LiveLoadoutPane from '../LiveLoadoutPane.svelte';
import { clearItems, seedItems } from './fixtures/itemsFixture';

const EQUIPPED = [
	'YakushimaHeadEquip005',
	'SFArmorWeight_5',
	'Accessory_HeatColdResist_1',
	'Accessory_Nonkilling',
	'Shield_04',
	'Glider_Tera',
	'Accessory_TalentChecker',
	'Accessory_JumpCount_Increase3',
	'SphereModule_Homing'
];

function slot(slotIndex: number, staticItemId: string): GameItemSlotJson {
	return { slotIndex, staticItemId, count: 1 };
}

function container(
	type: string,
	slotNum: number,
	slots: GameItemSlotJson[] = [],
	status = 'ok'
): GameInventoryContainerJson {
	return { containerId: `${type}-1`, type, slotNum, status, slots, buildObjectId: null, baseId: null };
}

function inventory(containers: GameInventoryContainerJson[]): GameInventoryJson {
	return { playerUid: 'p1', status: 'ok', containers };
}

const equipContainer = container(
	'playerEquipArmor',
	9,
	EQUIPPED.map((id, index) => slot(index, id))
);

function alts(root: Element | null): (string | null)[] {
	return Array.from(root?.querySelectorAll('img') ?? []).map((img) => img.getAttribute('alt'));
}

afterEach(() => {
	clearItems();
	vi.restoreAllMocks();
});

describe('LiveLoadoutPane equipment ordinals', () => {
	it('places every gear ordinal in its own section', () => {
		seedItems(EQUIPPED);

		const { container: root } = render(LiveLoadoutPane, {
			props: { inventory: inventory([equipContainer]) }
		});

		expect(alts(root.querySelector('#live-gear-equip'))).toEqual([
			'YakushimaHeadEquip005',
			'SFArmorWeight_5',
			'Shield_04',
			'Glider_Tera',
			'SphereModule_Homing'
		]);
	});

	it('fills the accessory grid from the four non-contiguous accessory ordinals', () => {
		seedItems(EQUIPPED);

		const { container: root } = render(LiveLoadoutPane, {
			props: { inventory: inventory([equipContainer]) }
		});

		expect(alts(root.querySelector('#live-accessory-equip'))).toEqual([
			'Accessory_HeatColdResist_1',
			'Accessory_Nonkilling',
			'Accessory_TalentChecker',
			'Accessory_JumpCount_Increase3'
		]);
	});

	it('leaves the gear sections empty when the equip container reports nothing', () => {
		seedItems(EQUIPPED);

		const { container: root } = render(LiveLoadoutPane, {
			props: { inventory: inventory([container('playerEquipArmor', 9)]) }
		});

		expect(alts(root.querySelector('#live-gear-equip'))).toEqual([]);
		expect(root.querySelectorAll('#live-accessory-equip button')).toHaveLength(4);
		expect(alts(root.querySelector('#live-accessory-equip'))).toEqual([]);
	});
});

describe('LiveLoadoutPane slot counts', () => {
	it('renders every weapon slot the game reports', () => {
		const { container: root } = render(LiveLoadoutPane, {
			props: {
				inventory: inventory([container('weaponLoadout', 6, [slot(0, 'Weapon_AssaultRifle')])])
			}
		});

		expect(root.querySelectorAll('#live-weapon-equip button')).toHaveLength(6);
	});

	it('back-fills a sparse container up to its declared size', () => {
		const { container: root } = render(LiveLoadoutPane, {
			props: {
				inventory: inventory([container('common', 8, [slot(0, 'Wood'), slot(5, 'Stone')])])
			}
		});

		expect(root.querySelectorAll('#live-inventory-panel button')).toHaveLength(8);
	});

	it('reports a partially read loadout', () => {
		const { getByText } = render(LiveLoadoutPane, {
			props: {
				inventory: inventory([container('common', 4, [], 'partial')])
			}
		});

		expect(getByText('Part of this loadout could not be read.')).toBeTruthy();
	});
});

describe('LiveLoadoutPane without an edit handler', () => {
	it('opens no item picker from any slot', async () => {
		seedItems(EQUIPPED);
		const showModal = vi.spyOn(getModalState(), 'showModal');

		const { container: root } = render(LiveLoadoutPane, {
			props: {
				inventory: inventory([
					equipContainer,
					container('common', 4, [slot(0, 'Wood')]),
					container('essential', 2, [slot(0, 'Stone')]),
					container('weaponLoadout', 6, [slot(0, 'Weapon_AssaultRifle')]),
					container('foodEquip', 5, [slot(0, 'Bread')])
				])
			}
		});

		const badges = root.querySelectorAll('button');
		expect(badges.length).toBe(28);
		for (const badge of badges) await fireEvent.click(badge);

		expect(showModal).not.toHaveBeenCalled();
	});
});

describe('LiveLoadoutPane slot editing', () => {
	function editable(onSetSlot: ReturnType<typeof vi.fn>, editDisabledReason?: string) {
		return render(LiveLoadoutPane, {
			props: {
				inventory: inventory([container('common', 4, [slot(0, 'Wood')])]),
				onSetSlot,
				editDisabledReason
			}
		});
	}

	it('sends the container and slot the badge belongs to', async () => {
		seedItems(['Wood'], 99);
		vi.spyOn(getModalState(), 'showModal').mockResolvedValue(['Wood', 7, undefined]);
		const onSetSlot = vi.fn();

		const { container: root } = editable(onSetSlot);
		const badges = root.querySelectorAll('#live-inventory-panel button');
		await fireEvent.click(badges[2]);

		expect(onSetSlot).toHaveBeenCalledWith('common-1', 2, 'Wood', 7);
	});

	it('sends a null item when the picker clears the slot', async () => {
		seedItems(['Wood']);
		vi.spyOn(getModalState(), 'showModal').mockResolvedValue(['None', 0, undefined]);
		const onSetSlot = vi.fn();

		const { container: root } = editable(onSetSlot);
		await fireEvent.click(root.querySelectorAll('#live-inventory-panel button')[0]);

		expect(onSetSlot).toHaveBeenCalledWith('common-1', 0, null, 0);
	});

	it('opens no picker while a reason says editing is unavailable', async () => {
		seedItems(['Wood']);
		const showModal = vi.spyOn(getModalState(), 'showModal');
		const onSetSlot = vi.fn();

		const { container: root } = editable(onSetSlot, 'Another change is still running.');
		await fireEvent.click(root.querySelectorAll('#live-inventory-panel button')[0]);

		expect(showModal).not.toHaveBeenCalled();
		expect(onSetSlot).not.toHaveBeenCalled();
	});

	it('opens no picker for a container the read could not identify', async () => {
		seedItems(['Wood']);
		const showModal = vi.spyOn(getModalState(), 'showModal');
		const onSetSlot = vi.fn();

		const { container: root } = render(LiveLoadoutPane, {
			props: {
				inventory: inventory([
					{ ...container('common', 4, [slot(0, 'Wood')]), containerId: null }
				]),
				onSetSlot
			}
		});
		await fireEvent.click(root.querySelectorAll('#live-inventory-panel button')[0]);

		expect(showModal).not.toHaveBeenCalled();
		expect(onSetSlot).not.toHaveBeenCalled();
	});
});
