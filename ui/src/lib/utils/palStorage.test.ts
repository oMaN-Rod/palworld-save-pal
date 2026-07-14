import { describe, expect, it } from 'vitest';
import { selectedStorageIndexes, storageIndexOf, withoutStorageIndexes } from './palStorage';

/**
 * GPS and DPS pals are held in a slot-indexed record. `Object.entries` hands back
 * STRING keys whatever the declared type says, and the server's `pal_indexes` is a
 * `Vec<i32>` — a `"3"` there is rejected outright with
 * `invalid type: string "3", expected i32`, so the delete never happens.
 */
const storage = {
	0: { instance_id: 'aaa' },
	3: { instance_id: 'bbb' },
	7: { instance_id: 'ccc' }
};

describe('selectedStorageIndexes', () => {
	it('returns numbers, not the string keys Object.entries yields', () => {
		const indexes = selectedStorageIndexes(storage, ['bbb', 'ccc']);
		expect(indexes).toEqual([3, 7]);
		for (const index of indexes) {
			expect(typeof index).toBe('number');
		}
	});

	it('is empty when nothing is selected', () => {
		expect(selectedStorageIndexes(storage, [])).toEqual([]);
	});
});

describe('storageIndexOf', () => {
	it('finds a single pal by instance id as a number', () => {
		expect(storageIndexOf(storage, 'ccc')).toBe(7);
	});

	it('is undefined for a pal that is not stored', () => {
		expect(storageIndexOf(storage, 'nope')).toBeUndefined();
	});
});

describe('withoutStorageIndexes', () => {
	it('drops exactly the deleted slots, matching on numeric keys', () => {
		expect(withoutStorageIndexes(storage, [3])).toEqual({
			0: { instance_id: 'aaa' },
			7: { instance_id: 'ccc' }
		});
	});

	it('keeps everything when no slot was deleted', () => {
		expect(withoutStorageIndexes(storage, [])).toEqual(storage);
	});
});
