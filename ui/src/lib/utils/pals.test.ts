import { describe, expect, it, vi } from 'vitest';

vi.mock('$lib/data', () => ({
	expData: { getExpDataByLevel: vi.fn() },
	palsData: { getByKey: vi.fn() }
}));
vi.mock('$lib/utils', () => ({ getStats: vi.fn() }));
vi.mock('$states', () => ({ getAppState: vi.fn() }));

import { expData, palsData } from '$lib/data';
import { getAppState } from '$states';
import { EntryState } from '$types';
import { editAwakened, editImported, handleMaxOutPal } from './pals';

function pal() {
	return {
		character_id: 'SheepBall',
		is_boss: false,
		is_lucky: false,
		is_awakened: false,
		is_imported: false,
		state: EntryState.NONE
	} as any;
}

describe('editAwakened', () => {
	it('flips the flag and marks the pal modified', () => {
		const target = pal();
		editAwakened(target);
		expect(target.is_awakened).toBe(true);
		expect(target.state).toBe(EntryState.MODIFIED);
		editAwakened(target);
		expect(target.is_awakened).toBe(false);
	});

	it('leaves boss, lucky, imported and character_id alone', () => {
		const target = pal();
		editAwakened(target);
		expect(target.is_boss).toBe(false);
		expect(target.is_lucky).toBe(false);
		expect(target.is_imported).toBe(false);
		expect(target.character_id).toBe('SheepBall');
	});
});

describe('editImported', () => {
	it('flips the flag and marks the pal modified', () => {
		const target = pal();
		editImported(target);
		expect(target.is_imported).toBe(true);
		expect(target.state).toBe(EntryState.MODIFIED);
		editImported(target);
		expect(target.is_imported).toBe(false);
	});

	it('leaves awakened, boss, lucky and character_id alone', () => {
		const target = pal();
		editImported(target);
		expect(target.is_awakened).toBe(false);
		expect(target.is_boss).toBe(false);
		expect(target.is_lucky).toBe(false);
		expect(target.character_id).toBe('SheepBall');
	});
});

describe('handleMaxOutPal', () => {
	it('sets awakened but leaves imported alone', async () => {
		vi.mocked(getAppState).mockReturnValue({
			settings: { cheat_mode: false }
		} as any);
		vi.mocked(expData.getExpDataByLevel).mockResolvedValue({
			PalTotalEXP: 1_000_000,
			PalNextEXP: 0
		} as any);
		vi.mocked(palsData.getByKey).mockReturnValue({
			max_full_stomach: 300,
			work_suitability: {}
		} as any);

		const target = pal();
		target.character_key = 'sheepball';
		target.work_suitability = {};

		await handleMaxOutPal(target, { level: 80 } as any);

		expect(target.is_awakened).toBe(true);
		expect(target.is_imported).toBe(false);
	});
});
