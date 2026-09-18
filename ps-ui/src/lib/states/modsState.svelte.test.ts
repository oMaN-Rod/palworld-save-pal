import type { ModProfile, ModTarget, ProfileMod, TargetPlan } from '$types';
import { beforeEach, describe, expect, it, vi } from 'vitest';

const send = vi.fn();
const sendAndWait = vi.fn();

vi.mock('$lib/utils/websocketUtils', () => ({
	send: (type: unknown, data?: unknown) => send(type, data),
	sendAndWait: (type: unknown, data?: unknown) => sendAndWait(type, data)
}));

import { modTarget } from '$lib/components/mods/__tests__/fixtures';
import { ModsState } from './modsState.svelte';

beforeEach(() => {
	send.mockReset();
	sendAndWait.mockReset();
});

describe('ModsState requests', () => {
	it('sends each request fire-and-forget with its wire payload', () => {
		const state = new ModsState();
		state.loadTargets();
		state.loadLibrary();
		state.loadProfiles('server-7');
		state.setMod('server-7', 'cool-mod', true);
		state.apply('server-7');
		state.install('server-7', '__select__');

		expect(send.mock.calls).toEqual([
			['mod_target_list', undefined],
			['mod_list', undefined],
			['profile_list', { target_id: 'server-7' }],
			['profile_set_mod', { target_id: 'server-7', mod_id: 'cool-mod', enabled: true }],
			['profile_apply', { target_id: 'server-7' }],
			['mod_install', { target_id: 'server-7', path: '__select__', accept_defaults: false }]
		]);
		expect(sendAndWait).not.toHaveBeenCalled();
	});

	it('marks the target as applying and installing until a reply arrives', () => {
		const state = new ModsState();
		state.apply('server-1');
		state.install('server-1', 'C:/mods/cool.zip', true);
		expect(state.applying['server-1']).toBe(true);
		expect(state.installing['server-1']).toBe(true);
	});

	it('clears progress by request and by target', () => {
		const state = new ModsState();
		state.progress = {
			a: { request_id: 'a', target_id: 'server-1', stage: 'applying', pct: 10, message: '' },
			b: { request_id: 'b', target_id: 'server-2', stage: 'applying', pct: 10, message: '' },
			c: { request_id: 'c', target_id: 'server-2', stage: 'done', pct: 100, message: '' }
		};
		state.clearProgress('a');
		expect(Object.keys(state.progress)).toEqual(['b', 'c']);
		state.clearTargetProgress('server-2');
		expect(state.progress).toEqual({});
	});

	it('scans a target and adopts a candidate by name, source and root', () => {
		const state = new ModsState();
		state.scan('server-3');
		state.adopt('server-3', {
			name: 'SteamPack',
			kind: 'workshop',
			root: 'C:/steam/1623730/3300000001',
			files: [],
			enabled: true,
			source: 'steam_subscribed'
		});

		expect(send.mock.calls).toEqual([
			['mod_target_scan', { target_id: 'server-3', candidates_only: true }],
			[
				'mod_adopt',
				{
					target_id: 'server-3',
					candidate_name: 'SteamPack',
					source: 'steam_subscribed',
					root: 'C:/steam/1623730/3300000001'
				}
			]
		]);
		expect(state.adopting['server-3']).toBe(true);
		expect(sendAndWait).not.toHaveBeenCalled();
	});

	it('scans each target once per session and rescans only targets it has scanned', () => {
		const state = new ModsState();
		state.scanOnce('server-1');
		state.scanOnce('server-1');
		state.scanOnce('server-2');
		state.rescan('server-3');
		state.rescan('server-1');

		expect(send.mock.calls).toEqual([
			['mod_target_scan', { target_id: 'server-1', candidates_only: true }],
			['mod_target_scan', { target_id: 'server-2', candidates_only: true }],
			['mod_target_scan', { target_id: 'server-1', candidates_only: true }]
		]);
	});

	it('drops pending install decisions when the install is retried', () => {
		const state = new ModsState();
		state.needsDecisions = { 'server-1': { path: 'C:/a.zip', decisions: [] } };
		state.install('server-1', 'C:/a.zip', true);
		expect(state.needsDecisions['server-1']).toBeUndefined();
	});
});

function entry(mod_id: string, enabled: boolean): ProfileMod {
	return { profile_id: 'p', mod_id, mod_version_id: null, enabled, load_order: 0 };
}

function profile(
	id: string,
	isActive: boolean,
	mods: ProfileMod[],
	targetId = 'server-1'
): ModProfile {
	return {
		id,
		target_id: targetId,
		name: id,
		is_active: isActive,
		is_default: isActive,
		mods,
		ue4ss_control_mode: 'auto',
		force_order_ue4ss: false,
		force_order_palschema: false,
		created_at: '2026-01-01T00:00:00Z',
		updated_at: '2026-01-01T00:00:00Z'
	};
}

