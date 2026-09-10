const DIR = 'ps-saves';
const LEGACY_DIR = 'psp-saves';

export class QuotaError extends Error {
	constructor() {
		super('OPFS quota exceeded');
		this.name = 'QuotaError';
	}
}

let legacyAdopted: Promise<void> | undefined;

/** The legacy dir is only removed once every entry has been copied across. */
async function adoptLegacyDir(root: FileSystemDirectoryHandle): Promise<void> {
	let legacy: FileSystemDirectoryHandle;
	try {
		legacy = await root.getDirectoryHandle(LEGACY_DIR);
	} catch {
		return;
	}
	const current = await root.getDirectoryHandle(DIR, { create: true });
	for await (const [name, handle] of (legacy as unknown as {
		entries(): AsyncIterableIterator<[string, FileSystemHandle]>;
	}).entries()) {
		if (handle.kind !== 'file') continue;
		const exists = await current.getFileHandle(name).then(
			() => true,
			() => false
		);
		if (exists) continue;
		const file = await (handle as FileSystemFileHandle).getFile();
		const writable = await (await current.getFileHandle(name, { create: true })).createWritable();
		await writable.write(new Uint8Array(await file.arrayBuffer()));
		await writable.close();
	}
	await root.removeEntry(LEGACY_DIR, { recursive: true });
}

async function dir(): Promise<FileSystemDirectoryHandle> {
	const root = await navigator.storage.getDirectory();
	legacyAdopted ??= adoptLegacyDir(root).catch(() => {});
	await legacyAdopted;
	return root.getDirectoryHandle(DIR, { create: true });
}

function isQuota(e: unknown): boolean {
	return e instanceof DOMException && (e.name === 'QuotaExceededError' || e.name === 'NS_ERROR_DOM_QUOTA_REACHED');
}

export async function putBlob(path: string, bytes: Uint8Array): Promise<void> {
	const d = await dir();
	const fh = await d.getFileHandle(path, { create: true });
	try {
		const w = await fh.createWritable();
		await w.write(bytes as BufferSource);
		await w.close();
	} catch (e) {
		await d.removeEntry(path).catch(() => {});
		if (isQuota(e)) throw new QuotaError();
		throw e;
	}
}

export async function getBlob(path: string): Promise<Uint8Array | null> {
	try {
		const d = await dir();
		const fh = await d.getFileHandle(path, { create: false });
		const file = await fh.getFile();
		return new Uint8Array(await file.arrayBuffer());
	} catch {
		return null;
	}
}

export async function deleteBlob(path: string): Promise<void> {
	const d = await dir();
	await d.removeEntry(path).catch(() => {});
}
