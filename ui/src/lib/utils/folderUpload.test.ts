import { describe, expect, it } from 'vitest';
import { hasDirectoryEntry } from './folderUpload';

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
