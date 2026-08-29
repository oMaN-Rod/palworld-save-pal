import { collectRelics, toggleRelic, type RelicPlayerView, type RelicRef } from '../features/relics';

export type ItemSlot = {
	static_id: string;
	slot_index: number;
	count: number;
	dynamic_item?: unknown;
};

export type RelicCountPlayer = { essential_container?: { slots: ItemSlot[] } };

export type SaveActionPlayer = RelicCountPlayer &
	RelicPlayerView & { unlocked_fast_travel_points?: string[] };

const same = (a: string, b: string) => a.toUpperCase() === b.toUpperCase();

export function updateRelicCount(player: RelicCountPlayer, delta: number): void {
	const container = player.essential_container;
	if (!container) return;
	const slot = container.slots.find((s) => s.static_id === 'Relic');
	if (slot) {
		slot.count += delta;
		if (slot.count <= 0) {
			slot.static_id = 'None';
			slot.count = 0;
			slot.dynamic_item = undefined;
		}
		return;
	}
	if (delta <= 0) return;
	const usedIndexes = new Set(
		container.slots.filter((s) => s.static_id !== 'None').map((s) => s.slot_index)
	);
	let slotIndex = 0;
	while (usedIndexes.has(slotIndex)) slotIndex++;
	const emptySlot = container.slots.find((s) => s.slot_index === slotIndex);
	if (emptySlot) {
		emptySlot.static_id = 'Relic';
		emptySlot.count = delta;
		emptySlot.dynamic_item = undefined;
		return;
	}
	container.slots.push({
		static_id: 'Relic',
		slot_index: slotIndex,
		count: delta,
		dynamic_item: undefined
	});
}

export function toggleRelicPoint(player: SaveActionPlayer, point: RelicRef): void {
	const delta = toggleRelic(player, point);
	if (delta !== 0) updateRelicCount(player, delta);
}

export function toggleFastTravelPoint(player: SaveActionPlayer, guid: string): void {
	const unlocks = player.unlocked_fast_travel_points ?? [];
	const index = unlocks.findIndex((existing) => same(existing, guid));
	if (index >= 0) {
		unlocks.splice(index, 1);
	} else {
		unlocks.push(guid);
	}
	player.unlocked_fast_travel_points = unlocks;
}

export function unlockFastTravelGuids(player: SaveActionPlayer, guids: string[]): number {
	const unlocked = player.unlocked_fast_travel_points ?? [];
	const existing = new Set(unlocked.map((guid) => guid.toUpperCase()));
	const toAdd = guids.filter((guid) => !existing.has(guid.toUpperCase()));
	if (toAdd.length === 0) return 0;
	player.unlocked_fast_travel_points = [...unlocked, ...toAdd];
	return toAdd.length;
}

export function collectAllRelics(player: SaveActionPlayer, points: RelicRef[]): number {
	const { added, capturePowerAdded } = collectRelics(player, points);
	if (added === 0) return 0;
	if (capturePowerAdded > 0) updateRelicCount(player, capturePowerAdded);
	return added;
}
