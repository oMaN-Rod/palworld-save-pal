import type { ZipEntry } from '$lib/utils/folderUpload';

type Perm = 'granted' | 'denied' | 'prompt';
interface PermHandle {
	queryPermission?(d: { mode: 'read' | 'readwrite' }): Promise<Perm>;
	requestPermission?(d: { mode: 'read' | 'readwrite' }): Promise<Perm>;
}

export function fsaSupported(): boolean {
	return typeof (globalThis as { showDirectoryPicker?: unknown }).showDirectoryPicker === 'function';
}

export async function pickSaveDirectory(): Promise<FileSystemDirectoryHandle | null> {
	try {
		return await (globalThis as unknown as {
			showDirectoryPicker(o?: { mode?: 'read' | 'readwrite' }): Promise<FileSystemDirectoryHandle>;
		}).showDirectoryPicker({ mode: 'readwrite' });
	} catch {
		// AbortError on cancel — treat as no selection.
		return null;
	}
}

async function collect(
	dir: FileSystemDirectoryHandle,
	prefix: string,
	out: ZipEntry[]
): Promise<void> {
	for await (const [name, handle] of (dir as unknown as {
		entries(): AsyncIterableIterator<[string, FileSystemHandle]>;
	}).entries()) {
		const path = prefix ? `${prefix}/${name}` : name;
		if (handle.kind === 'file') {
			if (!name.endsWith('.sav')) continue;
			const file = await (handle as FileSystemFileHandle).getFile();
			out.push({ path, data: new Uint8Array(await file.arrayBuffer()) });
		} else if (name === 'Players') {
			await collect(handle as FileSystemDirectoryHandle, path, out);
		}
	}
}

export async function readSaveFolder(dir: FileSystemDirectoryHandle): Promise<ZipEntry[]> {
	const out: ZipEntry[] = [];
	await collect(dir, '', out);
	if (!out.some((e) => e.path.endsWith('Level.sav'))) {
		throw new Error("That folder has no Level.sav — choose the world save folder itself.");
	}
	return out;
}

export async function ensureReadWrite(dir: FileSystemDirectoryHandle): Promise<boolean> {
	const h = dir as unknown as PermHandle;
	if (!h.queryPermission) return true;
	if ((await h.queryPermission({ mode: 'readwrite' })) === 'granted') return true;
	return (await h.requestPermission?.({ mode: 'readwrite' })) === 'granted';
}

async function findCaseInsensitive(
	dir: FileSystemDirectoryHandle,
	name: string
): Promise<FileSystemHandle | null> {
	for await (const [entryName, handle] of (dir as unknown as {
		entries(): AsyncIterableIterator<[string, FileSystemHandle]>;
	}).entries()) {
		if (entryName.toLowerCase() === name.toLowerCase()) return handle;
	}
	return null;
}

async function fileHandleFor(
	dir: FileSystemDirectoryHandle,
	path: string,
	create: boolean
): Promise<FileSystemFileHandle> {
	const segs = path.split('/');
	let cur = dir;
	for (let i = 0; i < segs.length - 1; i++) {
		const existing = await findCaseInsensitive(cur, segs[i]);
		cur =
			existing?.kind === 'directory'
				? (existing as FileSystemDirectoryHandle)
				: await cur.getDirectoryHandle(segs[i], { create });
	}
	const last = segs[segs.length - 1];
	const existingFile = await findCaseInsensitive(cur, last);
	if (existingFile?.kind === 'file') return existingFile as FileSystemFileHandle;
	return cur.getFileHandle(last, { create });
}

async function readIfExists(dir: FileSystemDirectoryHandle, path: string): Promise<Uint8Array | null> {
	try {
		const fh = await fileHandleFor(dir, path, false);
		const file = await fh.getFile();
		return new Uint8Array(await file.arrayBuffer());
	} catch {
		return null;
	}
}

export async function writeSaveInPlace(
	dir: FileSystemDirectoryHandle,
	files: { path: string; bytes: Uint8Array }[],
	timestamp: number
): Promise<string> {
	const backup = `.psp-backup/${timestamp}`;
	// Back up every original that exists BEFORE any overwrite.
	for (const f of files) {
		const original = await readIfExists(dir, f.path);
		if (original) {
			const bh = await fileHandleFor(dir, `${backup}/${f.path}`, true);
			const w = await bh.createWritable();
			await w.write(original as BufferSource);
			await w.close();
		}
	}
	// Then write the new bytes in place.
	for (const f of files) {
		const fh = await fileHandleFor(dir, f.path, true);
		const w = await fh.createWritable();
		await w.write(f.bytes as BufferSource);
		await w.close();
	}
	return backup;
}
