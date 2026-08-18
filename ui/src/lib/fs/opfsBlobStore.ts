const DIR = 'psp-saves';

export class QuotaError extends Error {
	constructor() {
		super('OPFS quota exceeded');
		this.name = 'QuotaError';
	}
}

async function dir(): Promise<FileSystemDirectoryHandle> {
	const root = await navigator.storage.getDirectory();
	return root.getDirectoryHandle(DIR, { create: true });
}

function isQuota(e: unknown): boolean {
	return (
		e instanceof DOMException &&
		(e.name === 'QuotaExceededError' || e.name === 'NS_ERROR_DOM_QUOTA_REACHED')
	);
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
