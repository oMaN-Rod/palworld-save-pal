import { describe, expect, it } from 'vitest';

import {
	EPalPlayerEquipItemSlotType,
	PLAYER_EQUIP_ACCESSORY_SLOTS,
	SPHERE_MODULE_SLOT
} from './game';

describe('EPalPlayerEquipItemSlotType', () => {
	it('places head, body, shield, glider and the capture item modifier at their game ordinals', () => {
		expect(EPalPlayerEquipItemSlotType.Head).toBe(0);
		expect(EPalPlayerEquipItemSlotType.Body).toBe(1);
		expect(EPalPlayerEquipItemSlotType.Shield).toBe(4);
		expect(EPalPlayerEquipItemSlotType.Glider).toBe(5);
		expect(EPalPlayerEquipItemSlotType.CaptureItemModifier).toBe(8);
	});

	it('keeps the accessory ordinals non-contiguous around shield and glider', () => {
		expect(PLAYER_EQUIP_ACCESSORY_SLOTS).toEqual([2, 3, 6, 7]);
	});

	it('aliases the capture item modifier as the sphere module slot', () => {
		expect(SPHERE_MODULE_SLOT).toBe(EPalPlayerEquipItemSlotType.CaptureItemModifier);
	});

	it('caps the container at the enum Max', () => {
		expect(EPalPlayerEquipItemSlotType.Max).toBe(9);
	});
});
