import { describe, it, expect, beforeEach } from 'vitest';
import 'fake-indexeddb/auto';
import { putRecent, listRecent, getMostRecent, removeRecent, type RecentSave } from './recentSaves';

function rec(id: string, savedAt: number): RecentSave {
	return { id, kind: 'opfs', opfsPath: `${id}.zip`, worldName: id, sizeBytes: 10, savedAt };
}

describe('recentSaves', () => {
	beforeEach(async () => {
		for (const r of await listRecent()) await removeRecent(r.id);
	});

	it('stores and lists newest-first', async () => {
		await putRecent(rec('a', 100));
		await putRecent(rec('b', 300));
		await putRecent(rec('c', 200));
		expect((await listRecent()).map((r) => r.id)).toEqual(['b', 'c', 'a']);
	});

	it('getMostRecent returns the latest', async () => {
		await putRecent(rec('a', 100));
		await putRecent(rec('b', 300));
		expect((await getMostRecent())?.id).toBe('b');
	});

	it('putRecent with an existing id replaces it', async () => {
		await putRecent(rec('a', 100));
		await putRecent({ ...rec('a', 400), worldName: 'renamed' });
		const all = await listRecent();
		expect(all.length).toBe(1);
		expect(all[0].worldName).toBe('renamed');
	});

	it('removeRecent deletes', async () => {
		await putRecent(rec('a', 100));
		await removeRecent('a');
		expect(await getMostRecent()).toBeNull();
	});
});
