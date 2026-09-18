import { beforeEach, describe, expect, it, vi } from 'vitest';

const { send, sendAndWait } = vi.hoisted(() => ({ send: vi.fn(), sendAndWait: vi.fn() }));
vi.mock('$lib/utils/websocketUtils', () => ({
	send: (type: string, data?: unknown) => send(type, data),
	sendAndWait
}));

import { ModsState } from './modsState.svelte';

describe('framework requests', () => {
	let state: ModsState;
	beforeEach(() => {
		send.mockReset();
		state = new ModsState();
	});

	it('loads status without reading GitHub unless asked', () => {
		state.loadFrameworks('t1');
		state.loadFrameworks('t1', true);
		expect(send.mock.calls).toEqual([
			['framework_status', { target_id: 't1' }],
			['framework_status', { target_id: 't1', check_latest: true }]
		]);
		expect(sendAndWait).not.toHaveBeenCalled();
	});

	it('installs the latest release or a library version, one operation per target', () => {
		state.installFramework('t1', 'ue4ss');
		expect(state.frameworkBusy.t1).toBe(true);
		state.finishBusy('frameworkBusy', 't1');
		state.installFramework('t1', 'amity', 'framework-amity@0.3.0');
		expect(send.mock.calls).toEqual([
			['framework_install', { target_id: 't1', key: 'ue4ss' }],
			[
				'framework_install',
				{ target_id: 't1', key: 'amity', mod_version_id: 'framework-amity@0.3.0' }
			]
		]);
	});

	it('removes with force only when asked, and removes hazards', () => {
		state.removeFramework('t1', 'ue4ss');
		state.finishBusy('frameworkBusy', 't1');
		state.removeFramework('t1', 'ue4ss', true);
		state.finishBusy('frameworkBusy', 't1');
		state.removeHazard('t1', 'workshop_proxy_dll');
		expect(send.mock.calls).toEqual([
			['framework_remove', { target_id: 't1', key: 'ue4ss' }],
			['framework_remove', { target_id: 't1', key: 'ue4ss', force: true }],
			['framework_hazard_remove', { target_id: 't1', hazard: 'workshop_proxy_dll' }]
		]);
	});

	it('clears both refusal types when either framework action starts', () => {
		state.recordRefusal(
			'framework_remove',
			{ code: 'framework_required', message: 'm', key: 'ue4ss' },
			't1'
		);
		state.installFramework('t1', 'ue4ss');
		expect(state.lastErrorFor('framework_remove', 't1')).toBeUndefined();

		state.recordRefusal(
			'framework_install',
			{ code: 'framework_required', message: 'm', key: 'ue4ss' },
			't1'
		);
		state.removeFramework('t1', 'ue4ss');
		expect(state.lastErrorFor('framework_install', 't1')).toBeUndefined();
	});

	it('clears framework state when a target is forgotten', () => {
		state.frameworks = { t1: { target_id: 't1', ue4ss_mode: 'none', hazards: [], frameworks: [] } };
		state.installFramework('t1', 'ue4ss');
		state.forgetTarget('t1');
		expect(state.frameworks.t1).toBeUndefined();
		expect(state.frameworkBusy.t1).toBeFalsy();
	});

	it('clears framework state on reset', () => {
		state.frameworks = { t1: { target_id: 't1', ue4ss_mode: 'none', hazards: [], frameworks: [] } };
		state.installFramework('t1', 'ue4ss');
		state.lastHazardRemove = {
			t1: {
				target_id: 't1',
				hazard: 'workshop_proxy_dll',
				moved: ['dwmapi.dll'],
				backup_dir: 'C:/b/1'
			}
		};
		state.reset();
		expect(state.frameworks).toEqual({});
		expect(state.frameworkBusy).toEqual({});
		expect(state.lastHazardRemove).toEqual({});
	});
});