function target(id: string): ModTarget {
	return modTarget({ id, name: id, root_path: `C:/Games/${id}` });
}

describe('ModsState target, library and backup requests', () => {
	it('sends every target request with its wire payload', () => {
		const state = new ModsState();
		state.detect();
		state.addTarget('__select__');
		state.removeTarget('client-abc');
		state.analyze('client-abc', 'C:/mods/cool.zip');

		expect(send.mock.calls).toEqual([
			['mod_target_detect', undefined],
			['mod_target_add', { root_path: '__select__' }],
			['mod_target_remove', { target_id: 'client-abc' }],
			['mod_analyze', { target_id: 'client-abc', path: 'C:/mods/cool.zip' }]
		]);
		expect(sendAndWait).not.toHaveBeenCalled();
	});

	it('sends every library request with its wire payload', () => {
		const state = new ModsState();
		state.removeMod('cool-mod');
		state.setCurrentVersion('cool-mod', 'v2');
		state.deleteVersion('v1');

		expect(send.mock.calls).toEqual([
			['mod_remove', { mod_id: 'cool-mod' }],
			['mod_version_set_current', { mod_id: 'cool-mod', version_id: 'v2' }],
			['mod_version_delete', { version_id: 'v1' }]
		]);
	});

	it('sends plan, apply with replacements and every backup request', () => {
		const state = new ModsState();
		state.plan('server-1');
		state.apply('server-1', ['C:/srv/Mods/odd.pak']);
		state.loadBackups('server-1');
		state.restoreBackup('server-1', '20260101-000000-abc');
		state.restoreBackup('server-1', '20260101-000000-abc', ['C:/srv/Mods/odd.pak']);
		state.deleteBackup('server-1', '20260101-000000-abc');

		expect(send.mock.calls).toEqual([
			['profile_plan', { target_id: 'server-1' }],
			['profile_apply', { target_id: 'server-1', replace_occupants: ['C:/srv/Mods/odd.pak'] }],
			['mod_backup_list', { target_id: 'server-1' }],
			['mod_backup_restore', { target_id: 'server-1', set: '20260101-000000-abc' }],
			[
				'mod_backup_restore',
				{ target_id: 'server-1', set: '20260101-000000-abc', paths: ['C:/srv/Mods/odd.pak'] }
			],
			['mod_backup_delete', { target_id: 'server-1', set: '20260101-000000-abc' }]
		]);
	});

	it('installs with an options object and still accepts the boolean form', () => {
		const state = new ModsState();
		state.install('server-1', 'C:/a.zip', {
			acceptDefaults: true,
			enable: false,
			customName: 'Cool'
		});
		state.install('server-1', 'C:/a.zip', { customName: '  ' });
		state.install('server-1', 'C:/a.zip', true);

		expect(send.mock.calls).toEqual([
			[
				'mod_install',
				{
					target_id: 'server-1',
					path: 'C:/a.zip',
					accept_defaults: true,
					enable: false,
					custom_name: 'Cool'
				}
			],
			['mod_install', { target_id: 'server-1', path: 'C:/a.zip', accept_defaults: false }],
			['mod_install', { target_id: 'server-1', path: 'C:/a.zip', accept_defaults: true }]
		]);
	});

	it('reads enabled state from the active profile and treats an absent entry as off', () => {
		const state = new ModsState();
		state.profiles = {
			'server-1': [
				profile('inactive', false, [entry('cool-mod', true), entry('old-mod', true)]),
				profile('active', true, [entry('cool-mod', true), entry('off-mod', false)])
			]
		};

		expect(state.enabledOn('server-1', 'cool-mod')).toBe(true);
		expect(state.enabledOn('server-1', 'off-mod')).toBe(false);
		expect(state.enabledOn('server-1', 'old-mod')).toBe(false);
		expect(state.enabledOn('server-2', 'cool-mod')).toBe(false);
	});

	it('marks each busy flag its request owns', () => {
		const state = new ModsState();
		state.scan('server-1', { full: true });
		state.scan('server-2');
		state.restoreBackup('server-1', 'set');
		state.deleteBackup('server-2', 'set');
		state.analyze('server-1', 'C:/a.zip');
		state.setMod('server-1', 'cool-mod', true);

		expect(state.scanning['server-1']).toBe(true);
		expect(state.scanning['server-2']).toBeFalsy();
		expect(state.restoring['server-1']).toBe(true);
		expect(state.deleting).toEqual({ 'server-2': true });
		expect(state.analyzing).toEqual({ 'server-1': true });
		expect(state.settingMod['server-1']).toBe(true);
	});

	it('never downgrades a target from full scans to candidates-only ones', () => {
		const state = new ModsState();
		state.scan('server-1', { full: true });
		state.scan('server-1');
		state.scanOnce('server-2');
		send.mockReset();

		state.rescan('server-1');
		state.refreshScan('server-1');
		state.rescanAll();

		expect(send.mock.calls).toEqual([
			['mod_target_scan', { target_id: 'server-1', candidates_only: false }],
			['mod_target_scan', { target_id: 'server-1', candidates_only: false }],
			['mod_target_scan', { target_id: 'server-1', candidates_only: false }],
			['mod_target_scan', { target_id: 'server-2', candidates_only: true }]
		]);
	});

	it('records whether the latest scan reply was full and when a full one last arrived', () => {
		const state = new ModsState();
		vi.spyOn(Date, 'now').mockReturnValue(1000);
		state.recordScan('server-1', { files: [], candidates: [] }, true);
		expect(state.scanReplyFull['server-1']).toBe(true);
		expect(state.lastFullScanAt['server-1']).toBe(1000);
		expect(state.scans['server-1']).toBeDefined();

		vi.spyOn(Date, 'now').mockReturnValue(2000);
		state.recordScan('server-1', { candidates: [] }, false);
		expect(state.scanReplyFull['server-1']).toBe(false);
		expect(state.lastFullScanAt['server-1']).toBe(1000);

		state.recordScan('server-1', { files: [] }, true);
		state.refuseScan('server-1');
		expect(state.scanReplyFull['server-1']).toBe(false);
		expect(state.lastFullScanAt['server-1']).toBe(2000);
		vi.restoreAllMocks();
	});

	it('stamps an install record with the request it answers, oldest first', () => {
		const state = new ModsState();
		state.install('server-1', 'C:/a.zip');
		state.install('server-1', 'C:/a.zip');

		state.finishBusy('installing', 'server-1');
		state.recordInstall('server-1', { path: 'C:/a.zip', mod_id: 'a', version_id: 'v1' });
		expect(state.lastInstall['server-1']?.seq).toBe(1);

		state.finishBusy('installing', 'server-1');
		state.recordInstall('server-1', { path: 'C:/a.zip', mod_id: 'a', version_id: 'v2' });
		expect(state.lastInstall['server-1']?.seq).toBe(2);
	});

	it('stamps no request on an install record once every install was released', () => {
		const state = new ModsState();
		state.install('server-1', 'C:/a.zip');
		state.finishBusy('installing', undefined);
		state.recordInstall('server-1', { path: 'C:/a.zip', mod_id: 'a', version_id: 'v1' });
		expect(state.lastInstall['server-1']?.seq).toBe(0);
	});

	it('forgets install numbering for a removed target and on reset', () => {
		const state = new ModsState();
		state.install('a', 'C:/a.zip');
		state.install('b', 'C:/b.zip');

		state.forgetTarget('a');
		expect(state.installSeq).toEqual({ b: 1 });

		state.reset();
		expect(state.installSeq).toEqual({});
		state.install('b', 'C:/b.zip');
		state.finishBusy('installing', 'b');
		state.recordInstall('b', { path: 'C:/b.zip', mod_id: 'b', version_id: 'vb' });
		expect(state.lastInstall.b?.seq).toBe(1);
	});

	it('keeps refusals for different subjects of one type and target side by side', () => {
		const state = new ModsState();
		const refuse = (type: string, message: string, target: string | undefined, mod_id: string) =>
			state.recordRefusal(type, { code: 'db', message }, target, { subject: { mod_id } });
		refuse('profile_set_mod', 'a', 't', 'a');
		refuse('profile_set_mod', 'b', 't', 'b');
		refuse('mod_remove', 'remove a', undefined, 'a');
		refuse('mod_remove', 'remove b', undefined, 'b');

		expect(state.lastErrorFor('profile_set_mod', 't', { mod_id: 'a' })?.message).toBe('a');
		expect(state.lastErrorFor('profile_set_mod', 't', { mod_id: 'b' })?.message).toBe('b');
		expect(state.lastErrorFor('profile_set_mod', 't')?.message).toBe('b');
		expect(state.lastErrorsFor('mod_remove').map((error) => error.message)).toEqual([
			'remove a',
			'remove b'
		]);

		refuse('profile_set_mod', 'a again', 't', 'a');
		expect(state.lastErrorsFor('profile_set_mod', 't').map((error) => error.message)).toEqual([
			'b',
			'a again'
		]);

		state.clearLastError('profile_set_mod', 't', { mod_id: 'a' });
		expect(state.lastErrorFor('profile_set_mod', 't', { mod_id: 'a' })).toBeUndefined();
		expect(state.lastErrorFor('profile_set_mod', 't', { mod_id: 'b' })?.message).toBe('b');
		state.clearLastError('mod_remove');
		expect(state.lastErrorsFor('mod_remove')).toEqual([]);
	});

	it('drops adopt refusals whose candidate a scan reply no longer lists', () => {
		const state = new ModsState();
		const kept = {
			name: 'Kept',
			kind: 'ue4ss' as const,
			root: 'C:/P/Mods/Kept',
			files: [],
			enabled: true,
			source: 'local' as const
		};
		const gone = { candidate_name: 'Gone', source: 'local', root: 'C:/P/Mods/Gone' };
		state.recordRefusal('mod_adopt', { code: 'not_a_candidate', message: 'gone' }, 't', {
			subject: gone
		});
		state.recordRefusal('mod_adopt', { code: 'already_managed', message: 'kept' }, 't', {
			subject: { candidate_name: kept.name, source: kept.source, root: kept.root }
		});
		state.recordRefusal('mod_adopt', { code: 'unknown', message: 'bare' }, 't');
		state.recordRefusal('mod_adopt', { code: 'not_a_candidate', message: 'other' }, 'u', {
			subject: gone
		});

		state.recordScan('t', { candidates: [kept] }, false);

		expect(state.lastErrorsFor('mod_adopt', 't').map((error) => error.message)).toEqual(['kept']);
		expect(state.lastErrorsFor('mod_adopt', 'u')).toHaveLength(1);
	});

	it('numbers installs so a record from an earlier install never reads as the current one', () => {
		const state = new ModsState();
		state.install('server-1', 'C:/a.zip');
		expect(state.installSeq['server-1']).toBe(1);
		state.finishBusy('installing', 'server-1');
		state.recordInstall('server-1', { path: 'C:/a.zip', mod_id: 'a', version_id: 'va' });
		expect(state.lastInstall['server-1']).toEqual({
			seq: 1,
			path: 'C:/a.zip',
			mod_id: 'a',
			version_id: 'va'
		});

		state.install('server-1', 'C:/b.zip');
		expect(state.installSeq['server-1']).toBe(2);
		expect(state.lastInstall['server-1']).toBeUndefined();
		state.finishBusy('installing', 'server-1');
		state.recordInstall('server-1', {
			path: 'C:/b.zip',
			mod_id: 'b',
			version_id: 'vb',
			enable_error: { code: 'db', message: 'locked' }
		});
		expect(state.lastInstall['server-1']?.seq).toBe(2);
		expect(state.lastInstall['server-1']?.enable_error?.code).toBe('db');
	});

	it('stays busy until every in-flight request of a kind has replied', () => {
		const state = new ModsState();
		state.setMod('server-1', 'a', true);
		state.setMod('server-1', 'b', true);
		state.setMod('server-2', 'a', true);
		state.install('server-1', 'C:/a.zip');
		state.install('server-1', 'C:/b.zip');

		state.finishBusy('settingMod', 'server-1');
		state.finishBusy('installing', 'server-1');
		expect(state.settingMod['server-1']).toBe(true);
		expect(state.installing['server-1']).toBe(true);

		state.finishBusy('settingMod', 'server-1');
		state.finishBusy('settingMod', 'server-1');
		expect(state.settingMod).toEqual({ 'server-1': false, 'server-2': true });

		state.setMod('server-1', 'c', true);
		expect(state.settingMod['server-1']).toBe(true);
		state.finishBusy('settingMod', 'server-1');
		expect(state.settingMod['server-1']).toBe(false);

		state.finishBusy('settingMod', undefined);
		expect(state.settingMod).toEqual({});
		expect(state.installing['server-1']).toBe(true);
	});

	it('scans fully on request and rescans each target with the kind it last had', () => {
		const state = new ModsState();
		state.scan('server-1', { full: true });
		state.scan('server-2');
		send.mockReset();

		state.rescan('server-1');
		state.rescan('server-2');
		state.rescanAll();

		expect(send.mock.calls).toEqual([
			['mod_target_scan', { target_id: 'server-1', candidates_only: false }],
			['mod_target_scan', { target_id: 'server-2', candidates_only: true }],
			['mod_target_scan', { target_id: 'server-1', candidates_only: false }],
			['mod_target_scan', { target_id: 'server-2', candidates_only: true }]
		]);
	});

	it('plans every target whose plan is loaded or was refused', () => {
		const state = new ModsState();
		state.plans = {
			'server-1': { profile_id: 'a' } as TargetPlan,
			'server-2': { profile_id: 'b' } as TargetPlan
		};
		state.recordRefusal('profile_plan', { code: 'db', message: 'no' }, 'server-3');
		state.recordRefusal('profile_plan', { code: 'unknown', message: 'bare' });
		state.recordRefusal('profile_apply', { code: 'db', message: 'no' }, 'server-4');
		state.planLoaded();

		expect(send.mock.calls).toEqual([
			['profile_plan', { target_id: 'server-1' }],
			['profile_plan', { target_id: 'server-2' }],
			['profile_plan', { target_id: 'server-3' }]
		]);
	});
});

