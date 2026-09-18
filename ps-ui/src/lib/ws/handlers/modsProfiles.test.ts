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

import { applyResult } from '$lib/components/mods/__tests__/fixtures';
import type { ModsState } from '$lib/states/modsState.svelte';
import type { ModProfile } from '$types';
import {
	profileActivateHandler,
	profileCreateHandler,
	profileDeleteHandler,
	profileRenameHandler,
	profileReorderHandler,
	profileSetModHandler,
	profileSetOptionsHandler
} from './modsHandler';

const context = { goto: vi.fn() };
let state: ModsState;

function profile(overrides: Partial<ModProfile> = {}): ModProfile {
	return {
		id: 'client-a/default',
		target_id: 'client-a',
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
	profile({ id: 'client-a/hard', name: 'Hard', is_active: false, is_default: false, ...overrides });

beforeEach(async () => {
	await import('$states');
	const { ModsState } = await import('$lib/states/modsState.svelte');
	holder.state = new ModsState();
	state = holder.state as ModsState;
	state.profiles = { 'client-a': [profile(), hard()] };
	send.mockReset();
	toast.add.mockReset();
});

describe('profile_create', () => {
	it('adds the new profile and views it', async () => {
		state.createProfile('client-a', 'Brutal');
		const created = hard({ id: 'client-a/brutal', name: 'Brutal' });

		await profileCreateHandler.handle({ target_id: 'client-a', profile: created }, context);

		expect(state.managingProfile['client-a']).toBe(false);
		expect(state.profiles['client-a'].map((p) => p.id)).toEqual([
			'client-a/default',
			'client-a/hard',
			'client-a/brutal'
		]);
		expect(state.viewedProfile('client-a')?.id).toBe('client-a/brutal');
	});

	it('records a refused name under the target without a toast', async () => {
		state.createProfile('client-a', 'hard');

		await profileCreateHandler.handle(
			{
				target_id: 'client-a',
				name: 'hard',
				error: { code: 'name_taken', message: 'taken', profile_id: 'client-a/hard' }
			},
			context
		);

		expect(state.managingProfile['client-a']).toBe(false);
		expect(state.lastErrorFor('profile_create', 'client-a')?.code).toBe('name_taken');
		expect(toast.add).not.toHaveBeenCalled();
	});
});

describe('profile_rename', () => {
	it('replaces the stored profile', async () => {
		await profileRenameHandler.handle(
			{ target_id: 'client-a', profile: hard({ name: 'Harder' }) },
			context
		);
		expect(state.profiles['client-a'][1].name).toBe('Harder');
	});
});

describe('profile_activate', () => {
	it('marks the profile active, reloads profiles and plan, and records the apply', async () => {
		state.activateProfile('client-a', 'client-a/hard');
		send.mockReset();

		await profileActivateHandler.handle(
			{
				target_id: 'client-a',
				profile_id: 'client-a/hard',
				request_id: 'req-1',
				pending: false,
				apply: applyResult({ target_id: 'client-a', request_id: 'req-1' })
			},
			context
		);

		expect(state.activating['client-a']).toBe(false);
		expect(state.activeProfile('client-a')?.id).toBe('client-a/hard');
		expect(state.lastApply['client-a'].request_id).toBe('req-1');
		expect(state.pending['client-a']).toBe(false);
		expect(send).toHaveBeenCalledWith('profile_list', { target_id: 'client-a' });
		expect(send).toHaveBeenCalledWith('profile_plan', { target_id: 'client-a' });
	});

	it('keeps the activation and marks the target pending while the game runs', async () => {
		state.progress = {
			'req-2': {
				request_id: 'req-2',
				target_id: 'client-a',
				stage: 'checking',
				pct: 0,
				message: ''
			}
		};

		await profileActivateHandler.handle(
			{
				target_id: 'client-a',
				profile_id: 'client-a/hard',
				request_id: 'req-2',
				pending: true,
				apply: null
			},
			context
		);

		expect(state.pending['client-a']).toBe(true);
		expect(state.progressFor('client-a')).toBeUndefined();
		expect(state.activeProfile('client-a')?.id).toBe('client-a/hard');
	});

	it('records a refused apply that carries no request id without a toast', async () => {
		await profileActivateHandler.handle(
			{
				target_id: 'client-a',
				profile_id: 'client-a/hard',
				request_id: null,
				pending: false,
				apply: { error: { code: 'journal_open', message: 'finish first' } } as never
			},
			context
		);

		expect(state.lastErrorFor('profile_apply', 'client-a')?.code).toBe('journal_open');
		expect(toast.add).not.toHaveBeenCalled();
	});

	it('reloads the profiles when the profile is gone', async () => {
		state.activateProfile('client-a', 'client-a/gone');
		await profileActivateHandler.handle(
			{
				target_id: 'client-a',
				profile_id: 'client-a/gone',
				error: { code: 'profile_not_found', message: 'gone', profile_id: 'client-a/gone' }
			},
			context
		);

		expect(state.activating['client-a']).toBe(false);
		expect(send).toHaveBeenCalledWith('profile_list', { target_id: 'client-a' });
		expect(state.lastErrorFor('profile_activate', 'client-a')?.code).toBe('profile_not_found');
		expect(toast.add).not.toHaveBeenCalled();
	});
});

describe('profile_delete', () => {
	it('drops the active profile, views the fallback and records its apply', async () => {
		state.profiles = { 'client-a': [profile({ is_active: false }), hard({ is_active: true })] };
		state.viewProfile('client-a', 'client-a/hard');

		await profileDeleteHandler.handle(
			{
				target_id: 'client-a',
				profile_id: 'client-a/hard',
				deleted: true,
				active_profile_id: 'client-a/default',
				request_id: 'req-3',
				pending: false,
				apply: applyResult({ target_id: 'client-a', request_id: 'req-3' })
			},
			context
		);

		expect(state.profiles['client-a'].map((p) => p.id)).toEqual(['client-a/default']);
		expect(state.viewedProfile('client-a')?.id).toBe('client-a/default');
		expect(state.activeProfile('client-a')?.id).toBe('client-a/default');
		expect(state.lastApply['client-a'].request_id).toBe('req-3');
	});

	it('leaves a pending selection alone when a profile that is not active is deleted', async () => {
		state.pending = { 'client-a': true };

		await profileDeleteHandler.handle(
			{
				target_id: 'client-a',
				profile_id: 'client-a/hard',
				deleted: true,
				active_profile_id: 'client-a/default',
				request_id: null,
				pending: false,
				apply: null
			},
			context
		);

		expect(state.pending['client-a']).toBe(true);
		expect(state.lastApply['client-a']).toBeUndefined();
	});
});

describe('profile_set_mod on a profile that is not active', () => {
	it('records the entry and leaves the active profile pending state alone', async () => {
		state.pending = { 'client-a': true };
		state.setMod('client-a', 'cool', true, 'client-a/hard');

		await profileSetModHandler.handle(
			{
				target_id: 'client-a',
				profile_id: 'client-a/hard',
				mod_id: 'cool',
				enabled: true,
				mod_version_id: null,
				request_id: null,
				pending: false,
				apply: null
			},
			context
		);

		expect(state.settingMod['client-a']).toBe(false);
		expect(state.pending['client-a']).toBe(true);
		expect(state.enabledIn('client-a', 'client-a/hard', 'cool')).toBe(true);
		expect(state.enabledOn('client-a', 'cool')).toBe(false);
	});
});

describe('profile_reorder and profile_set_options', () => {
	it('reloads profiles and plan after a reorder, and reloads profiles after a refused one', async () => {
		state.reorderProfile('client-a', 'client-a/default', 'ue4ss', []);
		await profileReorderHandler.handle(
			{
				target_id: 'client-a',
				profile_id: 'client-a/default',
				kind: 'ue4ss',
				ordered_mod_ids: [],
				request_id: 'req-4',
				pending: false,
				apply: applyResult({ target_id: 'client-a', request_id: 'req-4' })
			},
			context
		);
		expect(state.ordering['client-a']).toBe(false);
		expect(send).toHaveBeenCalledWith('profile_list', { target_id: 'client-a' });
		expect(send).toHaveBeenCalledWith('profile_plan', { target_id: 'client-a' });

		state.reorderProfile('client-a', 'client-a/default', 'ue4ss', ['x']);
		send.mockReset();
		await profileReorderHandler.handle(
			{
				target_id: 'client-a',
				profile_id: 'client-a/default',
				kind: 'ue4ss',
				error: { code: 'invalid_order', message: 'stale', expected: ['a'] }
			},
			context
		);
		expect(state.ordering['client-a']).toBe(false);
		expect(send.mock.calls).toEqual([['profile_list', { target_id: 'client-a' }]]);
		expect(state.lastErrorFor('profile_reorder', 'client-a')?.code).toBe('invalid_order');
		expect(toast.add).not.toHaveBeenCalled();
	});

	it('stores the profile an options change returns', async () => {
		state.setProfileOptions('client-a', 'client-a/hard', { force_order_ue4ss: true });
		await profileSetOptionsHandler.handle(
			{
				target_id: 'client-a',
				profile: hard({ force_order_ue4ss: true, ue4ss_control_mode: 'mods_txt' }),
				request_id: null,
				pending: false,
				apply: null
			},
			context
		);
		expect(state.ordering['client-a']).toBe(false);
		expect(state.profiles['client-a'][1].ue4ss_control_mode).toBe('mods_txt');
	});

	it('reloads the profiles when an options change is refused', async () => {
		await profileSetOptionsHandler.handle(
			{
				target_id: 'client-a',
				error: { code: 'invalid_option', message: 'no', ue4ss_control_mode: 'sometimes' }
			},
			context
		);
		expect(send).toHaveBeenCalledWith('profile_list', { target_id: 'client-a' });
	});
});
