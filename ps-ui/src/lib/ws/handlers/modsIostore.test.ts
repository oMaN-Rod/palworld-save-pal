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
import { MessageType } from '$types';
import { modIostoreConvertHandler } from './modsHandler';

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

describe('convertIostore', () => {
	it('sends mod_iostore_convert and marks the target converting', () => {
		state.convertIostore('t1', 'm1');
		expect(send).toHaveBeenCalledWith('mod_iostore_convert', { target_id: 't1', mod_id: 'm1' });
		expect(state.converting.t1).toBe(true);
	});

	it('clears converting on reset and when a target is forgotten', () => {
		state.convertIostore('t1', 'm1');
		state.forgetTarget('t1');
		expect(state.converting.t1).toBeFalsy();
	});
});

describe('mod_iostore_convert', () => {
	it('a success reply releases the target, applies the selection and reloads', async () => {
		state.convertIostore('t1', 'm1');
		send.mockReset();
		await modIostoreConvertHandler.handle(
			{
				target_id: 't1',
				mod_id: 'm1',
				mod_version_id: 'v2',
				version: '1.0.0+iostore',
				converted: ['a.pak'],
				reused: false,
				request_id: null,
				pending: true,
				apply: null
			},
			context
		);
		expect(state.converting.t1).toBe(false);
		expect(state.pending.t1).toBe(true);
		expect(send.mock.calls.map(([type]) => type)).toEqual(
			expect.arrayContaining(['mod_list', 'profile_list', 'profile_plan'])
		);
	});

	it('records a not_convertible refusal by mod id without toasting', async () => {
		state.convertIostore('t1', 'm1');
		await modIostoreConvertHandler.handle(
			{ target_id: 't1', mod_id: 'm1', error: { code: 'not_convertible', message: 'no paks' } },
			context
		);
		expect(state.converting.t1).toBe(false);
		expect(state.lastErrorFor(MessageType.MOD_IOSTORE_CONVERT, 't1', { mod_id: 'm1' })?.code).toBe(
			'not_convertible'
		);
		expect(toast.add).not.toHaveBeenCalled();
	});

	it('reloads a stored conflict report after a successful conversion', async () => {
		state.conflicts = {
			t1: { target_id: 't1', profile_id: 'p', platform: 'wingdk', conflicts: [], unreadable: [] }
		};
		state.convertIostore('t1', 'm1');
		send.mockReset();
		await modIostoreConvertHandler.handle(
			{
				target_id: 't1',
				mod_id: 'm1',
				mod_version_id: 'v2',
				version: '1.0.0+iostore',
				converted: ['a.pak'],
				reused: false,
				request_id: null,
				pending: false,
				apply: null
			},
			context
		);
		expect(send).toHaveBeenCalledWith('mod_conflicts', { target_id: 't1' });
	});

	it('does not reload conflicts after a successful conversion without a stored report', async () => {
		state.convertIostore('t1', 'm1');
		send.mockReset();
		await modIostoreConvertHandler.handle(
			{
				target_id: 't1',
				mod_id: 'm1',
				mod_version_id: 'v2',
				version: '1.0.0+iostore',
				converted: ['a.pak'],
				reused: false,
				request_id: null,
				pending: false,
				apply: null
			},
			context
		);
		expect(send).not.toHaveBeenCalledWith('mod_conflicts', expect.anything());
	});
});
