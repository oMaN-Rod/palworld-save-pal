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
import type { LocalSave, ModProfile } from '$types';
import { gameLaunchHandler, listLocalSavesHandler, worldProfileSetHandler } from './modsHandler';

const context = { goto: vi.fn() };
let state: ModsState;

function hard(): ModProfile {
	return {
		id: 'client-a/hard',
		target_id: 'client-a',
		name: 'Hard',
		is_active: false,
		is_default: false,
		mods: [],
		ue4ss_control_mode: 'enabled_txt',
		force_order_ue4ss: false,
		force_order_palschema: false,
		created_at: '',
		updated_at: ''
	};
}

function save(overrides: Partial<LocalSave> = {}): LocalSave {
	return {
		path: 'C:/saves/1/AAA/Level.sav',
		name: 'AAA',
		save_type: 'steam',
		modified_ms: 1,
		world_key: 'C:/saves/1/AAA',
		mod_profile: null,
		...overrides
	};
}

beforeEach(async () => {
	await import('$states');
	const { ModsState } = await import('$lib/states/modsState.svelte');
	holder.state = new ModsState();
	state = holder.state as ModsState;
	send.mockReset();
	toast.add.mockReset();
});

describe('world and launch requests', () => {
	it('sends each request with its wire payload and marks it busy', () => {
		state.loadLocalSaves();
		state.setWorldProfile('C:/saves/1/AAA', 'AAA', 'client-a/hard');
		state.setWorldProfile('C:/saves/1/AAA', 'AAA', null);
		state.launch('client-a');
		state.launch('client-a', 'C:/saves/1/AAA');

		expect(send.mock.calls).toEqual([
			['list_local_saves', { include_gamepass: true }],
			[
				'world_profile_set',
				{ world_key: 'C:/saves/1/AAA', world_name: 'AAA', profile_id: 'client-a/hard' }
			],
			['world_profile_set', { world_key: 'C:/saves/1/AAA', world_name: 'AAA', profile_id: null }],
			['game_launch', { target_id: 'client-a' }],
			['game_launch', { target_id: 'client-a', world_key: 'C:/saves/1/AAA' }]
		]);
		expect(state.loadingSaves).toBe(true);
		expect(state.linkingWorld['C:/saves/1/AAA']).toBe(true);
		expect(state.launching['client-a']).toBe(true);
	});

	it('forgets loading, linking and launching when the connection drops, and saves and launches on reset', () => {
		state.connectionChanged(true);
		state.loadLocalSaves();
		state.setWorldProfile('k', 'W', null);
		state.launch('client-a');
		state.localSaves = [save()];
		state.lastLaunch = {
			'client-a': {
				target_id: 'client-a',
				world_key: null,
				profile_id: 'p',
				activated: false,
				apply: null,
				launched: true
			}
		};

		state.connectionChanged(false);
		expect(state.loadingSaves).toBe(false);
		expect(state.linkingWorld).toEqual({});
		expect(state.launching).toEqual({});

		state.reset();
		expect(state.localSaves).toBeUndefined();
		expect(state.lastLaunch).toEqual({});
	});
});

describe('list_local_saves', () => {
	it('stores the saves', async () => {
		state.loadLocalSaves();
		await listLocalSavesHandler.handle({ saves: [save()] }, context);
		expect(state.loadingSaves).toBe(false);
		expect(state.localSaves).toEqual([save()]);
	});

	it('records the legacy string refusal without a toast', async () => {
		state.loadLocalSaves();
		await listLocalSavesHandler.handle(
			{ error: 'Desktop mode is required to list local saves' },
			context
		);
		expect(state.loadingSaves).toBe(false);
		expect(state.lastErrorFor('list_local_saves')?.message).toBe(
			'Desktop mode is required to list local saves'
		);
		expect(toast.add).not.toHaveBeenCalled();
	});
});

describe('world_profile_set', () => {
	it('shows the new link at once and reloads profiles and saves', async () => {
		state.profiles = { 'client-a': [hard()] };
		state.localSaves = [save()];
		state.setWorldProfile('C:/saves/1/AAA', 'AAA', 'client-a/hard');
		send.mockReset();

		await worldProfileSetHandler.handle(
			{
				world_key: 'C:/saves/1/AAA',
				world_name: 'AAA',
				profile_id: 'client-a/hard',
				linked: true
			},
			context
		);

		expect(state.linkingWorld['C:/saves/1/AAA']).toBe(false);
		expect(state.localSaves?.[0].mod_profile).toEqual({
			profile_id: 'client-a/hard',
			profile_name: 'Hard',
			target_id: 'client-a'
		});
		expect(send.mock.calls).toEqual([
			['profile_list', { target_id: 'client-a' }],
			['list_local_saves', { include_gamepass: true }]
		]);
	});

	it('records a refusal with the world it named', async () => {
		await worldProfileSetHandler.handle(
			{
				world_key: 'C:/saves/1/AAA',
				world_name: 'AAA',
				profile_id: 'gone',
				error: { code: 'profile_not_found', message: 'no', profile_id: 'gone' }
			},
			context
		);
		expect(state.lastErrorFor('world_profile_set')).toMatchObject({
			code: 'profile_not_found',
			world_key: 'C:/saves/1/AAA',
			world_name: 'AAA'
		});
		expect(toast.add).not.toHaveBeenCalled();
	});
});

