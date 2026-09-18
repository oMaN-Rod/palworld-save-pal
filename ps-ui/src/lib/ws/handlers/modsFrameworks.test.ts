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
	mods_panel_title: () => 'Mods'
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
import {
	frameworkHazardRemoveHandler,
	frameworkInstallHandler,
	frameworkRemoveHandler,
	frameworkStatusHandler
} from './modsHandler';

const context = { goto: vi.fn() };

beforeEach(async () => {
	await import('$states');
	const { ModsState } = await import('$lib/states/modsState.svelte');
	holder.state = new ModsState();
	send.mockReset();
	toast.add.mockReset();
});

describe('framework_status', () => {
	it('stores framework status', async () => {
		await frameworkStatusHandler.handle(
			{ target_id: 't1', ue4ss_mode: 'standard', hazards: [], frameworks: [] },
			context
		);
		expect((holder.state as ModsState).frameworks.t1.ue4ss_mode).toBe('standard');
	});

	it('records a status refusal on the target without toasting', async () => {
		await frameworkStatusHandler.handle(
			{ target_id: 't1', error: { code: 'not_supported_on_target', message: 'docker' } },
			context
		);
		expect((holder.state as ModsState).lastErrorFor(MessageType.FRAMEWORK_STATUS, 't1')?.code).toBe(
			'not_supported_on_target'
		);
		expect(toast.add).not.toHaveBeenCalled();
	});
});

describe('framework_install', () => {
	it('an install reply releases the target, applies the selection and reloads status and library', async () => {
		(holder.state as ModsState).installFramework('t1', 'ue4ss');
		send.mockReset();
		await frameworkInstallHandler.handle(
			{
				target_id: 't1',
				key: 'ue4ss',
				mod_version_id: 'v',
				version: '3.0.1',
				display: '3.0.1',
				installed_new: true,
				request_id: null,
				pending: true,
				apply: null
			},
			context
		);
		expect((holder.state as ModsState).frameworkBusy.t1).toBe(false);
		expect((holder.state as ModsState).pending.t1).toBe(true);
		expect(send.mock.calls.map(([type]) => type)).toEqual(
			expect.arrayContaining(['framework_status', 'mod_list', 'mod_target_list'])
		);
	});
});

describe('framework_remove', () => {
	it('a remove reply releases the target, applies also_removed and reloads status and library', async () => {
		(holder.state as ModsState).removeFramework('t1', 'ue4ss');
		send.mockReset();
		await frameworkRemoveHandler.handle(
			{
				target_id: 't1',
				key: 'ue4ss',
				removed: true,
				also_removed: ['palschema', 'amity'],
				request_id: null,
				pending: true,
				apply: null
			},
			context
		);
		expect((holder.state as ModsState).frameworkBusy.t1).toBe(false);
		expect((holder.state as ModsState).pending.t1).toBe(true);
		expect(send.mock.calls.map(([type]) => type)).toEqual(
			expect.arrayContaining(['framework_status', 'mod_list', 'mod_target_list'])
		);
	});

	it('refreshes a stored conflict report after an install success', async () => {
		(holder.state as ModsState).conflicts = {
			t1: { target_id: 't1', profile_id: 'p1', platform: 'wingdk', conflicts: [], unreadable: [] }
		};
		(holder.state as ModsState).installFramework('t1', 'ue4ss');
		send.mockReset();
		await frameworkInstallHandler.handle(
			{ target_id: 't1', key: 'ue4ss', request_id: null, pending: false, apply: null },
			context
		);
		expect(send.mock.calls.map(([type]) => type)).toContain('mod_conflicts');
	});

	it('does not request conflicts without a stored report', async () => {
		(holder.state as ModsState).installFramework('t1', 'ue4ss');
		send.mockReset();
		await frameworkInstallHandler.handle(
			{ target_id: 't1', key: 'ue4ss', request_id: null, pending: false, apply: null },
			context
		);
		expect(send.mock.calls.map(([type]) => type)).not.toContain('mod_conflicts');
	});

	it('keeps framework_required dependents on the refusal', async () => {
		(holder.state as ModsState).removeFramework('t1', 'ue4ss');
		await frameworkRemoveHandler.handle(
			{
				target_id: 't1',
				key: 'ue4ss',
				error: {
					code: 'framework_required',
					message: 'm',
					framework: 'ue4ss',
					dependents: ['CoolMod']
				}
			},
			context
		);
		expect((holder.state as ModsState).frameworkBusy.t1).toBe(false);
		const refusal = (holder.state as ModsState).lastErrorFor(MessageType.FRAMEWORK_REMOVE, 't1');
		expect(refusal?.dependents).toEqual(['CoolMod']);
		expect(refusal?.key).toBe('ue4ss');
	});
});

describe('framework_hazard_remove', () => {
	it('a refusal releases the target, leaves lastHazardRemove untouched and does not toast', async () => {
		(holder.state as ModsState).removeHazard('t1', 'workshop_proxy_dll');
		send.mockReset();
		await frameworkHazardRemoveHandler.handle(
			{
				target_id: 't1',
				hazard: 'workshop_proxy_dll',
				error: { code: 'hazard_not_found', message: 'gone' }
			},
			context
		);
		expect((holder.state as ModsState).frameworkBusy.t1).toBe(false);
		expect((holder.state as ModsState).lastHazardRemove.t1).toBeUndefined();
		expect(
			(holder.state as ModsState).lastErrorFor(MessageType.FRAMEWORK_HAZARD_REMOVE, 't1')?.code
		).toBe('hazard_not_found');
		expect(toast.add).not.toHaveBeenCalled();
	});

	it('a hazard removal records where the files went and reloads the target list', async () => {
		(holder.state as ModsState).removeHazard('t1', 'workshop_proxy_dll');
		send.mockReset();
		await frameworkHazardRemoveHandler.handle(
			{
				target_id: 't1',
				hazard: 'workshop_proxy_dll',
				moved: ['dwmapi.dll'],
				backup_dir: 'C:/b/1'
			},
			context
		);
		expect((holder.state as ModsState).lastHazardRemove.t1.backup_dir).toBe('C:/b/1');
		expect(send.mock.calls.map(([type]) => type)).toEqual(
			expect.arrayContaining(['mod_target_list', 'framework_status'])
		);
	});
});
