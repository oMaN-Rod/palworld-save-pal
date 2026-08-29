import { describe, it, expect, beforeEach, vi } from 'vitest';

const recent = vi.hoisted(() => ({ store: [] as any[] }));
vi.mock('./recentSaves', () => ({
	putRecent: vi.fn(async (r) => {
		recent.store = recent.store.filter((x) => x.id !== r.id);
		recent.store.push(r);
	}),
	getMostRecent: vi.fn(async () => [...recent.store].sort((a, b) => b.savedAt - a.savedAt)[0] ?? null),
	removeRecent: vi.fn(async (id) => {
		recent.store = recent.store.filter((x) => x.id !== id);
	})
}));
const blobs = vi.hoisted(() => ({
	map: new Map<string, Uint8Array>(),
	failMode: 'none' as 'none' | 'quota' | 'unavailable'
}));
vi.mock('./opfsBlobStore', () => {
	class QuotaError extends Error {}
	return {
		QuotaError,
		putBlob: vi.fn(async (p: string, b: Uint8Array) => {
			if (blobs.failMode === 'quota') throw new QuotaError();
			// Simulates private mode / embedded webview where OPFS throws a non-quota error.
			if (blobs.failMode === 'unavailable') throw new DOMException('no OPFS', 'SecurityError');
			blobs.map.set(p, b);
		}),
		getBlob: vi.fn(async (p: string) => blobs.map.get(p) ?? null),
		deleteBlob: vi.fn(async (p: string) => void blobs.map.delete(p))
	};
});
vi.mock('$lib/utils/folderUpload', () => ({
	zipEntries: (entries: { path: string; data: Uint8Array }[]) =>
		new Uint8Array([entries.length])
}));
vi.mock('./fileSystemAccess', () => ({
	readSaveFolder: vi.fn(async () => [{ path: 'Level.sav', data: new Uint8Array([1]) }]),
	ensureReadWrite: vi.fn(async () => true)
}));
const active = vi.hoisted(() => ({ set: vi.fn() }));
vi.mock('./activeSave', () => ({ setActiveDirectory: active.set }));

import { recordSession, restoreMostRecent } from './sessionPersistence';

beforeEach(() => {
	recent.store = [];
	blobs.map.clear();
	blobs.failMode = 'none';
	active.set.mockClear();
});

describe('sessionPersistence', () => {
	it('records an opfs session and persists the bytes', async () => {
		const res = await recordSession({ zipBytes: new Uint8Array([5, 6]), name: 'world1', savedAt: 1 });
		expect(res).toEqual({ persisted: true, quota: false });
		const restored: number[] = [];
		const r = await restoreMostRecent((b) => restored.push(...b));
		expect(r.restored).toBe(true);
		expect(restored).toEqual([5, 6]);
	});

	it('records a handle session and re-reads from disk on restore', async () => {
		const handle = { name: 'world1' } as unknown as FileSystemDirectoryHandle;
		const res = await recordSession({ zipBytes: new Uint8Array([9]), name: 'world1', savedAt: 2, handle, writable: true });
		expect(res.persisted).toBe(true);
		expect(active.set).toHaveBeenCalledWith(handle, true);
		const restored: number[] = [];
		const r = await restoreMostRecent((b) => restored.push(...b));
		expect(r.restored).toBe(true);
		expect(restored).toEqual([1]);
	});

	it('returns quota:false persisted:false and no throw when OPFS is full', async () => {
		blobs.failMode = 'quota';
		const res = await recordSession({ zipBytes: new Uint8Array([1]), name: 'big', savedAt: 3 });
		expect(res).toEqual({ persisted: false, quota: true });
	});

	it('restoreMostRecent returns restored:false when nothing is stored', async () => {
		expect(await restoreMostRecent(() => {})).toEqual({ restored: false, needsPermission: false });
	});
});

describe('recordSession when storage is unavailable', () => {
	it('reports not-persisted instead of throwing when OPFS is missing', async () => {
		// putBlob is mocked at the module boundary in this file, so we drive the
		// same failure through the mock rather than stubbing navigator.storage
		// (which the mocked opfsBlobStore never touches).
		blobs.failMode = 'unavailable';
		const res = await recordSession({
			zipBytes: new Uint8Array([1, 2, 3]),
			name: 'world',
			savedAt: 1
		});
		expect(res).toEqual({ persisted: false, quota: false });
	});

	it('still reports quota separately so the caller can warn about size', async () => {
		blobs.failMode = 'quota';
		const res = await recordSession({
			zipBytes: new Uint8Array([1, 2, 3]),
			name: 'world',
			savedAt: 1
		});
		expect(res.persisted).toBe(false);
		expect(res.quota).toBe(true);
	});
});
