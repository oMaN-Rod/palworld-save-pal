import { beforeEach, describe, expect, it, vi } from 'vitest';

const { send, toast, holder } = vi.hoisted(() => ({
	send: vi.fn(),
	toast: { add: vi.fn() },
	holder: { state: undefined as unknown, server: undefined as unknown }
}));

vi.mock('$lib/utils/websocketUtils', () => ({
	send: (type: unknown, data?: unknown) => send(type, data),
	sendAndWait: vi.fn()
}));

vi.mock('$i18n/messages', () => ({
	mods_panel_title: () => 'Mods',
	mods_panel_applied: () => 'Mods applied'
}));

vi.mock('$states', async () => {
	const { ModsState } = await import('$lib/states/modsState.svelte');
	const { ServerState } = await import('$lib/states/serverState.svelte');
	holder.state = new ModsState();
	holder.server = new ServerState();
	return {
		getModsState: () => holder.state,
		getServerState: () => holder.server,
		getToastState: () => toast
	};
});

import type { ModsState } from '$lib/states/modsState.svelte';
import type { ModProfile } from '$types';
import { MessageType } from '$types';
import { modReleaseProfilesHandler, profileRemoveModHandler } from './modsHandler';

const context = { goto: vi.fn() };
let state: ModsState;

beforeEach(async () => {
	await import('$states');
	const { ModsState } = await import('$lib/states/modsState.svelte');
	holder.state = new ModsState();
	state = holder.state as ModsState;
	send.mockReset();
	toast.add.mockReset();
});

function profile(id: string, targetId: string, modIds: string[]): ModProfile {
	return {
		id,
		target_id: targetId,
		name: 'Default',
		is_active: true,
		is_default: true,
		mods: modIds.map((modId, index) => ({
			profile_id: id,
			mod_id: modId,
			mod_version_id: null,
			enabled: false,
			load_order: index
		})),
		ue4ss_control_mode: 'enabled_txt',
		force_order_ue4ss: false,
		force_order_palschema: false,
		created_at: '',
		updated_at: ''
	};
}

describe('removeFromProfile', () => {
	it('sends the viewed profile when given and marks the target busy', () => {
		state.removeFromProfile('t1', 'm1');
		state.finishBusy('settingMod', 't1');
		state.removeFromProfile('t1', 'm1', 'p2');
		expect(send.mock.calls).toEqual([
			['profile_remove_mod', { target_id: 't1', mod_id: 'm1' }],
			['profile_remove_mod', { target_id: 't1', profile_id: 'p2', mod_id: 'm1' }]
		]);
		expect(state.settingMod.t1).toBe(true);
	});
});

describe('profile_remove_mod', () => {
	it('drops the entry, releases the target and reloads profiles and the plan', async () => {
		state.profiles = { t1: [profile('p1', 't1', ['m1', 'm2'])] };
		state.removeFromProfile('t1', 'm1', 'p1');
		send.mockReset();
		await profileRemoveModHandler.handle(
			{ target_id: 't1', profile_id: 'p1', mod_id: 'm1', removed: true },
			context
		);
		expect(state.settingMod.t1).toBe(false);
		expect(state.profiles.t1[0].mods.map((entry) => entry.mod_id)).toEqual(['m2']);
		expect(send.mock.calls).toEqual([
			['profile_list', { target_id: 't1' }],
			['profile_plan', { target_id: 't1' }]
		]);
	});

	it('keeps a refusal on the row without toasting', async () => {
		state.removeFromProfile('t1', 'm1');
		await profileRemoveModHandler.handle(
			{
				target_id: 't1',
				mod_id: 'm1',
				error: { code: 'mod_not_in_profile', message: 'gone' }
			},
			context
		);
		expect(state.settingMod.t1).toBe(false);
		expect(state.lastErrorFor(MessageType.PROFILE_REMOVE_MOD, 't1', { mod_id: 'm1' })?.code).toBe(
			'mod_not_in_profile'
		);
		expect(toast.add).not.toHaveBeenCalled();
	});
});

describe('mod_release_profiles', () => {
	const holders = [
		{
			target_id: 'a',
			target_name: 'A',
			profile_id: 'a/default',
			profile_name: 'Default',
			enabled: false
		},
		{
			target_id: 'b',
			target_name: 'B',
			profile_id: 'b/default',
			profile_name: 'Default',
			enabled: true
		}
	];

	it('sends by mod id and marks the mod busy', () => {
		state.releaseProfiles('m1');
		expect(send).toHaveBeenCalledWith('mod_release_profiles', { mod_id: 'm1' });
		expect(state.releasing.m1).toBe(true);
	});

	it('trims the stored in-use refusal, records the outcome and reloads each released target once', async () => {
		state.recordRefusal(
			MessageType.MOD_REMOVE,
			{
				code: 'version_in_use',
				message: 'in use',
				targets: ['a', 'b'],
				profiles: ['a/default', 'b/default'],
				holders,
				deployed: [{ target_id: 'a', target_name: 'A' }]
			},
			undefined,
			{ subject: { mod_id: 'm1' } }
		);
		state.releaseProfiles('m1');
		send.mockReset();
		await modReleaseProfilesHandler.handle(
			{
				mod_id: 'm1',
				removed: [{ target_id: 'a', profile_id: 'a/default' }],
				enabled: [{ target_id: 'b', profile_id: 'b/default' }]
			},
			context
		);
		expect(state.releasing.m1).toBe(false);
		expect(state.lastRelease.m1.removed).toHaveLength(1);
		const stored = state.lastErrorFor(MessageType.MOD_REMOVE, undefined, { mod_id: 'm1' });
		expect(stored?.holders).toEqual([holders[1]]);
		expect(stored?.profiles).toEqual(['b/default']);
		expect(stored?.deployed).toEqual([{ target_id: 'a', target_name: 'A' }]);
		expect(send.mock.calls).toEqual([
			['profile_list', { target_id: 'a' }],
			['profile_plan', { target_id: 'a' }]
		]);
	});

	it('keeps a release refusal on the mod without toasting', async () => {
		state.releaseProfiles('m1');
		await modReleaseProfilesHandler.handle(
			{ mod_id: 'm1', error: { code: 'db', message: 'locked' } },
			context
		);
		expect(state.releasing.m1).toBe(false);
		expect(
			state.lastErrorFor(MessageType.MOD_RELEASE_PROFILES, undefined, { mod_id: 'm1' })?.code
		).toBe('db');
		expect(toast.add).not.toHaveBeenCalled();
	});

	it('clears release state on reset', () => {
		state.releaseProfiles('m1');
		state.lastRelease = { m1: { mod_id: 'm1', removed: [], enabled: [] } };
		state.reset();
		expect(state.releasing.m1).toBeFalsy();
		expect(state.lastRelease.m1).toBeUndefined();
	});
});
