import type { ModProfile, ProfileMod } from '$types';
import { beforeEach, describe, expect, it, vi } from 'vitest';

const send = vi.fn();

vi.mock('$lib/utils/websocketUtils', () => ({
	send: (type: unknown, data?: unknown) => send(type, data),
	sendAndWait: vi.fn()
}));

import { ModsState } from './modsState.svelte';

function entry(mod_id: string, load_order: number, enabled = true): ProfileMod {
	return { profile_id: 't/default', mod_id, mod_version_id: null, enabled, load_order };
}

function profile(overrides: Partial<ModProfile> = {}): ModProfile {
	return {
		id: 't/default',
		target_id: 't',
		name: 'Default',
		is_active: true,
		is_default: true,
		mods: [],
		ue4ss_control_mode: 'enabled_txt',
		force_order_ue4ss: false,
		force_order_palschema: false,
		created_at: '',
		updated_at: '',
		worlds: [],
		...overrides
	};
}

const hard = (overrides: Partial<ModProfile> = {}) =>
	profile({ id: 't/hard', name: 'Hard', is_active: false, is_default: false, ...overrides });

beforeEach(() => send.mockReset());

describe('ModsState profile requests', () => {
	it('sends every profile request fire-and-forget with its wire payload', () => {
		const state = new ModsState();
		state.createProfile('t', 'Hard');
		state.createProfile('t', 'Copy', 't/default');
		state.renameProfile('t', 't/hard', 'Harder');
		state.deleteProfile('t', 't/hard');
		state.activateProfile('t', 't/hard');
		state.reorderProfile('t', 't/hard', 'ue4ss', ['b', 'a']);
		state.setProfileOptions('t', 't/hard', { force_order_palschema: true });
		state.setMod('t', 'a', true);
		state.setMod('t', 'a', false, 't/hard');
		state.setModVersion('t', 't/hard', 'a', true, 'a@1.0');
		state.setModVersion('t', 't/hard', 'a', false, null);

		expect(send.mock.calls).toEqual([
			['profile_create', { target_id: 't', name: 'Hard' }],
			['profile_create', { target_id: 't', name: 'Copy', copy_from: 't/default' }],
			['profile_rename', { target_id: 't', profile_id: 't/hard', name: 'Harder' }],
			['profile_delete', { target_id: 't', profile_id: 't/hard' }],
			['profile_activate', { target_id: 't', profile_id: 't/hard' }],
			[
				'profile_reorder',
				{ target_id: 't', profile_id: 't/hard', kind: 'ue4ss', ordered_mod_ids: ['b', 'a'] }
			],
			[
				'profile_set_options',
				{ target_id: 't', profile_id: 't/hard', force_order_palschema: true }
			],
			['profile_set_mod', { target_id: 't', mod_id: 'a', enabled: true }],
			['profile_set_mod', { target_id: 't', profile_id: 't/hard', mod_id: 'a', enabled: false }],
			[
				'profile_set_mod',
				{
					target_id: 't',
					profile_id: 't/hard',
					mod_id: 'a',
					enabled: true,
					mod_version_id: 'a@1.0'
				}
			],
			[
				'profile_set_mod',
				{ target_id: 't', profile_id: 't/hard', mod_id: 'a', enabled: false, mod_version_id: null }
			]
		]);
	});

	it('marks profile work busy until a reply, and forgets it when the connection drops', () => {
		const state = new ModsState();
		state.connectionChanged(true);
		state.createProfile('t', 'Hard');
		state.activateProfile('t', 't/hard');
		state.reorderProfile('t', 't/hard', 'palschema', []);
		expect(state.managingProfile.t).toBe(true);
		expect(state.activating.t).toBe(true);
		expect(state.ordering.t).toBe(true);

		state.finishBusy('managingProfile', 't');
		expect(state.managingProfile.t).toBe(false);

		state.connectionChanged(false);
		expect(state.activating).toEqual({});
		expect(state.ordering).toEqual({});
	});
});

describe('ModsState viewed profile', () => {
	it('views the active profile until another is chosen, and forgets a removed choice', () => {
		const state = new ModsState();
		state.profiles = { t: [profile(), hard()] };
		expect(state.viewedProfile('t')?.id).toBe('t/default');

		state.viewProfile('t', 't/hard');
		expect(state.viewedProfile('t')?.id).toBe('t/hard');

		state.removeProfile('t', 't/hard');
		expect(state.profiles.t.map((p) => p.id)).toEqual(['t/default']);
		expect(state.viewedProfileId.t).toBeUndefined();
		expect(state.viewedProfile('t')?.id).toBe('t/default');
	});

	it('falls back to the active profile when the chosen id is unknown', () => {
		const state = new ModsState();
		state.profiles = { t: [profile()] };
		state.viewProfile('t', 't/missing');
		expect(state.viewedProfile('t')?.id).toBe('t/default');
	});

	it('reads enabled state from any profile', () => {
		const state = new ModsState();
		state.profiles = { t: [profile({ mods: [entry('a', 0)] }), hard({ mods: [entry('b', 0)] })] };
		expect(state.enabledIn('t', 't/hard', 'b')).toBe(true);
		expect(state.enabledIn('t', 't/hard', 'a')).toBe(false);
		expect(state.enabledIn('t', 't/other', 'a')).toBe(false);
	});

	it('adds, replaces and activates profiles', () => {
		const state = new ModsState();
		state.profiles = { t: [profile()] };
		state.upsertProfile(hard());
		state.upsertProfile(hard({ name: 'Harder' }));
		state.markActive('t', 't/hard');

		expect(state.profiles.t.map((p) => [p.name, p.is_active])).toEqual([
			['Default', false],
			['Harder', true]
		]);
	});

	it('forgets viewed profiles on reset and when a target is forgotten', () => {
		const state = new ModsState();
		state.viewProfile('t', 't/hard');
		state.viewProfile('u', 'u/hard');
		state.forgetTarget('t');
		expect(state.viewedProfileId).toEqual({ u: 'u/hard' });
		state.reset();
		expect(state.viewedProfileId).toEqual({});
	});
});

describe('ModsState optimistic order and options', () => {
	it('gives the listed mods the positions they already held, in the new order', () => {
		const state = new ModsState();
		state.profiles = {
			t: [profile({ mods: [entry('a', 0), entry('p', 1), entry('b', 2), entry('c', 5)] })]
		};

		state.reorderProfile('t', 't/default', 'ue4ss', ['c', 'a', 'b']);

		const order = Object.fromEntries(state.profiles.t[0].mods.map((m) => [m.mod_id, m.load_order]));
		expect(order).toEqual({ a: 2, p: 1, b: 5, c: 0 });
	});

	it('spreads tied positions before reordering', () => {
		const state = new ModsState();
		state.profiles = { t: [profile({ mods: [entry('a', 3), entry('b', 3)] })] };

		state.recordOrder('t', 't/default', ['b', 'a']);

		const order = Object.fromEntries(state.profiles.t[0].mods.map((m) => [m.mod_id, m.load_order]));
		expect(order).toEqual({ b: 3, a: 4 });
	});

	it('applies option changes to the stored profile at once', () => {
		const state = new ModsState();
		state.profiles = { t: [profile()] };
		state.setProfileOptions('t', 't/default', { ue4ss_control_mode: 'mods_txt' });
		expect(state.profiles.t[0].ue4ss_control_mode).toBe('mods_txt');
		expect(state.profiles.t[0].force_order_ue4ss).toBe(false);
	});
});
