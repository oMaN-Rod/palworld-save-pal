import { describe, expect, it } from 'vitest';
import { saveRoot, underSaveRoot } from './saveRoot';

describe('saveRoot', () => {
	it('is null without a Level.sav', () => {
		expect(saveRoot(['world1/Players/abc.sav'])).toBeNull();
	});

	it('is the empty prefix when Level.sav sits at the top', () => {
		expect(saveRoot(['Level.sav', 'Players/abc.sav'])).toBe('');
	});

	it('prefers the shallowest Level.sav over a nested backup copy', () => {
		expect(
			saveRoot([
				'world1/backups/2026-08-01/Level.sav',
				'world1/Level.sav',
				'world1/.psp-backup/1700000000/Level.sav'
			])
		).toBe('world1/');
	});

	it('picks the shallowest whatever the listing order', () => {
		expect(saveRoot(['a/b/Level.sav', 'a/Level.sav'])).toBe('a/');
		expect(saveRoot(['a/Level.sav', 'a/b/Level.sav'])).toBe('a/');
	});
});

describe('underSaveRoot', () => {
	const root = 'world1/';

	it("accepts the save's own top-level files and its Players folder", () => {
		expect(underSaveRoot('world1/Level.sav', root)).toBe(true);
		expect(underSaveRoot('world1/LevelMeta.sav', root)).toBe(true);
		expect(underSaveRoot('world1/Players/abc.sav', root)).toBe(true);
	});

	it('rejects anything from a nested copy of the save', () => {
		expect(underSaveRoot('world1/backups/2026-08-01/Level.sav', root)).toBe(false);
		expect(underSaveRoot('world1/backups/2026-08-01/Players/abc.sav', root)).toBe(false);
		expect(underSaveRoot('world1/.psp-backup/17/Players/abc.sav', root)).toBe(false);
		expect(underSaveRoot('other/Level.sav', root)).toBe(false);
	});
});
