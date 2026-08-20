// Object.entries hands back STRING keys regardless of the Record<number, T>
// declaration, and the backend's pal_indexes is a Vec<i32> that rejects "3"
// outright rather than coercing it -- every slot index leaving this module must
// be a real number.
type SlotStorage<T> = Record<number, T>;

type Identifiable = { instance_id: string };

function slots<T>(storage: SlotStorage<T>): [number, T][] {
	return Object.entries(storage).map(([index, value]) => [Number(index), value]);
}

export function selectedStorageIndexes<T extends Identifiable>(
	storage: SlotStorage<T>,
	selectedInstanceIds: string[]
): number[] {
	return slots(storage)
		.filter(([, pal]) => selectedInstanceIds.includes(pal.instance_id))
		.map(([index]) => index);
}

export function storageIndexOf<T extends Identifiable>(
	storage: SlotStorage<T>,
	instanceId: string
): number | undefined {
	return slots(storage).find(([, pal]) => pal.instance_id === instanceId)?.[0];
}

export function withoutStorageIndexes<T>(
	storage: SlotStorage<T>,
	deleted: number[]
): SlotStorage<T> {
	return Object.fromEntries(slots(storage).filter(([index]) => !deleted.includes(index)));
}
