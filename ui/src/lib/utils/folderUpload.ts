import { zipSync } from 'fflate';

export interface ZipEntry {
	path: string;
	data: Uint8Array;
}

function isSaveFile(path: string): boolean {
	return path.toLowerCase().endsWith('.sav');
}

/** Files chosen through an `<input webkitdirectory>`; keeps `webkitRelativePath`
 * as the zip path (e.g. `world1/Level.sav`, which the loader tolerates). */
export async function readInputFolder(fileList: FileList): Promise<ZipEntry[]> {
	const entries: ZipEntry[] = [];
	for (const file of Array.from(fileList)) {
		const path = (file as File & { webkitRelativePath?: string }).webkitRelativePath || file.name;
		if (!isSaveFile(path)) continue;
		entries.push({ path, data: new Uint8Array(await file.arrayBuffer()) });
	}
	return entries;
}

interface FsEntry {
	isFile: boolean;
	isDirectory: boolean;
	name: string;
	file?: (onOk: (f: File) => void, onErr: (e: unknown) => void) => void;
	createReader?: () => {
		readEntries: (onOk: (batch: FsEntry[]) => void, onErr: (e: unknown) => void) => void;
	};
}

/** Walks folders dropped onto a drop target via the non-standard
 * `DataTransferItem.webkitGetAsEntry` API (Chrome/Edge/Firefox). */
export async function readDroppedItems(items: DataTransferItemList): Promise<ZipEntry[]> {
	const roots: FsEntry[] = [];
	for (const item of Array.from(items)) {
		const entry = (
			item as DataTransferItem & { webkitGetAsEntry?: () => FsEntry | null }
		).webkitGetAsEntry?.();
		if (entry) roots.push(entry);
	}

	const out: ZipEntry[] = [];
	async function walk(entry: FsEntry, prefix: string): Promise<void> {
		if (entry.isFile && entry.file) {
			const file = await new Promise<File>((res, rej) => entry.file!(res, rej));
			const path = `${prefix}${entry.name}`;
			if (isSaveFile(path)) out.push({ path, data: new Uint8Array(await file.arrayBuffer()) });
		} else if (entry.isDirectory && entry.createReader) {
			const reader = entry.createReader();
			// readEntries yields in batches; keep reading until an empty batch.
			let batch: FsEntry[];
			do {
				batch = await new Promise<FsEntry[]>((res, rej) => reader.readEntries(res, rej));
				for (const child of batch) await walk(child, `${prefix}${entry.name}/`);
			} while (batch.length > 0);
		}
	}
	for (const root of roots) await walk(root, '');
	return out;
}

/** Zips collected save files into a single archive for the `load_zip_file` path. */
export function zipEntries(entries: ZipEntry[]): Uint8Array {
	const files: Record<string, Uint8Array> = {};
	for (const e of entries) files[e.path] = e.data;
	return zipSync(files);
}

/** True when the collected files include a `Level.sav` (the minimum for a load). */
export function hasLevelSav(entries: ZipEntry[]): boolean {
	return entries.some((e) => e.path.toLowerCase().endsWith('level.sav'));
}

/** True when a drop contains at least one directory entry (a world folder),
 *  vs. only files (e.g. a .zip). Uses the non-standard webkitGetAsEntry API. */
export function hasDirectoryEntry(items: DataTransferItemList): boolean {
	for (const item of Array.from(items)) {
		const entry = (
			item as DataTransferItem & { webkitGetAsEntry?: () => { isDirectory: boolean } | null }
		).webkitGetAsEntry?.();
		if (entry?.isDirectory) return true;
	}
	return false;
}
