import type { ModProfile, ProfileMod } from '$types';
import { describe, expect, it } from 'vitest';
import { libraryMod } from './__tests__/fixtures';
import { alphabeticalRows, libraryCovers, moveId, orderRows } from './loadOrder';

function entry(mod_id: string, load_order: number): ProfileMod {
	return { profile_id: 'p', mod_id, mod_version_id: null, enabled: true, load_order };
}

function profile(mods: ProfileMod[]): ModProfile {
	return {
		id: 'p',
		target_id: 't',
		name: 'Default',
		is_active: true,
		is_default: true,
		mods,
		ue4ss_control_mode: 'enabled_txt',
		force_order_ue4ss: false,
		force_order_palschema: false,
		created_at: '',
		updated_at: ''
	};
}

const library = [
	libraryMod({ id: 'lua-b', name: 'Beta', mod_type: 'ue4ss' }),
	libraryMod({ id: 'hyb', name: 'Hybrid', mod_type: 'hybrid' }),
	libraryMod({ id: 'lua-a', name: 'Alpha', mod_type: 'ue4ss' }),
	libraryMod({ id: 'schema', name: 'Schema', mod_type: 'palschema' }),
	libraryMod({ id: 'pak-z', name: 'Zulu', mod_type: 'pak' }),
	libraryMod({ id: 'logic-a', name: 'able', mod_type: 'logicmods' })
];

describe('orderRows', () => {
	it('lists UE4SS and hybrid mods by load order, then mod id', () => {
		const rows = orderRows(
			profile([entry('lua-a', 2), entry('schema', 0), entry('hyb', 1), entry('lua-b', 1)]),
			library,
			'ue4ss'
		);
		expect(rows.map((row) => row.entry.mod_id)).toEqual(['hyb', 'lua-b', 'lua-a']);
	});

	it('lists only PalSchema mods for the palschema kind', () => {
		const rows = orderRows(profile([entry('lua-a', 0), entry('schema', 1)]), library, 'palschema');
		expect(rows.map((row) => row.entry.mod_id)).toEqual(['schema']);
	});
});

describe('alphabeticalRows', () => {
	it('sorts pak and logic mods by display name, ignoring case', () => {
		const rows = alphabeticalRows(
			profile([entry('pak-z', 0), entry('logic-a', 1), entry('lua-a', 2)]),
			library,
			['pak', 'logicmods']
		);
		expect(rows.map((row) => row.mod.id)).toEqual(['logic-a', 'pak-z']);
	});
});

describe('moveId', () => {
	it('swaps a mod with its neighbour', () => {
		expect(moveId(['a', 'b', 'c'], 1, -1)).toEqual(['b', 'a', 'c']);
		expect(moveId(['a', 'b', 'c'], 1, 1)).toEqual(['a', 'c', 'b']);
	});

	it('refuses to move past either end', () => {
		expect(moveId(['a', 'b'], 0, -1)).toBeNull();
		expect(moveId(['a', 'b'], 1, 1)).toBeNull();
	});
});

describe('libraryCovers', () => {
	it('is true only when every profile entry is in the library', () => {
		expect(libraryCovers(profile([entry('lua-a', 0)]), library)).toBe(true);
		expect(libraryCovers(profile([entry('gone', 0)]), library)).toBe(false);
	});
});
