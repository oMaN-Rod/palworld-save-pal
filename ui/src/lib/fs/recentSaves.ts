export interface RecentSave {
	id: string;
	kind: 'handle' | 'opfs';
	handle?: FileSystemDirectoryHandle;
	opfsPath?: string;
	worldName: string;
	sizeBytes: number;
	savedAt: number;
}

const DB = 'psp-recent-saves';
const STORE = 'saves';

function open(): Promise<IDBDatabase> {
	return new Promise((resolve, reject) => {
		const req = indexedDB.open(DB, 1);
		req.onupgradeneeded = () => {
			req.result.createObjectStore(STORE, { keyPath: 'id' });
		};
		req.onsuccess = () => resolve(req.result);
		req.onerror = () => reject(req.error);
	});
}

function tx<T>(mode: IDBTransactionMode, run: (s: IDBObjectStore) => IDBRequest<T>): Promise<T> {
	return new Promise((resolve, reject) => {
		open().then((db) => {
			const t = db.transaction(STORE, mode);
			const req = run(t.objectStore(STORE));
			req.onsuccess = () => resolve(req.result);
			req.onerror = () => reject(req.error);
			t.oncomplete = () => db.close();
		}, reject);
	});
}

export async function putRecent(rec: RecentSave): Promise<void> {
	await tx('readwrite', (s) => s.put(rec));
}

export async function listRecent(): Promise<RecentSave[]> {
	const all = (await tx<RecentSave[]>('readonly', (s) => s.getAll())) ?? [];
	return all.sort((a, b) => b.savedAt - a.savedAt);
}

export async function getMostRecent(): Promise<RecentSave | null> {
	return (await listRecent())[0] ?? null;
}

export async function removeRecent(id: string): Promise<void> {
	await tx('readwrite', (s) => s.delete(id));
}
