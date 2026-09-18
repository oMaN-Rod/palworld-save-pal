// @vitest-environment jsdom
import { render } from '@testing-library/svelte';
import { tick } from 'svelte';
import { beforeEach, describe, expect, it, vi } from 'vitest';

const { holder, send, toasts } = vi.hoisted(() => ({
	holder: { state: undefined as unknown },
	send: vi.fn(),
	toasts: { add: vi.fn() }
}));

vi.mock('$lib/utils/websocketUtils', () => ({
	send: (type: unknown, data?: unknown) => send(type, data),
	sendAndWait: vi.fn(),
	sendBytes: vi.fn(),
	isReady: () => true,
	pushProgressMessage: vi.fn()
}));

vi.mock('$states', async () => {
	const { ModsState } = await import('$lib/states/modsState.svelte');
	holder.state = new ModsState();
	return { getModsState: () => holder.state, getToastState: () => toasts };
});

import type { ModsState } from '$lib/states/modsState.svelte';
import LaunchStatus from '../LaunchStatus.svelte';

const state = () => holder.state as ModsState;

beforeEach(() => {
	send.mockReset();
	toasts.add.mockReset();
	state().reset();
});

describe('LaunchStatus', () => {
	it('raises nothing while the launch runs', () => {
		state().launching = { 'client-abc': true };
		render(LaunchStatus, { targetId: 'client-abc' });
		expect(toasts.add).not.toHaveBeenCalled();
	});

	it.each([
		['target_locked', {}, 'Palworld is already running. Close it before launching from PalStudio.'],
		[
			'apply_failed',
			{ apply: {} },
			'Palworld was not started because its mods could not be applied.'
		],
		[
			'world_profile_other_target',
			{},
			'This world is linked to a profile of another game install.'
		],
		[
			'unsupported_platform',
			{ platform: 'mac' },
			"Launching isn't supported for macOS installs yet."
		],
		['launch_unavailable', {}, "PalStudio can't find this install's game executable"],
		[
			'launch_failed',
			{ reason: 'Access is denied.' },
			'Palworld could not be started: Access is denied.'
		],
		['desktop_only', {}, 'The game can only be launched from the PalStudio app'],
		['remote_denied', {}, 'The game can only be launched from the PalStudio app'],
		['not_supported_on_target', {}, 'Only a game install can be launched.'],
		['db', {}, 'database is locked']
	])('raises an error toast explaining %s, once', async (code, detail, text) => {
		state().recordRefusal(
			'game_launch',
			{ code, message: code === 'db' ? 'database is locked' : 'raw', ...detail },
			'client-abc'
		);
		render(LaunchStatus, { targetId: 'client-abc' });
		await tick();

		expect(toasts.add).toHaveBeenCalledTimes(1);
		const [message, , color] = toasts.add.mock.calls[0];
		expect(message).toContain(text);
		expect(color).toBe('error');
		expect(state().lastErrorFor('game_launch', 'client-abc')).toBeUndefined();
	});

	it('raises a success toast naming the profile a launch used, once', async () => {
		state().profiles = {
			'client-abc': [
				{
					id: 'client-abc/hard',
					target_id: 'client-abc',
					name: 'Hard',
					is_active: true,
					is_default: false,
					mods: [],
					ue4ss_control_mode: 'enabled_txt',
					force_order_ue4ss: false,
					force_order_palschema: false,
					created_at: '',
					updated_at: ''
				}
			]
		};
		render(LaunchStatus, { targetId: 'client-abc' });
		state().lastLaunch = {
			'client-abc': {
				target_id: 'client-abc',
				world_key: null,
				profile_id: 'client-abc/hard',
				activated: false,
				apply: null,
				launched: true
			}
		};
		await tick();

		expect(toasts.add).toHaveBeenCalledTimes(1);
		expect(toasts.add.mock.calls[0][0]).toBe('Palworld is starting with the Hard profile.');
		expect(toasts.add.mock.calls[0][2]).toBe('success');
		expect(state().lastLaunch['client-abc']).toBeUndefined();
	});

	it("ignores another target's launch", async () => {
		render(LaunchStatus, { targetId: 'client-abc' });
		state().recordRefusal('game_launch', { code: 'target_locked', message: 'raw' }, 'client-xyz');
		await tick();

		expect(toasts.add).not.toHaveBeenCalled();
	});
});
