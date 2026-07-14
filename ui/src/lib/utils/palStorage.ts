/**
 * Slot-indexed pal storage (GPS, DPS). The records are declared `Record<number, Pal>`,
 * but `Object.entries` hands back STRING keys regardless — and the server's
 * `pal_indexes` is a `Vec<i32>`, which rejects `"3"` outright rather than coercing it.
 * Every slot index leaving this module is therefore a real number.
 */
type SlotStorage<T> = Record<number, T>;

type Identifiable = { instance_id: string };

function slots<T>(storage: SlotStorage<T>): [number, T][] {
	return Object.entries(storage).map(([index, value]) => [Number(index), value]);
}

/** Slot indexes of every pal whose `instance_id` is selected. */
export function selectedStorageIndexes<T extends Identifiable>(
	storage: SlotStorage<T>,
	selectedInstanceIds: string[]
): number[] {
	return slots(storage)
		.filter(([, pal]) => selectedInstanceIds.includes(pal.instance_id))
		.map(([index]) => index);
}

/** The slot a single pal occupies, or `undefined` when it is not stored. */
export function storageIndexOf<T extends Identifiable>(
	storage: SlotStorage<T>,
	instanceId: string
): number | undefined {
	return slots(storage).find(([, pal]) => pal.instance_id === instanceId)?.[0];
}

/** `storage` without the given slots — the optimistic local mirror of a delete. */
export function withoutStorageIndexes<T>(
	storage: SlotStorage<T>,
	deleted: number[]
): SlotStorage<T> {
	return Object.fromEntries(slots(storage).filter(([index]) => !deleted.includes(index)));
}
