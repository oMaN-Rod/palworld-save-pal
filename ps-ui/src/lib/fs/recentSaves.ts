export interface RecentSave {
	id: string;
	kind: 'handle' | 'opfs';
	handle?: FileSystemDirectoryHandle;
	opfsPath?: string;
	worldName: string;
	sizeBytes: number;
	savedAt: number;
}

const DB = 'ps-recent-saves';
const LEGACY_DB = 'psp-recent-saves';
const STORE = 'saves';

function openCurrent(): Promise<IDBDatabase> {
	return new Promise((resolve, reject) => {
		const req = indexedDB.open(DB, 1);
		req.onupgradeneeded = () => {
			req.result.createObjectStore(STORE, { keyPath: 'id' });
		};
		req.onsuccess = () => resolve(req.result);
		req.onerror = () => reject(req.error);
	});
}

/** Resolves to null when the legacy database does not exist; aborting the
 *  upgrade keeps the probe from creating it. */
function readLegacy(): Promise<RecentSave[] | null> {
	return new Promise((resolve) => {
		const req = indexedDB.open(LEGACY_DB);
		req.onupgradeneeded = () => req.transaction?.abort();
		req.onerror = () => resolve(null);
		req.onsuccess = () => {
			const db = req.result;
			if (!db.objectStoreNames.contains(STORE)) {
				db.close();
				resolve(null);
				return;
			}
			const all = db.transaction(STORE, 'readonly').objectStore(STORE).getAll();
			all.onsuccess = () => {
				db.close();
				resolve(all.result as RecentSave[]);
			};
			all.onerror = () => {
				db.close();
				resolve(null);
			};
		};
	});
}

async function adoptLegacy(): Promise<void> {
	const records = await readLegacy();
	if (!records) return;
	const db = await openCurrent();
	await new Promise<void>((resolve, reject) => {
		const t = db.transaction(STORE, 'readwrite');
		const store = t.objectStore(STORE);
		for (const rec of records) store.add(rec).onerror = (e) => e.preventDefault();
		t.oncomplete = () => resolve();
		t.onabort = () => reject(t.error);
	}).finally(() => db.close());
	await new Promise<void>((resolve) => {
		const req = indexedDB.deleteDatabase(LEGACY_DB);
		req.onsuccess = req.onerror = req.onblocked = () => resolve();
	});
}

let legacyAdopted: Promise<void> | undefined;

function open(): Promise<IDBDatabase> {
	legacyAdopted ??= adoptLegacy().catch(() => {});
	return legacyAdopted.then(openCurrent);
}

function tx<T>(mode: IDBTransactionMode, run: (s: IDBObjectStore) => IDBRequest<T>): Promise<T> {
	return new Promise((resolve, reject) => {
		open().then((db) => {
			const t = db.transaction(STORE, mode);
			const req = run(t.objectStore(STORE));
			req.onsuccess = () => resolve(req.result);
			req.onerror = () => reject(req.error);
			const settle = () => db.close();
			t.oncomplete = settle;
			t.onabort = settle;
			t.onerror = settle;
		}, reject);
	});
}

export async function putRecent(rec: RecentSave): Promise<void> {
	await tx('readwrite', (s) => s.put(rec));
}

export async function listRecent(): Promise<RecentSave[]> {
	// IndexedDB is absent or blocked in some private windows; the recents list
	// is a convenience, so degrade to empty rather than rejecting into callers.
	try {
		const all = (await tx<RecentSave[]>('readonly', (s) => s.getAll())) ?? [];
		return all.sort((a, b) => b.savedAt - a.savedAt);
	} catch {
		return [];
	}
}

export async function getMostRecent(): Promise<RecentSave | null> {
	return (await listRecent())[0] ?? null;
}

export async function removeRecent(id: string): Promise<void> {
	try {
		await tx('readwrite', (s) => s.delete(id));
	} catch {
	}
}
