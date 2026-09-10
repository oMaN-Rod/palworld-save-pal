import { describe, expect, it } from 'vitest';
import { hasDirectoryEntry, readInputFolder } from './folderUpload';

function items(kinds: Array<'file' | 'dir' | null>): DataTransferItemList {
	return kinds.map((k) => ({
		webkitGetAsEntry: () => (k === null ? null : { isDirectory: k === 'dir', isFile: k === 'file' })
	})) as unknown as DataTransferItemList;
}

describe('hasDirectoryEntry', () => {
	it('is true when any dropped item is a directory', () => {
		expect(hasDirectoryEntry(items(['file', 'dir']))).toBe(true);
	});
	it('is false for files only', () => {
		expect(hasDirectoryEntry(items(['file', 'file']))).toBe(false);
	});
	it('is false when entries are unavailable', () => {
		expect(hasDirectoryEntry(items([null]))).toBe(false);
	});
});

function fileList(paths: string[]): FileList {
	return paths.map((path) => ({
		name: path.split('/').pop(),
		webkitRelativePath: path,
		arrayBuffer: async () => new Uint8Array([1]).buffer
	})) as unknown as FileList;
}

describe('readInputFolder', () => {
	it('keeps only the files the save itself owns', async () => {
		const entries = await readInputFolder(
			fileList([
				'world1/backups/2026-08-01/Level.sav',
				'world1/backups/2026-08-01/Players/abc.sav',
				'world1/Level.sav',
				'world1/LevelMeta.sav',
				'world1/Players/abc.sav',
				'world1/notes.txt'
			])
		);

		expect(entries.map((e) => e.path)).toEqual([
			'world1/Level.sav',
			'world1/LevelMeta.sav',
			'world1/Players/abc.sav'
		]);
	});

	it('falls through untouched when there is no Level.sav to anchor on', async () => {
		const entries = await readInputFolder(fileList(['stray/Players/abc.sav']));
		expect(entries.map((e) => e.path)).toEqual(['stray/Players/abc.sav']);
	});
});