describe('game_launch', () => {
	it('records a launch, its apply and a reload after activating a linked profile', async () => {
		state.launch('client-a', 'C:/saves/1/AAA');
		state.pending['client-a'] = true;
		send.mockReset();

		await gameLaunchHandler.handle(
			{
				target_id: 'client-a',
				world_key: 'C:/saves/1/AAA',
				profile_id: 'client-a/hard',
				activated: true,
				apply: applyResult({ target_id: 'client-a', request_id: 'req-9' }),
				launched: true
			},
			context
		);

		expect(state.launching['client-a']).toBe(false);
		expect(state.lastLaunch['client-a'].profile_id).toBe('client-a/hard');
		expect(state.lastApply['client-a'].request_id).toBe('req-9');
		expect(state.pending['client-a']).toBe(false);
		expect(send).toHaveBeenCalledWith('profile_list', { target_id: 'client-a' });
		expect(send).toHaveBeenCalledWith('profile_plan', { target_id: 'client-a' });
	});

	it('leaves a prior pending selection alone when the launch is refused', async () => {
		state.launch('client-a');
		state.pending['client-a'] = true;
		await gameLaunchHandler.handle(
			{
				target_id: 'client-a',
				world_key: null,
				error: { code: 'target_locked', message: 'running' }
			},
			context
		);
		expect(state.pending['client-a']).toBe(true);
	});

	it.each([
		'target_locked',
		'world_profile_other_target',
		'unsupported_platform',
		'launch_unavailable',
		'launch_failed',
		'desktop_only',
		'not_supported_on_target'
	])('records %s under the target without a toast', async (code) => {
		state.launch('client-a');
		await gameLaunchHandler.handle(
			{ target_id: 'client-a', world_key: null, error: { code, message: code } },
			context
		);
		expect(state.launching['client-a']).toBe(false);
		expect(state.lastErrorFor('game_launch', 'client-a')?.code).toBe(code);
		expect(toast.add).not.toHaveBeenCalled();
	});

	it.each(['layout_error', 'unsupported_platform', 'launch_unavailable', 'launch_failed', 'db'])(
		'reloads the profiles and plan after %s, which leaves a linked profile active and applied',
		async (code) => {
			state.launch('client-a', 'C:/saves/1/AAA');
			send.mockReset();
			await gameLaunchHandler.handle(
				{ target_id: 'client-a', world_key: 'C:/saves/1/AAA', error: { code, message: code } },
				context
			);
			expect(send.mock.calls).toEqual([
				['profile_list', { target_id: 'client-a' }],
				['profile_plan', { target_id: 'client-a' }]
			]);
		}
	);

	it.each(['target_locked', 'world_profile_other_target', 'desktop_only'])(
		'reloads nothing after %s, which is refused before anything is activated',
		async (code) => {
			state.launch('client-a');
			send.mockReset();
			await gameLaunchHandler.handle(
				{ target_id: 'client-a', world_key: null, error: { code, message: code } },
				context
			);
			expect(send).not.toHaveBeenCalled();
		}
	);

	it('shows the apply of an apply_failed refusal through the apply bar', async () => {
		const failed = applyResult({
			target_id: 'client-a',
			request_id: 'req-10',
			error: { code: 'unmanaged_occupant', message: 'in the way', paths: ['x.pak'] }
		});
		await gameLaunchHandler.handle(
			{
				target_id: 'client-a',
				world_key: null,
				error: { code: 'apply_failed', message: 'apply failed', apply: failed }
			},
			context
		);
		expect(state.lastErrorFor('game_launch', 'client-a')?.code).toBe('apply_failed');
		expect(state.lastApply['client-a'].request_id).toBe('req-10');
		expect(state.lastErrorFor('profile_apply', 'client-a')?.code).toBe('unmanaged_occupant');
		expect(send).toHaveBeenCalledWith('profile_list', { target_id: 'client-a' });
	});

	it('clears the previous outcome when a new launch starts', () => {
		state.recordRefusal('game_launch', { code: 'target_locked', message: 'running' }, 'client-a');
		state.launch('client-a');
		expect(state.lastErrorFor('game_launch', 'client-a')).toBeUndefined();
	});
});
