import { beforeEach, describe, expect, it } from 'vitest';
import {
	clearActiveDirectory,
	getActiveDirectory,
	setActiveDirectory,
	setSaveTarget,
	takeSaveTarget
} from './activeSave';

describe('activeSave', () => {
	beforeEach(() => clearActiveDirectory());

	it('defaults to no directory and download target', () => {
		expect(getActiveDirectory()).toEqual({ handle: null, writable: false });
		expect(takeSaveTarget()).toBe('download');
	});

	it('stores an active directory', () => {
		const h = {} as FileSystemDirectoryHandle;
		setActiveDirectory(h, true);
		expect(getActiveDirectory()).toEqual({ handle: h, writable: true });
	});

	it('takeSaveTarget returns then resets to download', () => {
		setSaveTarget('folder');
		expect(takeSaveTarget()).toBe('folder');
		expect(takeSaveTarget()).toBe('download');
	});

	it('clearActiveDirectory resets handle and target', () => {
		setActiveDirectory({} as FileSystemDirectoryHandle, true);
		setSaveTarget('folder');
		clearActiveDirectory();
		expect(getActiveDirectory().handle).toBeNull();
		expect(takeSaveTarget()).toBe('download');
	});
});
