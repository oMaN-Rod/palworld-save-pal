import { describe, it, expect } from 'vitest';
import {
	updateRelicCount,
	toggleRelicPoint,
	toggleFastTravelPoint,
	unlockFastTravelGuids,
	collectAllRelics,
	type ItemSlot,
	type SaveActionPlayer
} from './saveMapActions';

function playerWith(slots: ItemSlot[]): SaveActionPlayer {
	return { essential_container: { slots } };
}

describe('updateRelicCount', () => {
	it('adds to an existing Relic slot', () => {
		const player = playerWith([{ static_id: 'Relic', slot_index: 0, count: 3 }]);
		updateRelicCount(player, 2);
		expect(player.essential_container!.slots[0].count).toBe(5);
	});

	it('empties a slot that drops to zero', () => {
		const player = playerWith([
			{ static_id: 'Relic', slot_index: 0, count: 1, dynamic_item: { a: 1 } }
		]);
		updateRelicCount(player, -1);
		expect(player.essential_container!.slots[0]).toMatchObject({
			static_id: 'None',
			count: 0,
			dynamic_item: undefined
		});
	});

	it('claims the lowest free slot index when none exists yet', () => {
		const player = playerWith([
			{ static_id: 'Bread', slot_index: 0, count: 1 },
			{ static_id: 'None', slot_index: 1, count: 0 }
		]);
		updateRelicCount(player, 4);
		expect(player.essential_container!.slots[1]).toMatchObject({
			static_id: 'Relic',
			count: 4
		});
	});

	it('appends a slot when no free slot object exists', () => {
		const player = playerWith([{ static_id: 'Bread', slot_index: 0, count: 1 }]);
		updateRelicCount(player, 2);
		expect(player.essential_container!.slots).toHaveLength(2);
		expect(player.essential_container!.slots[1]).toMatchObject({
			static_id: 'Relic',
			slot_index: 1,
			count: 2
		});
	});

	it('does nothing without an essential container', () => {
		const player: SaveActionPlayer = {};
		expect(() => updateRelicCount(player, 1)).not.toThrow();
	});

	it('ignores a removal when the player holds no relics', () => {
		const player = playerWith([{ static_id: 'Bread', slot_index: 0, count: 1 }]);
		updateRelicCount(player, -1);
		expect(player.essential_container!.slots).toHaveLength(1);
	});
});

describe('toggleRelicPoint', () => {
	it('collecting an effigy raises the Relic item count', () => {
		const player = playerWith([]);
		toggleRelicPoint(player, { guid: 'g-1', relic_type: 'capture_power' });
		expect(player.collected_effigies).toEqual(['g-1']);
		expect(player.essential_container!.slots[0]).toMatchObject({ static_id: 'Relic', count: 1 });
	});

	it('un-collecting the same effigy returns the count to zero', () => {
		const player = playerWith([]);
		toggleRelicPoint(player, { guid: 'g-1', relic_type: 'capture_power' });
		toggleRelicPoint(player, { guid: 'g-1', relic_type: 'capture_power' });
		expect(player.collected_effigies).toEqual([]);
		expect(player.essential_container!.slots[0]).toMatchObject({ static_id: 'None', count: 0 });
	});

	it('a non-effigy relic never touches the item count', () => {
		const player = playerWith([]);
		toggleRelicPoint(player, { guid: 'g-9', relic_type: 'sealed_realm' });
		expect(player.collected_relics!.sealed_realm).toEqual(['g-9']);
		expect(player.essential_container!.slots).toHaveLength(0);
	});
});

describe('toggleFastTravelPoint', () => {
	it('adds an unlock, case-insensitively removing it again', () => {
		const player: SaveActionPlayer = { unlocked_fast_travel_points: [] };
		toggleFastTravelPoint(player, 'abc');
		expect(player.unlocked_fast_travel_points).toEqual(['abc']);
		toggleFastTravelPoint(player, 'ABC');
		expect(player.unlocked_fast_travel_points).toEqual([]);
	});

	it('seeds the list when the player has none', () => {
		const player: SaveActionPlayer = {};
		toggleFastTravelPoint(player, 'abc');
		expect(player.unlocked_fast_travel_points).toEqual(['abc']);
	});
});

describe('unlockFastTravelGuids', () => {
	it('adds only what is missing and reports the count', () => {
		const player: SaveActionPlayer = { unlocked_fast_travel_points: ['AAA'] };
		expect(unlockFastTravelGuids(player, ['aaa', 'bbb'])).toBe(1);
		expect(player.unlocked_fast_travel_points).toEqual(['AAA', 'bbb']);
	});

	it('is a no-op when everything is already unlocked', () => {
		const player: SaveActionPlayer = { unlocked_fast_travel_points: ['AAA'] };
		expect(unlockFastTravelGuids(player, ['aaa'])).toBe(0);
	});
});

describe('collectAllRelics', () => {
	it('collects the missing relics and credits only new effigies', () => {
		const player = playerWith([]);
		player.collected_relics = { capture_power: ['g-1'] };
		const added = collectAllRelics(player, [
			{ guid: 'g-1', relic_type: 'capture_power' },
			{ guid: 'g-2', relic_type: 'capture_power' },
			{ guid: 'g-3', relic_type: 'sealed_realm' }
		]);
		expect(added).toBe(2);
		expect(player.essential_container!.slots[0]).toMatchObject({ static_id: 'Relic', count: 1 });
	});

	it('returns zero and leaves the count alone when nothing is new', () => {
		const player = playerWith([]);
		player.collected_relics = { capture_power: ['g-1'] };
		expect(collectAllRelics(player, [{ guid: 'g-1', relic_type: 'capture_power' }])).toBe(0);
		expect(player.essential_container!.slots).toHaveLength(0);
	});
});
