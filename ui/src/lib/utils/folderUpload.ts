import { zipSync } from 'fflate';
import { saveRoot, underSaveRoot } from './saveRoot';

export interface ZipEntry {
	path: string;
	data: Uint8Array;
}

function isSaveFile(path: string): boolean {
	return path.toLowerCase().endsWith('.sav');
}

/**
 * Bytes are read only for the files the save itself owns — a backup tree next
 * to it can be as large as the save again, and none of it is ever loaded.
 */
async function readOwnFiles(found: { path: string; file: File }[]): Promise<ZipEntry[]> {
	const root = saveRoot(found.map((f) => f.path));
	const own = root === null ? found : found.filter((f) => underSaveRoot(f.path, root));
	const entries: ZipEntry[] = [];
	for (const { path, file } of own) {
		entries.push({ path, data: new Uint8Array(await file.arrayBuffer()) });
	}
	return entries;
}

export async function readInputFolder(fileList: FileList): Promise<ZipEntry[]> {
	const found = Array.from(fileList)
		.map((file) => ({
			path: (file as File & { webkitRelativePath?: string }).webkitRelativePath || file.name,
			file
		}))
		.filter(({ path }) => isSaveFile(path));
	return readOwnFiles(found);
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

export async function readDroppedItems(items: DataTransferItemList): Promise<ZipEntry[]> {
	const roots: FsEntry[] = [];
	for (const item of Array.from(items)) {
		const entry = (
			item as DataTransferItem & { webkitGetAsEntry?: () => FsEntry | null }
		).webkitGetAsEntry?.();
		if (entry) roots.push(entry);
	}

	const found: { path: string; file: File }[] = [];
	async function walk(entry: FsEntry, prefix: string): Promise<void> {
		if (entry.isFile && entry.file) {
			const path = `${prefix}${entry.name}`;
			if (!isSaveFile(path)) return;
			found.push({ path, file: await new Promise<File>((res, rej) => entry.file!(res, rej)) });
		} else if (entry.isDirectory && entry.createReader) {
			const reader = entry.createReader();
			let batch: FsEntry[];
			do {
				batch = await new Promise<FsEntry[]>((res, rej) => reader.readEntries(res, rej));
				for (const child of batch) await walk(child, `${prefix}${entry.name}/`);
			} while (batch.length > 0);
		}
	}
	for (const root of roots) await walk(root, '');
	return readOwnFiles(found);
}

export function zipEntries(entries: ZipEntry[]): Uint8Array {
	const files: Record<string, Uint8Array> = {};
	for (const e of entries) files[e.path] = e.data;
	return zipSync(files);
}

export function hasLevelSav(entries: ZipEntry[]): boolean {
	return entries.some((e) => e.path.toLowerCase().endsWith('level.sav'));
}

export function hasDirectoryEntry(items: DataTransferItemList): boolean {
	for (const item of Array.from(items)) {
		const entry = (
			item as DataTransferItem & { webkitGetAsEntry?: () => { isDirectory: boolean } | null }
		).webkitGetAsEntry?.();
		if (entry?.isDirectory) return true;
	}
	return false;
}