describe('ModsState connection and lifecycle', () => {
	function startEverything(state: ModsState, targetId: string): void {
		state.scan(targetId, { full: true });
		state.apply(targetId);
		state.install(targetId, 'C:/a.zip');
		state.adopt(targetId, {
			name: 'SteamPack',
			kind: 'workshop',
			root: 'C:/steam/1623730/3300000001',
			files: [],
			enabled: true,
			source: 'steam_subscribed'
		});
		state.restoreBackup(targetId, 'set');
		state.addTarget('__select__');
		state.setMod(targetId, 'cool-mod', true);
		state.progress = {
			r: { request_id: 'r', target_id: targetId, stage: 'applying', pct: 10, message: '' }
		};
	}

	it('keeps in-flight bookkeeping on the first connection', () => {
		const state = new ModsState();
		state.scan('server-1', { full: true });
		state.apply('server-1');

		state.connectionChanged(false);
		state.connectionChanged(true);

		expect(state.scanning['server-1']).toBe(true);
		expect(state.applying['server-1']).toBe(true);
		expect(state.completeScan('server-1')).toBe(true);
	});

	it('flips a stored verification to not live when the connection drops', () => {
		const state = new ModsState();
		state.connectionChanged(true);
		state.verification = {
			t1: {
				target_id: 't1',
				instance_id: 'saved:1',
				live: true,
				checked_at: '2026-09-01T10:00:00Z',
				build_info: null,
				resolution: { complete: true, missing: [] },
				status: []
			}
		};

		state.connectionChanged(false);

		expect(state.verification.t1.live).toBe(false);
		expect(state.verification.t1.instance_id).toBe('saved:1');
	});

	it('forgets in-flight bookkeeping when the connection drops but remembers answered scan kinds', () => {
		const state = new ModsState();
		state.connectionChanged(true);
		state.scan('server-1');
		state.completeScan('server-1');
		state.recordScan('server-1', { candidates: [] }, false);
		startEverything(state, 'server-1');
		state.analyze('server-1', 'C:/a.zip');
		state.deleteBackup('server-1', 'set');
		state.pending = { 'server-1': true };

		state.connectionChanged(true);
		expect(state.applying['server-1']).toBe(true);

		state.connectionChanged(false);

		expect(state.adopting).toEqual({});
		expect(state.applying).toEqual({});
		expect(state.installing).toEqual({});
		expect(state.scanning).toEqual({});
		expect(state.restoring).toEqual({});
		expect(state.analyzing).toEqual({});
		expect(state.deleting).toEqual({});
		expect(state.settingMod).toEqual({});
		expect(state.progress).toEqual({});
		expect(state.pending).toEqual({ 'server-1': true });

		send.mockReset();
		state.rescanAll();
		state.scanOnce('server-1');
		expect(send.mock.calls).toEqual([
			['mod_target_scan', { target_id: 'server-1', candidates_only: false }]
		]);
	});

	it('forgets requests sent during a remote gap once the remote transport reconnects', () => {
		const state = new ModsState();
		state.connectionChanged(true, 'remote');
		state.connectionChanged(false, 'remote');
		state.scan('server-2', { full: true });
		state.setMod('server-2', 'cool-mod', true);
		state.analyze('server-2', 'C:/a.zip');

		state.connectionChanged(true, 'remote');

		expect(state.scanning).toEqual({});
		expect(state.settingMod).toEqual({});
		expect(state.analyzing).toEqual({});
		expect(state.completeScan('server-2')).toBe(false);
	});

	it('keeps in-flight bookkeeping on the first remote connection', () => {
		const state = new ModsState();
		state.setMod('server-1', 'cool-mod', true);

		state.connectionChanged(false, 'remote');
		state.connectionChanged(true, 'remote');

		expect(state.settingMod['server-1']).toBe(true);
	});

	it('reloads after every reconnect but not on the first connection', () => {
		const state = new ModsState();
		state.connectionChanged(false);
		state.connectionChanged(true);
		expect(state.resets).toBe(0);

		state.connectionChanged(false);
		expect(state.resets).toBe(0);
		state.connectionChanged(true);
		expect(state.resets).toBe(1);

		state.targetsLoaded = true;
		state.connectionChanged(false, 'remote');
		state.connectionChanged(true, 'remote');
		expect(state.resets).toBe(2);
		expect(state.targetsLoaded).toBe(true);
	});

	it('scans again after a remote reconnect when the scan sent during the gap was never answered', () => {
		const state = new ModsState();
		state.connectionChanged(true, 'remote');
		state.connectionChanged(false, 'remote');
		state.scanOnce('server-2');
		state.connectionChanged(true, 'remote');
		send.mockReset();

		state.scanOnce('server-2');

		expect(send.mock.calls).toEqual([
			['mod_target_scan', { target_id: 'server-2', candidates_only: true }]
		]);
	});

	it('forgets a scan a drop left unanswered and keeps one answered earlier', () => {
		const state = new ModsState();
		state.connectionChanged(true);
		state.scanOnce('server-1');
		state.scanOnce('server-2');
		state.completeScan('server-1');
		state.recordScan('server-1', { candidates: [] }, false);
		state.scan('server-1', { full: true });

		state.connectionChanged(false);
		state.connectionChanged(true);
		send.mockReset();
		state.scanOnce('server-1');
		state.scanOnce('server-2');
		state.rescan('server-1');

		expect(send.mock.calls).toEqual([
			['mod_target_scan', { target_id: 'server-2', candidates_only: true }],
			['mod_target_scan', { target_id: 'server-1', candidates_only: false }]
		]);
	});

	it('repeats a full scan a drop left unanswered as full after the reconnect', () => {
		const state = new ModsState();
		state.connectionChanged(true);
		state.scanOnce('server-1');
		state.scan('server-1', { full: true });

		state.connectionChanged(false);
		state.connectionChanged(true);
		send.mockReset();
		state.refreshScan('server-1');
		state.scanOnce('server-1');

		expect(send.mock.calls).toEqual([
			['mod_target_scan', { target_id: 'server-1', candidates_only: false }]
		]);
	});

	it('resets when the transport changes identity, never on the first transport it sees', () => {
		const state = new ModsState();
		const remote = {};
		state.targetsLoaded = true;

		state.transportChanged(null);
		state.transportChanged(null);
		expect(state.resets).toBe(0);
		expect(state.targetsLoaded).toBe(true);

		state.transportChanged(remote);
		expect(state.resets).toBe(1);
		state.transportChanged(remote);
		expect(state.resets).toBe(1);
		state.transportChanged(null);
		expect(state.resets).toBe(2);
	});

	it('keeps a request sent while disconnected busy after the reconnect', () => {
		const state = new ModsState();
		state.connectionChanged(true);
		state.scan('server-1', { full: true });
		state.setMod('server-1', 'cool-mod', true);
		state.connectionChanged(false);

		state.scan('server-2', { full: true });
		state.setMod('server-2', 'cool-mod', true);
		state.connectionChanged(true);

		expect(state.scanning).toEqual({ 'server-2': true });
		expect(state.settingMod).toEqual({ 'server-2': true });
		expect(state.completeScan('server-1')).toBe(false);
		expect(state.completeScan('server-2')).toBe(true);
		state.finishBusy('settingMod', 'server-2');
		expect(state.settingMod['server-2']).toBe(false);
	});

	it('resets to an empty store', () => {
		const state = new ModsState();
		startEverything(state, 'server-1');
		state.targets = [target('server-1')];
		state.profiles = { 'server-1': [profile('p', true, [])] };
		state.plans = { 'server-1': { profile_id: 'p' } as TargetPlan };
		state.pending = { 'server-1': true };
		state.candidates = { 'server-1': [] };
		state.scanReplyFull = { 'server-1': true };
		state.lastFullScanAt = { 'server-1': 1 };
		state.lastInstall = {
			'server-1': { seq: 1, path: 'C:/a.zip', mod_id: 'cool-mod', version_id: 'v1' }
		};
		state.recordRefusal('mod_backup_list', { code: 'io', message: 'no' }, 'server-1');

		state.reset();
		send.mockReset();
		state.rescanAll();
		state.scanOnce('server-1');

		expect(state.targets).toEqual([]);
		expect(state.profiles).toEqual({});
		expect(state.plans).toEqual({});
		expect(state.pending).toEqual({});
		expect(state.candidates).toEqual({});
		expect(state.scanReplyFull).toEqual({});
		expect(state.lastFullScanAt).toEqual({});
		expect(state.lastInstall).toEqual({});
		expect(state.lastErrorFor('mod_backup_list', 'server-1')).toBeUndefined();
		expect(state.lastError).toEqual({});
		expect(state.progress).toEqual({});
		expect(state.scanning).toEqual({});
		expect(state.settingMod).toEqual({});
		expect(send.mock.calls).toEqual([
			['mod_target_scan', { target_id: 'server-1', candidates_only: true }]
		]);
	});

	it("forgets every record of a removed target and keeps the others'", () => {
		const state = new ModsState();
		for (const id of ['a', 'b']) {
			state.scan(id);
			state.setMod(id, 'cool-mod', true);
		}
		state.targets = [target('a'), target('b')];
		state.profiles = { a: [profile('pa', true, [], 'a')], b: [profile('pb', true, [], 'b')] };
		state.plans = { a: { profile_id: 'pa' } as TargetPlan, b: { profile_id: 'pb' } as TargetPlan };
		state.pending = { a: true, b: true };
		state.needsDecisions = { a: { path: 'x', decisions: [] }, b: { path: 'y', decisions: [] } };
		state.progress = {
			ra: { request_id: 'ra', target_id: 'a', stage: 'applying', pct: 1, message: '' },
			rb: { request_id: 'rb', target_id: 'b', stage: 'applying', pct: 1, message: '' }
		};
		state.recordRefusal('mod_backup_list', { code: 'io', message: 'a' }, 'a');
		state.recordRefusal('profile_plan', { code: 'io', message: 'b' }, 'b');
		state.recordRefusal('mod_list', { code: 'db', message: 'library' }, undefined);
		for (const id of ['a', 'b']) {
			state.analyze(id, 'C:/a.zip');
			state.deleteBackup(id, 'set');
		}
		state.scanReplyFull = { a: true, b: true };
		state.lastFullScanAt = { a: 1, b: 2 };
		state.lastInstall = {
			a: { seq: 1, path: 'x', mod_id: 'm', version_id: 'v' },
			b: { seq: 1, path: 'y', mod_id: 'm', version_id: 'v' }
		};
		send.mockReset();

		state.forgetTarget('a');
		state.rescanAll();

		expect(state.targets.map((entry) => entry.id)).toEqual(['b']);
		expect(Object.keys(state.profiles)).toEqual(['b']);
		expect(Object.keys(state.plans)).toEqual(['b']);
		expect(Object.keys(state.pending)).toEqual(['b']);
		expect(Object.keys(state.needsDecisions)).toEqual(['b']);
		expect(Object.keys(state.progress)).toEqual(['rb']);
		expect(Object.keys(state.settingMod)).toEqual(['b']);
		expect(Object.keys(state.analyzing)).toEqual(['b']);
		expect(Object.keys(state.deleting)).toEqual(['b']);
		expect(Object.keys(state.scanReplyFull)).toEqual(['b']);
		expect(Object.keys(state.lastFullScanAt)).toEqual(['b']);
		expect(Object.keys(state.lastInstall)).toEqual(['b']);
		expect(state.lastErrorFor('mod_backup_list', 'a')).toBeUndefined();
		expect(state.lastErrorFor('profile_plan', 'b')?.message).toBe('b');
		expect(state.lastErrorFor('mod_list')?.message).toBe('library');
		expect(send.mock.calls).toEqual([
			['mod_target_scan', { target_id: 'b', candidates_only: true }]
		]);
	});

	it("keeps each target's refusal of one type apart from another's", () => {
		const state = new ModsState();
		state.recordRefusal('profile_plan', { code: 'db', message: 'a' }, 'a');
		state.recordRefusal('profile_plan', { code: 'db', message: 'b' }, 'b');

		expect(state.lastErrorFor('profile_plan', 'a')?.message).toBe('a');
		expect(state.lastErrorFor('profile_plan', 'b')?.message).toBe('b');
		expect(state.lastErrorFor('profile_plan')).toBeUndefined();
	});

	it("clears a type's refusal only for its own target or when it named none", () => {
		const state = new ModsState();
		state.recordRefusal('mod_target_scan', { code: 'scan_failed', message: 'a' }, 'a');
		state.clearLastError('mod_target_scan', 'b');
		expect(state.lastErrorFor('mod_target_scan', 'a')?.target_id).toBe('a');
		state.clearLastError('mod_target_scan', 'a');
		expect(state.lastErrorFor('mod_target_scan', 'a')).toBeUndefined();

		state.recordRefusal('mod_list', { code: 'db', message: 'library' });
		state.clearLastError('mod_list', 'b');
		expect(state.lastErrorFor('mod_list')).toBeUndefined();

		state.recordRefusal('mod_remove', {
			code: 'apply_in_progress',
			message: 'busy',
			target_id: 'c'
		});
		state.clearLastError('mod_remove', 'b');
		expect(state.lastErrorFor('mod_remove')?.target_id).toBe('c');
		state.clearLastError('mod_remove');
		expect(state.lastErrorFor('mod_remove')).toBeUndefined();
	});

	it("clears a type's refusal only for the subject it named or when it named none", () => {
		const state = new ModsState();
		state.recordRefusal(
			'mod_version_set_current',
			{ code: 'subscribed_package', message: 'a', mod_id: 'workshop-1' },
			undefined,
			{ subject: { mod_id: 'a', version_id: 'va' } }
		);
		expect(state.lastErrorFor('mod_version_set_current')?.subject).toEqual({
			mod_id: 'a',
			version_id: 'va'
		});
		expect(state.lastErrorFor('mod_version_set_current')?.mod_id).toBe('workshop-1');
		state.clearLastError('mod_version_set_current', undefined, { mod_id: 'b', version_id: 'vb' });
		expect(state.lastErrorFor('mod_version_set_current')).toBeDefined();
		state.clearLastError('mod_version_set_current', undefined, { mod_id: 'a', version_id: 'v2' });
		expect(state.lastErrorFor('mod_version_set_current')).toBeUndefined();

		state.recordRefusal('mod_version_delete', { code: 'db', message: 'v' }, undefined, {
			subject: { version_id: 'v1' }
		});
		state.clearLastError('mod_version_delete', undefined, { version_id: 'v2' });
		expect(state.lastErrorFor('mod_version_delete')).toBeDefined();
		state.clearLastError('mod_version_delete');
		expect(state.lastErrorFor('mod_version_delete')).toBeUndefined();

		state.recordRefusal('mod_remove', { code: 'db', message: 'no subject' });
		state.clearLastError('mod_remove', undefined, { mod_id: 'b' });
		expect(state.lastErrorFor('mod_remove')).toBeUndefined();
	});

	it('reads a refusal for a subject only when the refusal named that subject', () => {
		const state = new ModsState();
		state.recordRefusal('profile_set_mod', { code: 'db', message: 'x' }, 't', {
			subject: { mod_id: 'a' }
		});
		state.recordRefusal('mod_remove', { code: 'db', message: 'y' });

		expect(state.lastErrorFor('profile_set_mod', 't', { mod_id: 'a' })?.message).toBe('x');
		expect(state.lastErrorFor('profile_set_mod', 't', { mod_id: 'b' })).toBeUndefined();
		expect(state.lastErrorFor('profile_set_mod', 'u', { mod_id: 'a' })).toBeUndefined();
		expect(state.lastErrorFor('mod_remove', undefined, { mod_id: 'a' })).toBeUndefined();
		expect(state.lastErrorFor('mod_remove')?.message).toBe('y');
	});

	it('tells adopt refusals apart by candidate name, source and root', () => {
		const state = new ModsState();
		const paks = { candidate_name: 'Foo.pak', source: 'local', root: 'C:/P/Paks/~mods/Foo.pak' };
		const logic = { ...paks, root: 'C:/P/Paks/LogicMods/Foo.pak' };
		state.recordRefusal('mod_adopt', { code: 'not_a_candidate', message: 'gone' }, 't', {
			subject: paks
		});

		expect(state.lastErrorFor('mod_adopt', 't', paks)?.code).toBe('not_a_candidate');
		expect(state.lastErrorFor('mod_adopt', 't', logic)).toBeUndefined();
		expect(
			state.lastErrorFor('mod_adopt', 't', { ...paks, source: 'steam_subscribed' })
		).toBeUndefined();

		state.clearLastError('mod_adopt', 't', logic);
		expect(state.lastErrorFor('mod_adopt', 't', paks)).toBeDefined();
		state.clearLastError('mod_adopt', 't', paks);
		expect(state.lastErrorFor('mod_adopt', 't')).toBeUndefined();
	});

	it('applies a saved selection to its profile and ignores targets not loaded', () => {
		const state = new ModsState();
		state.profiles = {
			t: [
				{
					id: 'p',
					target_id: 't',
					name: 'Default',
					is_active: true,
					is_default: true,
					mods: [
						{ profile_id: 'p', mod_id: 'a', mod_version_id: null, enabled: false, load_order: 3 }
					],
					ue4ss_control_mode: 'managed',
					force_order_ue4ss: false,
					force_order_palschema: false,
					created_at: '',
					updated_at: ''
				}
			]
		};

		state.recordSetMod('t', 'p', 'a', true, 'va');
		state.recordSetMod('t', 'p', 'b', true, null);
		state.recordSetMod('unloaded', 'p', 'a', true, null);

		expect(state.profiles.t[0].mods).toEqual([
			{ profile_id: 'p', mod_id: 'a', mod_version_id: 'va', enabled: true, load_order: 3 },
			{ profile_id: 'p', mod_id: 'b', mod_version_id: null, enabled: true, load_order: 4 }
		]);
		expect(state.profiles.unloaded).toBeUndefined();
	});
});

describe('ModsState target bookkeeping', () => {
	it('clears a stale detected list while detecting', () => {
		const state = new ModsState();
		state.detected = [
			{ root: 'C:/Old', platform: 'win64', ue4ss_mode: 'none', hazards: [], source: 'steam' }
		];

		state.detect();

		expect(state.detecting).toBe(true);
		expect(state.detected).toEqual([]);
		expect(send).toHaveBeenCalledWith('mod_target_detect', undefined);
	});

	it('counts resets and forgets that targets were loaded', () => {
		const state = new ModsState();
		state.targetsLoaded = true;
		state.detect();

		state.reset();

		expect(state.resets).toBe(1);
		expect(state.targetsLoaded).toBe(false);
		expect(state.detecting).toBe(false);
	});

	it('answers a pending add with nothing when the connection drops', () => {
		const state = new ModsState();
		state.connectionChanged(true);
		state.addTarget('__select__');
		state.detect();

		state.connectionChanged(false);

		expect(state.addReply).toEqual({ seq: 1, targetId: null });
		expect(state.detecting).toBe(false);
	});
});
