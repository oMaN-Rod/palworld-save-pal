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
import { modConflictsHandler } from './modsHandler';

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

describe('loadConflicts', () => {
	it('asks for the active profile and marks the target busy', () => {
		state.loadConflicts('t1');
		expect(send).toHaveBeenCalledWith('mod_conflicts', { target_id: 't1' });
		expect(state.checkingConflicts.t1).toBe(true);
	});

	it('clears conflicts on reset and when a target is forgotten', () => {
		state.conflicts = {
			t1: { target_id: 't1', profile_id: 'p', platform: 'wingdk', conflicts: [], unreadable: [] }
		};
		state.loadConflicts('t1');
		state.forgetTarget('t1');
		expect(state.conflicts.t1).toBeUndefined();
		expect(state.checkingConflicts.t1).toBeFalsy();
	});
});

describe('mod_conflicts', () => {
	it('stores the report and releases the target', async () => {
		state.loadConflicts('t1');
		await modConflictsHandler.handle(
			{ target_id: 't1', profile_id: 'p', platform: 'wingdk', conflicts: [], unreadable: [] },
			context
		);
		expect(state.checkingConflicts.t1).toBe(false);
		expect(state.conflicts.t1.platform).toBe('wingdk');
	});

	it('records a refusal on the target without toasting', async () => {
		state.loadConflicts('t1');
		await modConflictsHandler.handle(
			{ target_id: 't1', error: { code: 'no_active_profile', message: 'none' } },
			context
		);
		expect(state.lastErrorFor(MessageType.MOD_CONFLICTS, 't1')?.code).toBe('no_active_profile');
		expect(toast.add).not.toHaveBeenCalled();
	});
});
