import { describe, expect, it, vi } from 'vitest';

vi.mock('$lib/data', () => ({
	expData: { getExpDataByLevel: vi.fn() },
	palsData: { getByKey: vi.fn() }
}));
vi.mock('$lib/utils', () => ({ getStats: vi.fn() }));
vi.mock('$states', () => ({ getAppState: vi.fn() }));

import { EntryState } from '$types';
import { editAwakened, editImported } from './pals';

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

	it('leaves boss, lucky and character_id alone', () => {
		const target = pal();
		editAwakened(target);
		expect(target.is_boss).toBe(false);
		expect(target.is_lucky).toBe(false);
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
});
