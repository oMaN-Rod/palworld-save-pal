import { describe, it, expect, beforeEach, vi } from 'vitest';
import { zipSync } from 'fflate';

const active = vi.hoisted(() => ({
	target: 'download' as 'download' | 'folder',
	dir: { handle: null as unknown, writable: false }
}));
vi.mock('$lib/fs', () => ({
	takeSaveTarget: () => {
		const t = active.target;
		active.target = 'download';
		return t;
	},
	getActiveDirectory: () => active.dir,
	writeSaveInPlace: vi.fn(async () => '.psp-backup/1000')
}));

import { writeSaveInPlace } from '$lib/fs';
import { handleSaveOutput } from './saveFileHandler';

function zipOf(files: Record<string, number[]>): string {
	const zip = zipSync(Object.fromEntries(Object.entries(files).map(([k, v]) => [k, new Uint8Array(v)])));
	let s = '';
	for (const b of zip) s += String.fromCharCode(b);
	return btoa(s);
}

describe('handleSaveOutput (save-in-place branch)', () => {
	beforeEach(() => {
		active.target = 'download';
		active.dir = { handle: null, writable: false };
		vi.mocked(writeSaveInPlace).mockClear();
	});

	it('writes to the folder when target=folder and a writable handle exists', async () => {
		active.target = 'folder';
		active.dir = { handle: {}, writable: true };
		const content = zipOf({ 'Level.sav': [1, 2], 'Players/x.sav': [3] });
		const downloaded: string[] = [];
		const result = await handleSaveOutput([{ name: 'world1.zip', content }], (n) => downloaded.push(n), 1000);
		expect(result).toBe('folder');
		expect(downloaded).toEqual([]);
		expect(writeSaveInPlace).toHaveBeenCalledTimes(1);
		const files = vi.mocked(writeSaveInPlace).mock.calls[0][1] as { path: string; bytes: Uint8Array }[];
		expect(files.map((f) => f.path).sort()).toEqual(['Level.sav', 'Players/x.sav']);
	});

	it('falls back to download when no writable handle', async () => {
		active.target = 'folder';
		active.dir = { handle: null, writable: false };
		const content = zipOf({ 'Level.sav': [1] });
		const downloaded: string[] = [];
		const result = await handleSaveOutput([{ name: 'world1.zip', content }], (n) => downloaded.push(n), 1000);
		expect(result).toBe('download');
		expect(downloaded).toEqual(['world1.zip']);
		expect(writeSaveInPlace).not.toHaveBeenCalled();
	});
});
