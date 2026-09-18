import { describe, expect, it } from 'vitest';
import { conflictErrorText, conflictSummary, refLabel, unreadableReasonText } from './conflictText';

const name = (id: string) => (id === 'mod-a' ? 'Alpha' : id === 'mod-b' ? 'Beta' : id);

describe('conflictSummary', () => {
	it('reads a framework dependency from mod_type', () => {
		expect(
			conflictSummary(
				{ kind: 'missing_dependency', mod_id: 'mod-a', dependency: 'UE4SS', source: 'mod_type' },
				name
			)
		).toBe('Alpha needs UE4SS, which is not installed.');
	});

	it('reads a Workshop package dependency from workshop_info', () => {
		expect(
			conflictSummary(
				{
					kind: 'missing_dependency',
					mod_id: 'mod-a',
					dependency: 'Some Package',
					source: 'workshop_info'
				},
				name
			)
		).toBe('Alpha needs the Workshop package Some Package, which is not enabled.');
	});

	it('names the winner and the asset count for a pak overlap', () => {
		const text = conflictSummary(
			{
				kind: 'pak_overlap',
				paks: [
					{ mod_id: 'mod-a', file: 'a.pak', source: 'library', path: null },
					{ mod_id: 'mod-b', file: 'b.pak', source: 'library', path: null }
				],
				winner: { mod_id: 'mod-b', file: 'b.pak', source: 'library', path: null },
				assets: ['/Game/x'],
				asset_count: 3
			},
			name
		);
		expect(text).toContain('Alpha, Beta');
		expect(text).toContain('Beta loads last and wins');
		expect(text).toContain('3 assets');
	});

	it('lists the mods editing a PalSchema row', () => {
		const text = conflictSummary(
			{
				kind: 'palschema_row',
				key: 'Pal.Cattiva',
				mods: [
					{ mod_id: 'mod-a', file: 'a.json', source: 'library', path: null },
					{ mod_id: 'mod-b', file: 'b.json', source: 'library', path: null }
				]
			},
			name
		);
		expect(text).toBe('Pal.Cattiva is edited by Alpha, Beta.');
	});

	it('counts the pak files a Game Pass conversion needs', () => {
		const text = conflictSummary(
			{ kind: 'gamepass_pak_incompatible', mod_id: 'mod-a', files: ['a.pak', 'b.pak'] },
			name
		);
		expect(text).toContain('Alpha');
		expect(text).toContain('2');
	});
});

describe('unreadableReasonText', () => {
	it('reads iostore as already converted', () => {
		expect(unreadableReasonText('iostore')).toBe(
			'Already converted for Game Pass; its contents are not checked'
		);
	});

	it('falls back to the io text for an unknown reason', () => {
		expect(unreadableReasonText('io')).toBe('The file could not be read');
		expect(unreadableReasonText('something_else')).toBe('The file could not be read');
	});
});

describe('conflictErrorText', () => {
	it('reads no_active_profile', () => {
		expect(conflictErrorText({ code: 'no_active_profile', message: 'raw' })).toBe(
			'This target has no active profile to check.'
		);
	});

	it('falls back to the message for an unknown code', () => {
		expect(conflictErrorText({ code: 'something_else', message: 'raw message' })).toBe(
			'raw message'
		);
	});
});

describe('untracked paks', () => {
	it('names an untracked pak by its file name', () => {
		expect(refLabel({ mod_id: null, file: 'Stray_P.pak' }, name)).toBe('Stray_P.pak (untracked)');
		expect(refLabel({ mod_id: 'mod-a', file: 'a.pak' }, name)).toBe('Alpha');
	});

	it('uses the file name for an untracked pak in an overlap', () => {
		const text = conflictSummary(
			{
				kind: 'pak_overlap',
				paks: [
					{ mod_id: 'mod-a', file: 'a.pak', source: 'library', path: null },
					{ mod_id: null, file: 'zzz_P.pak', source: 'disk', path: '~mods/zzz_P.pak' }
				],
				winner: { mod_id: null, file: 'zzz_P.pak', source: 'disk', path: '~mods/zzz_P.pak' },
				assets: ['x'],
				asset_count: 1
			},
			name
		);
		expect(text).toContain('Alpha, zzz_P.pak (untracked)');
		expect(text).toContain('zzz_P.pak (untracked) loads last and wins');
	});
});
