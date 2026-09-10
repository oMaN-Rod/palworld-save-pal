import { describe, it, expect, beforeEach, afterEach, vi } from 'vitest';
import 'fake-indexeddb/auto';
import { putRecent, listRecent, getMostRecent, removeRecent, type RecentSave } from './recentSaves';

function rec(id: string, savedAt: number): RecentSave {
	return { id, kind: 'opfs', opfsPath: `${id}.zip`, worldName: id, sizeBytes: 10, savedAt };
}

afterEach(() => {
	vi.unstubAllGlobals();
});

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

	it('adopts records from the legacy database, keeping newer copies', async () => {
		await putRecent({ ...rec('kept', 500), worldName: 'current' });
		await new Promise<void>((resolve, reject) => {
			const req = indexedDB.open('psp-recent-saves', 1);
			req.onupgradeneeded = () => req.result.createObjectStore('saves', { keyPath: 'id' });
			req.onsuccess = () => {
				const t = req.result.transaction('saves', 'readwrite');
				t.objectStore('saves').put(rec('old', 100));
				t.objectStore('saves').put({ ...rec('kept', 50), worldName: 'legacy' });
				t.oncomplete = () => {
					req.result.close();
					resolve();
				};
			};
			req.onerror = () => reject(req.error);
		});
		vi.resetModules();
		const fresh = await import('./recentSaves');

		const all = await fresh.listRecent();

		expect(all.map((r) => [r.id, r.worldName])).toEqual([
			['kept', 'current'],
			['old', 'old']
		]);
		const names = (await indexedDB.databases()).map((d) => d.name);
		expect(names).not.toContain('psp-recent-saves');
	});
});

describe('recentSaves when IndexedDB is unavailable', () => {
	it('resolves to an empty list instead of rejecting', async () => {
		vi.stubGlobal('indexedDB', {
			open: () => {
				throw new DOMException('blocked', 'SecurityError');
			}
		});
		await expect(listRecent()).resolves.toEqual([]);
	});

	it('resolves getMostRecent to null instead of rejecting', async () => {
		vi.stubGlobal('indexedDB', {
			open: () => {
				throw new DOMException('blocked', 'SecurityError');
			}
		});
		await expect(getMostRecent()).resolves.toBeNull();
	});
});
