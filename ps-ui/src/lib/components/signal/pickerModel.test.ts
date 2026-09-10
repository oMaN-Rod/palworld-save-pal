import { describe, expect, it } from 'vitest';
import {
	type BrowseEntry,
	type BrowseFrame,
	type GamepassSaveEntry,
	type LocalSaveEntry,
	hasLevelSav,
	isServerRunning,
	levelSavPath,
	popFrame,
	popToIndex,
	pushFrame,
	replyError,
	sortByRecency,
	sortGamepassSaves,
	sortLocalSaves
} from './pickerModel';

describe('sortByRecency', () => {
	it('sorts newest first', () => {
		const items = [{ ts: 100 }, { ts: 300 }, { ts: 200 }];
		expect(sortByRecency(items, (item) => item.ts)).toEqual([{ ts: 300 }, { ts: 200 }, { ts: 100 }]);
	});

	it('sorts unknown (<= 0) timestamps after every known one, preserving their relative order', () => {
		const items = [
			{ id: 'a', ts: 0 },
			{ id: 'b', ts: 500 },
			{ id: 'c', ts: 0 },
			{ id: 'd', ts: 100 }
		];
		expect(sortByRecency(items, (item) => item.ts).map((item) => item.id)).toEqual([
			'b',
			'd',
			'a',
			'c'
		]);
	});
});

describe('sortLocalSaves', () => {
	it('sorts by modified_ms, newest first, with unknown (0) last', () => {
		const saves: LocalSaveEntry[] = [
			{ path: '/a/Level.sav', name: 'a', save_type: 'steam', modified_ms: 100 },
			{ path: '/b/Level.sav', name: 'b', save_type: 'steam', modified_ms: 0 },
			{ path: '/c/Level.sav', name: 'c', save_type: 'steam', modified_ms: 300 }
		];
		expect(sortLocalSaves(saves).map((save) => save.name)).toEqual(['c', 'a', 'b']);
	});
});

describe('sortGamepassSaves', () => {
	it('sorts by last_modified, newest first', () => {
		const saves: GamepassSaveEntry[] = [
			{ save_id: '1', world_name: 'old', player_count: 1, last_modified: 100 },
			{ save_id: '2', world_name: 'new', player_count: 2, last_modified: 500 }
		];
		expect(sortGamepassSaves(saves).map((save) => save.world_name)).toEqual(['new', 'old']);
	});
});

describe('hasLevelSav / levelSavPath', () => {
	const levelSavFile: BrowseEntry = { name: 'Level.sav', path: 'C:/save/Level.sav', is_dir: false };
	const levelSavDir: BrowseEntry = { name: 'Level.sav', path: 'C:/save/Level.sav', is_dir: true };
	const otherFile: BrowseEntry = { name: 'LevelMeta.sav', path: 'C:/save/LevelMeta.sav', is_dir: false };

	it('is true only when a non-directory Level.sav entry is present', () => {
		expect(hasLevelSav([levelSavFile, otherFile])).toBe(true);
		expect(hasLevelSav([otherFile])).toBe(false);
		expect(hasLevelSav([levelSavDir])).toBe(false);
		expect(hasLevelSav([])).toBe(false);
	});

	it('returns the Level.sav file path, or null when absent', () => {
		expect(levelSavPath([levelSavFile, otherFile])).toBe('C:/save/Level.sav');
		expect(levelSavPath([otherFile])).toBeNull();
		expect(levelSavPath([levelSavDir])).toBeNull();
	});
});

describe('browse frame stack', () => {
	const root: BrowseFrame = { path: '', entries: [] };
	const child: BrowseFrame = { path: 'C:/', entries: [] };
	const grandchild: BrowseFrame = { path: 'C:/Games', entries: [] };

	it('pushFrame appends a frame', () => {
		expect(pushFrame([root], child)).toEqual([root, child]);
	});

	it('popFrame removes the last frame but keeps the root', () => {
		expect(popFrame([root, child, grandchild])).toEqual([root, child]);
		expect(popFrame([root])).toEqual([root]);
	});

	it('popToIndex truncates to the given breadcrumb', () => {
		expect(popToIndex([root, child, grandchild], 0)).toEqual([root]);
		expect(popToIndex([root, child, grandchild], 1)).toEqual([root, child]);
	});

	it('popToIndex ignores an out-of-range index', () => {
		const stack = [root, child];
		expect(popToIndex(stack, 5)).toEqual(stack);
		expect(popToIndex(stack, -1)).toEqual(stack);
	});
});

describe('replyError', () => {
	it('extracts a string error field', () => {
		expect(replyError({ error: 'boom' })).toBe('boom');
	});

	it('returns null when there is no error field', () => {
		expect(replyError({ saves: [] })).toBeNull();
		expect(replyError({})).toBeNull();
	});

	it('ignores a non-string error field', () => {
		expect(replyError({ error: 123 })).toBeNull();
		expect(replyError({ error: null })).toBeNull();
	});
});

describe('isServerRunning', () => {
	it('is true only when status.running is true', () => {
		expect(isServerRunning({ status: { running: true } })).toBe(true);
		expect(isServerRunning({ status: { running: false } })).toBe(false);
	});

	it('is false when status is absent', () => {
		expect(isServerRunning({})).toBe(false);
	});
});
