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
	mods_panel_applied: () => 'Mods applied',
	mods_panel_installed: ({ name }: { name: string }) => `Installed ${name}`,
	mods_panel_adopted: ({ name }: { name: string }) => `Now managing ${name}`
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

import { applyResult as baseApplyResult, noOps } from '$lib/components/mods/__tests__/fixtures';
import type { ApplyResult, ModsState } from '$lib/states/modsState.svelte';
import type { ServerState } from '$lib/states/serverState.svelte';
import type { TargetPlan } from '$types';
import {
	modAdoptHandler,
	modAnalyzeHandler,
	modBackupDeleteHandler,
	modBackupListHandler,
	modBackupRestoreHandler,
	modInstallHandler,
	modProgressHandler,
	modRemoveHandler,
	modTargetAddHandler,
	modTargetDetectHandler,
	modTargetListHandler,
	modTargetRemoveHandler,
	modTargetScanHandler,
	modVersionDeleteHandler,
	modVersionSetCurrentHandler,
	profileApplyHandler,
	profilePlanHandler,
	profileSetModHandler
} from './modsHandler';

const context = { goto: vi.fn() };

function applyResult(overrides: Partial<ApplyResult> = {}): ApplyResult {
	return baseApplyResult({
		preserved: ['C:/srv/Mods/keep.pak'],
		needs_attention: [{ path: 'C:/srv/Mods/odd.pak', reason: 'drift' }],
		counts: { ...noOps, preserve: 1, add: 2 },
		...overrides
	});
}

let state: ModsState;

beforeEach(async () => {
	await import('$states');
	const { ModsState } = await import('$lib/states/modsState.svelte');
	const { ServerState } = await import('$lib/states/serverState.svelte');
	holder.state = new ModsState();
	holder.server = new ServerState();
	state = holder.state as ModsState;
	send.mockReset();
	toast.add.mockReset();
});

describe('an apply on a server whose mods have not finished moving', () => {
	const servers = () => holder.server as ServerState;
	const occupant = {
		code: 'unmanaged_occupant',
		message: 'in the way',
		paths: ['C:/srv/Mods/stray.pak']
	};

	it('resets the relocation card to plain pending once an apply closes cleanly', async () => {
		servers().setRelocation(7, applyResult({ target_id: 'server-7', error: occupant }), occupant);

		await profileApplyHandler.handle(
			applyResult({ target_id: 'server-7', request_id: 'req-2' }),
			context
		);

		expect(servers().relocationPending[7]).toBe(true);
		expect(servers().relocationError[7]).toBeUndefined();
	});

	it('resets it for the apply a mod toggle ran', async () => {
		servers().setRelocation(7, applyResult({ target_id: 'server-7', mid_apply: true }));

		await profileSetModHandler.handle(
			{
				target_id: 'server-7',
				mod_id: 'cool-mod',
				enabled: true,
				apply: applyResult({ target_id: 'server-7', request_id: 'req-2' })
			},
			context
		);

		expect(servers().relocationPending[7]).toBe(true);
	});

	it('keeps the card for an apply that stopped midway or was refused', async () => {
		const stored = applyResult({ target_id: 'server-7', error: occupant });
		servers().setRelocation(7, stored, occupant);

		await profileApplyHandler.handle(
			applyResult({ target_id: 'server-7', request_id: 'req-2', mid_apply: true }),
			context
		);
		await profileApplyHandler.handle(
			applyResult({ target_id: 'server-7', request_id: 'req-3', error: occupant }),
			context
		);

		expect(servers().relocationPending[7]).toEqual(stored);
		expect(servers().relocationError[7]).toEqual(occupant);
	});

	it('keeps what it knows once the mods have moved', async () => {
		const created = { code: 'container_create_failed', message: 'no such image' };
		const stored = applyResult({ target_id: 'server-7' });
		servers().setRelocation(7, stored, created, true);

		await profileApplyHandler.handle(
			applyResult({ target_id: 'server-7', request_id: 'req-2' }),
			context
		);

		expect(servers().relocationPending[7]).toEqual(stored);
		expect(servers().relocationError[7]).toEqual(created);
		expect(servers().relocationMoved[7]).toBe(true);
	});

	it('leaves other servers and client targets alone', async () => {
		const stored = applyResult({ target_id: 'server-8', error: occupant });
		servers().setRelocation(8, stored, occupant);

		await profileApplyHandler.handle(
			applyResult({ target_id: 'server-7', request_id: 'req-2' }),
			context
		);
		await profileApplyHandler.handle(
			applyResult({ target_id: 'client-abc', request_id: 'req-3' }),
			context
		);

		expect(servers().relocationPending[8]).toEqual(stored);
		expect(servers().relocationPending[7]).toBeUndefined();
	});
});

describe('modsHandler', () => {
	it('returns quietly on a cancelled dialog', async () => {
		state.installing = { 'server-1': true };
		await modInstallHandler.handle(
			{ target_id: 'server-1', path: '__select__', canceled: true },
			context
		);
		expect(state.installing['server-1']).toBe(false);
		expect(toast.add).not.toHaveBeenCalled();
		expect(send).not.toHaveBeenCalled();
	});

	it('records a refusal by code and toasts its message', async () => {
		state.applying = { 'server-1': true };
		await profileApplyHandler.handle(
			{
				target_id: 'server-1',
				error: { code: 'target_not_found', message: 'no mod target server-1' }
			},
			context
		);
		expect(state.lastErrorFor('profile_apply', 'server-1')).toEqual({
			code: 'target_not_found',
			message: 'no mod target server-1',
			target_id: 'server-1'
		});
		expect(state.applying['server-1']).toBe(false);
		expect(toast.add).toHaveBeenCalledWith('no mod target server-1', 'Mods', 'error');
	});

	it('keeps a failed apply result so what needs attention stays visible, without a toast', async () => {
		const error = { code: 'target_locked', message: 'the server is running' };
		await profileApplyHandler.handle(applyResult({ error }), context);
		expect(state.lastApply['server-1'].needs_attention).toHaveLength(1);
		expect(state.lastErrorFor('profile_apply', 'server-1')?.code).toBe('target_locked');
		expect(toast.add).not.toHaveBeenCalled();
	});

	it('stores a successful apply and clears its progress', async () => {
		await modProgressHandler.handle(
			{ request_id: 'req-1', target_id: 'server-1', stage: 'applying', pct: 10, message: '' },
			context
		);
		expect(state.progressFor('server-1')?.stage).toBe('applying');
		state.applying = { 'server-1': true };
		state.pending = { 'server-1': true };

		await profileApplyHandler.handle(applyResult(), context);

		expect(state.lastApply['server-1'].preserved).toEqual(['C:/srv/Mods/keep.pak']);
		expect(state.progressFor('server-1')).toBeUndefined();
		expect(state.applying['server-1']).toBe(false);
		expect(state.pending['server-1']).toBe(false);
		expect(toast.add).toHaveBeenCalledWith('Mods applied', 'Mods', 'success');
	});

	it('refreshes the library, profiles and plan after a successful install, leaving the outcome to the install modal', async () => {
		await modInstallHandler.handle(
			{
				target_id: 'server-1',
				path: 'C:/cool.zip',
				mod_id: 'cool-mod',
				manifest: { display_name: 'Cool Mod' }
			},
			context
		);
		expect(send.mock.calls).toEqual([
			['mod_list', undefined],
			['profile_list', { target_id: 'server-1' }],
			['profile_plan', { target_id: 'server-1' }]
		]);
		expect(toast.add).not.toHaveBeenCalled();
	});

	it('drops the progress of a selection saved while the server runs', async () => {
		await modProgressHandler.handle(
			{ request_id: 'req-9', target_id: 'server-1', stage: 'checking', pct: 0, message: '' },
			context
		);
		await modProgressHandler.handle(
			{ request_id: 'req-8', target_id: 'server-2', stage: 'applying', pct: 10, message: '' },
			context
		);
		await profileSetModHandler.handle(
			{
				target_id: 'server-1',
				profile_id: 'server-1/default',
				mod_id: 'cool-mod',
				enabled: true,
				request_id: 'req-9',
				pending: true,
				apply: null
			},
			context
		);
		expect(state.pending['server-1']).toBe(true);
		expect(state.progressFor('server-1')).toBeUndefined();
		expect(state.progressFor('server-2')?.request_id).toBe('req-8');
	});

	it('lists the subscribed packages a scan offers apart from local candidates', async () => {
		await modTargetScanHandler.handle(
			{
				target_id: 'server-1',
				candidates: [
					{
						name: 'HandMade',
						kind: 'ue4ss',
						root: 'C:/srv/Mods/HandMade',
						files: [],
						enabled: true,
						source: 'local'
					},
					{
						name: 'SteamPack',
						kind: 'workshop',
						root: 'C:/steam/1623730/3300000001',
						files: [],
						enabled: false,
						source: 'steam_subscribed'
					}
				]
			},
			context
		);
		expect(state.subscribedCandidates('server-1').map((c) => c.name)).toEqual(['SteamPack']);
		expect(state.subscribedCandidates('server-2')).toEqual([]);
	});

	it('refreshes the library, profiles, scan and plan after adopting', async () => {
		state.adopting = { 'server-1': true };
		await modAdoptHandler.handle(
			{ target_id: 'server-1', candidate_name: 'SteamPack', mod_id: 'steampack-workshop' },
			context
		);
		expect(state.adopting['server-1']).toBe(false);
		expect(send.mock.calls).toEqual([
			['mod_list', undefined],
			['profile_list', { target_id: 'server-1' }],
			['mod_target_scan', { target_id: 'server-1', candidates_only: true }],
			['profile_plan', { target_id: 'server-1' }]
		]);
		expect(toast.add).toHaveBeenCalledWith('Now managing SteamPack', 'Mods', 'success');
	});

	it("clears a target's candidates and warnings when its scan is refused", async () => {
		state.candidates = {
			'server-1': [
				{
					name: 'SteamPack',
					kind: 'workshop',
					root: 'C:/steam/1623730/3300000001',
					files: [],
					enabled: true,
					source: 'steam_subscribed'
				}
			],
			'server-2': []
		};
		state.scanWarnings = { 'server-1': ['palmodsettings_unreadable'] };

		await modTargetScanHandler.handle(
			{ target_id: 'server-1', error: { code: 'scan_failed', message: 'layout broke' } },
			context
		);

		expect(state.candidates['server-1']).toBeUndefined();
		expect(state.candidates['server-2']).toEqual([]);
		expect(state.scanWarnings['server-1']).toBeUndefined();
		expect(state.lastErrorFor('mod_target_scan', 'server-1')?.message).toBe('layout broke');
		expect(toast.add).not.toHaveBeenCalled();
	});

	it('rescans every scanned target after a mod is removed', async () => {
		state.scan('server-1');
		send.mockReset();

		await modRemoveHandler.handle({ mod_id: 'workshop-3300000001', removed: true }, context);

		expect(send.mock.calls).toEqual([
			['mod_list', undefined],
			['mod_target_scan', { target_id: 'server-1', candidates_only: true }]
		]);
	});

	it('records a refused adoption against its target and refreshes nothing', async () => {
		state.adopting = { 'server-1': true };
		await modAdoptHandler.handle(
			{
				target_id: 'server-1',
				candidate_name: 'SteamPack',
				error: { code: 'not_a_candidate', message: 'SteamPack is not an adoption candidate' }
			},
			context
		);
		expect(state.adopting['server-1']).toBe(false);
		expect(state.lastErrorFor('mod_adopt', 'server-1')?.message).toBe(
			'SteamPack is not an adoption candidate'
		);
		expect(send).not.toHaveBeenCalled();
	});

	it('toasts a scan refusal that names no target, since no panel can show it', async () => {
		await modTargetScanHandler.handle(
			{ error: { code: 'db', message: 'locked' } } as never,
			context
		);

		expect(toast.add).toHaveBeenCalledWith('locked', 'Mods', 'error');
	});
});

describe('modsHandler replies to the remaining requests', () => {
	const backupSet = {
		name: '20260101-000000-abc',
		size_bytes: 10,
		entries: [{ original_path: 'C:/srv/Mods/odd.pak', backup_key: 'k', hash: 'h' }]
	};

	function scanReport(fileCount: number) {
		return {
			target_id: 'server-1',
			files: Array.from({ length: fileCount }, (_, index) => ({
				path: `C:/srv/Mods/${index}.pak`,
				kind: 'pak',
				state: 'unmanaged',
				mod_version_id: null
			})),
			candidates: [],
			drifted: [],
			missing: [],
			unreadable: [],
			warnings: []
		};
	}

	it('stores a refusal per type and target, keeps previous data and shows it only inline', async () => {
		state.backups = { 'server-1': [backupSet] };
		await modBackupListHandler.handle(
			{ target_id: 'server-1', error: { code: 'io', message: 'cannot read backups' } },
			context
		);

		expect(state.lastErrorFor('mod_backup_list', 'server-1')).toEqual({
			code: 'io',
			message: 'cannot read backups',
			target_id: 'server-1'
		});
		expect(state.lastErrorFor('mod_backup_list', 'server-2')).toBeUndefined();
		expect(state.lastErrorFor('mod_backup_list')).toBeUndefined();
		expect(state.backups['server-1']).toEqual([backupSet]);
		expect(toast.add).not.toHaveBeenCalled();
	});

	it("clears a target's restore and delete refusals once its backups list again", async () => {
		const error = { code: 'journal_open', message: 'an apply is open' };
		for (const target_id of ['server-1', 'server-2']) {
			await modBackupRestoreHandler.handle({ target_id, set: 's', error }, context);
			await modBackupDeleteHandler.handle({ target_id, set: 's', error }, context);
		}

		await modBackupListHandler.handle({ target_id: 'server-1', sets: [] }, context);

		expect(state.lastErrorFor('mod_backup_restore', 'server-1')).toBeUndefined();
		expect(state.lastErrorFor('mod_backup_delete', 'server-1')).toBeUndefined();
		expect(state.lastErrorFor('mod_backup_restore', 'server-2')?.code).toBe('journal_open');
		expect(state.lastErrorFor('mod_backup_delete', 'server-2')?.code).toBe('journal_open');
		expect(toast.add).not.toHaveBeenCalled();
	});

	it('keeps the target named in a library-wide refusal until that request succeeds', async () => {
		await modRemoveHandler.handle(
			{
				mod_id: 'cool-mod',
				error: { code: 'apply_in_progress', message: 'busy', target_id: 'server-4' }
			},
			context
		);
		expect(state.lastErrorFor('mod_remove')?.target_id).toBe('server-4');

		await modRemoveHandler.handle({ mod_id: 'cool-mod', removed: true }, context);
		expect(state.lastErrorFor('mod_remove')).toBeUndefined();
	});

	it('clears the stored refusal once the same request succeeds', async () => {
		await modBackupListHandler.handle(
			{ target_id: 'server-1', error: { code: 'io', message: 'no' } },
			context
		);
		await modBackupListHandler.handle({ target_id: 'server-1', sets: [backupSet] }, context);

		expect(state.lastErrorFor('mod_backup_list', 'server-1')).toBeUndefined();
		expect(state.backups['server-1']).toEqual([backupSet]);
	});

	it("keeps one target's refusal when the same request succeeds for another", async () => {
		await modTargetScanHandler.handle(
			{ target_id: 'server-1', error: { code: 'scan_failed', message: 'broke' } },
			context
		);
		await modTargetScanHandler.handle({ ...scanReport(0), target_id: 'server-2' }, context);
		expect(state.lastErrorFor('mod_target_scan', 'server-1')?.target_id).toBe('server-1');

		await modBackupListHandler.handle(
			{ target_id: 'server-2', error: { code: 'io', message: 'no' } },
			context
		);
		await modBackupListHandler.handle({ target_id: 'server-1', sets: [] }, context);
		expect(state.lastErrorFor('mod_backup_list', 'server-2')?.target_id).toBe('server-2');

		await modTargetScanHandler.handle(scanReport(0), context);
		expect(state.lastErrorFor('mod_target_scan', 'server-1')).toBeUndefined();
	});

	it("keeps each target's plan refusal when another target's plan is refused", async () => {
		const refuse = (target_id: string, message: string) =>
			profilePlanHandler.handle(
				{ target_id, error: { code: 'unmanaged_occupant', message, paths: [] } },
				context
			);
		await refuse('server-1', 'one');
		await refuse('server-2', 'two');

		expect(state.lastErrorFor('profile_plan', 'server-1')?.message).toBe('one');
		expect(state.lastErrorFor('profile_plan', 'server-2')?.message).toBe('two');
	});

	it('records a plan refusal without toasting and forgets the stale plan', async () => {
		const plan = { profile_id: 'p', counts: applyResult().counts, entries: [] } as TargetPlan;
		state.plans = { 'server-1': plan, 'server-2': plan };

		await profilePlanHandler.handle(
			{
				target_id: 'server-1',
				error: { code: 'unmanaged_occupant', message: 'occupied', paths: ['C:/srv/Mods/x.pak'] }
			},
			context
		);

		expect(state.lastErrorFor('profile_plan', 'server-1')?.code).toBe('unmanaged_occupant');
		expect(state.plans['server-1']).toBeUndefined();
		expect(state.plans['server-2']).toEqual(plan);
		expect(toast.add).not.toHaveBeenCalled();
	});

	it('normalises a bare string refusal and releases every restore it could answer', async () => {
		state.restoreBackup('server-1', 'set');
		state.restoreBackup('server-2', 'set');

		await modBackupRestoreHandler.handle({ error: 'Not available over a remote session' }, context);

		expect(state.restoring).toEqual({});
		expect(state.lastErrorFor('mod_backup_restore')).toEqual({
			code: 'unknown',
			message: 'Not available over a remote session',
			target_id: undefined
		});
		expect(toast.add).toHaveBeenCalledWith('Not available over a remote session', 'Mods', 'error');
	});

	it('releases the flags of installs, adoptions, toggles and applies refused without a target', async () => {
		state.install('server-1', 'C:/a.zip');
		state.adopt('server-2', {
			name: 'SteamPack',
			kind: 'workshop',
			root: 'C:/steam/1623730/3300000001',
			files: [],
			enabled: true,
			source: 'steam_subscribed'
		});
		state.setMod('server-3', 'cool-mod', true);
		state.apply('server-4');
		const error = { code: 'remote_denied', message: 'Not available over a remote session' };

		await modInstallHandler.handle({ error }, context);
		await modAdoptHandler.handle({ error }, context);
		await profileSetModHandler.handle({ error }, context);
		await profileApplyHandler.handle({ error }, context);

		expect(state.installing).toEqual({});
		expect(state.adopting).toEqual({});
		expect(state.settingMod).toEqual({});
		expect(state.applying).toEqual({});
	});

	it('keeps the target a remote refusal echoes', async () => {
		state.install('server-1', 'C:/a.zip');
		state.install('server-2', 'C:/b.zip');
		await modInstallHandler.handle(
			{
				target_id: 'server-1',
				path: 'C:/a.zip',
				error: { code: 'remote_denied', message: 'Not available over a remote session' }
			},
			context
		);

		expect(state.installing).toEqual({ 'server-1': false, 'server-2': true });
		expect(state.lastErrorFor('mod_install', 'server-1')?.target_id).toBe('server-1');
	});

	it('changes nothing on a cancelled folder pick except the busy flag', async () => {
		state.targets = [];
		state.recordRefusal('mod_target_add', { code: 'not_an_install', message: 'no' }, undefined);
		state.addTarget('__select__');
		send.mockReset();

		await modTargetAddHandler.handle({ root_path: '__select__', canceled: true }, context);

		expect(state.targets).toEqual([]);
		expect(state.lastErrorFor('mod_target_add')?.code).toBe('not_an_install');
		expect(toast.add).not.toHaveBeenCalled();
		expect(send).not.toHaveBeenCalled();
	});

	it('adds a registered target and stores detected installs', async () => {
		state.addTarget('__select__');
		const target = {
			id: 'client-abc',
			kind: 'client',
			server_id: null,
			name: 'Palworld',
			root_path: 'C:/Games/Palworld',
			platform: 'win64',
			ue4ss_mode: 'none',
			layout_overrides: null,
			detected: null,
			last_scanned_at: null,
			layout: null
		};
		await modTargetAddHandler.handle({ target }, context);
		await modTargetDetectHandler.handle(
			{
				detected: [
					{
						root: 'D:/Palworld',
						platform: 'win64',
						ue4ss_mode: 'none',
						hazards: [],
						source: 'steam'
					}
				]
			},
			context
		);

		expect(state.targets.map((entry) => entry.id)).toEqual(['client-abc']);
		expect(state.detected.map((entry) => entry.root)).toEqual(['D:/Palworld']);
	});

	it('forgets a removed target in the scan cache', async () => {
		state.targets = [
			{
				id: 'client-abc',
				kind: 'client',
				server_id: null,
				name: 'Palworld',
				root_path: 'C:/Games/Palworld',
				platform: 'win64',
				ue4ss_mode: 'none',
				layout_overrides: null,
				detected: null,
				last_scanned_at: null,
				layout: null
			}
		];
		state.scan('client-abc');
		state.candidates = { 'client-abc': [] };
		state.scanWarnings = { 'client-abc': ['palmodsettings_unreadable'] };
		state.plans = { 'client-abc': { profile_id: 'p' } as TargetPlan };
		send.mockReset();

		await modTargetRemoveHandler.handle({ target_id: 'client-abc', removed: true }, context);
		state.rescanAll();

		expect(state.targets).toEqual([]);
		expect(state.candidates['client-abc']).toBeUndefined();
		expect(state.scanWarnings['client-abc']).toBeUndefined();
		expect(state.plans).toEqual({});
		expect(send).not.toHaveBeenCalled();
	});

	it('stores a full scan report and leaves it alone for a candidates-only scan', async () => {
		state.scan('server-1', { full: true });
		expect(state.scanning['server-1']).toBe(true);
		await modTargetScanHandler.handle(scanReport(1), context);
		expect(state.scanning['server-1']).toBe(false);
		expect(state.scans['server-1'].files).toHaveLength(1);

		state.scan('server-1');
		await modTargetScanHandler.handle(
			{ ...scanReport(0), warnings: ['palmodsettings_unreadable'] },
			context
		);
		expect(state.scans['server-1'].files).toHaveLength(1);
		expect(state.scanWarnings['server-1']).toEqual(['palmodsettings_unreadable']);
	});

	it('answers in-flight scans oldest first and stays scanning while a full scan is pending', async () => {
		state.scan('server-1');
		state.scan('server-1', { full: true });
		state.scan('server-1', { full: true });

		await modTargetScanHandler.handle(scanReport(0), context);
		expect(state.scans['server-1']).toBeUndefined();
		expect(state.scanning['server-1']).toBe(true);

		await modTargetScanHandler.handle(scanReport(1), context);
		expect(state.scans['server-1'].files).toHaveLength(1);
		expect(state.scanning['server-1']).toBe(true);

		await modTargetScanHandler.handle(scanReport(2), context);
		expect(state.scans['server-1'].files).toHaveLength(2);
		expect(state.scanning['server-1']).toBe(false);
	});

	it('stores an analysis and a plan', async () => {
		const manifest = {
			folder_name: 'CoolMod',
			display_name: 'Cool Mod',
			mod_type: 'ue4ss',
			version: '1.0',
			routes: [],
			decisions: [],
			platform_filtered: null,
			source: {}
		};
		await modAnalyzeHandler.handle(
			{ target_id: 'server-1', path: 'C:/cool.zip', manifest },
			context
		);
		await profilePlanHandler.handle(
			{
				target_id: 'server-1',
				profile_id: 'server-1/default',
				entries: [{ op: 'remove', path: 'C:/srv/Mods/old.pak' }],
				counts: { ...applyResult().counts, remove: 1 }
			},
			context
		);

		expect(state.analysis['server-1'].path).toBe('C:/cool.zip');
		expect(state.plans['server-1'].counts.remove).toBe(1);
		expect(state.plans['server-1'].entries).toHaveLength(1);
	});

	it('clears restoring, keeps what was skipped and reloads backups and plan after a restore', async () => {
		state.restoreBackup('server-1', backupSet.name);
		send.mockReset();

		await modBackupRestoreHandler.handle(
			{
				target_id: 'server-1',
				set: backupSet.name,
				restored: [],
				skipped: [{ path: 'C:/srv/Mods/odd.pak', reason: 'occupied' }]
			},
			context
		);

		expect(state.restoring['server-1']).toBe(false);
		expect(state.lastRestore['server-1'].skipped[0].reason).toBe('occupied');
		expect(send.mock.calls).toEqual([
			['mod_backup_list', { target_id: 'server-1' }],
			['profile_plan', { target_id: 'server-1' }]
		]);
	});

	it('reloads backups after a delete and the library after a version change', async () => {
		await modBackupDeleteHandler.handle(
			{ target_id: 'server-1', set: backupSet.name, removed: true },
			context
		);
		await modVersionSetCurrentHandler.handle({ mod_id: 'cool-mod', version_id: 'v2' }, context);
		await modVersionDeleteHandler.handle({ version_id: 'v1', removed: true }, context);

		expect(send.mock.calls).toEqual([
			['mod_backup_list', { target_id: 'server-1' }],
			['mod_list', undefined],
			['mod_list', undefined]
		]);
	});

	it('plans every loaded target again after a library-wide change', async () => {
		state.plans = {
			'server-1': { profile_id: 'a' } as TargetPlan,
			'client-abc': { profile_id: 'b' } as TargetPlan
		};
		const planned = [
			['profile_plan', { target_id: 'server-1' }],
			['profile_plan', { target_id: 'client-abc' }]
		];

		await modVersionSetCurrentHandler.handle({ mod_id: 'cool-mod', version_id: 'v2' }, context);
		expect(send.mock.calls).toEqual([['mod_list', undefined], ...planned]);

		send.mockReset();
		await modVersionDeleteHandler.handle({ version_id: 'v1', removed: true }, context);
		expect(send.mock.calls).toEqual([['mod_list', undefined], ...planned]);

		send.mockReset();
		await modRemoveHandler.handle({ mod_id: 'cool-mod', removed: true }, context);
		expect(send.mock.calls).toEqual([['mod_list', undefined], ...planned]);
	});

	it('plans again after a selection change and after an apply', async () => {
		state.setMod('server-1', 'cool-mod', true);
		send.mockReset();

		await profileSetModHandler.handle(
			{
				target_id: 'server-1',
				profile_id: 'server-1/default',
				mod_id: 'cool-mod',
				enabled: true,
				mod_version_id: null,
				request_id: 'req-3',
				pending: true,
				apply: null
			},
			context
		);
		await profileApplyHandler.handle(applyResult({ request_id: 'req-4' }), context);

		expect(state.settingMod['server-1']).toBe(false);
		expect(state.pending['server-1']).toBe(false);
		expect(send.mock.calls).toEqual([
			['profile_list', { target_id: 'server-1' }],
			['profile_plan', { target_id: 'server-1' }],
			['profile_plan', { target_id: 'server-1' }]
		]);
	});

	it('keeps toggles busy until the second of two replies', async () => {
		state.setMod('server-1', 'a', true);
		state.setMod('server-1', 'b', true);
		const reply = {
			target_id: 'server-1',
			profile_id: 'server-1/default',
			enabled: true,
			mod_version_id: null,
			request_id: 'req',
			pending: false,
			apply: null
		};

		await profileSetModHandler.handle({ ...reply, mod_id: 'a' }, context);
		expect(state.settingMod['server-1']).toBe(true);
		await profileSetModHandler.handle({ ...reply, mod_id: 'b' }, context);
		expect(state.settingMod['server-1']).toBe(false);
	});

	it('marks the selection pending when an embedded apply finds another apply running', async () => {
		await profileSetModHandler.handle(
			{
				target_id: 'server-1',
				profile_id: 'server-1/default',
				mod_id: 'cool-mod',
				enabled: true,
				mod_version_id: null,
				request_id: 'req-5',
				pending: true,
				apply: null
			},
			context
		);
		expect(state.pending['server-1']).toBe(true);
		expect(state.lastApply['server-1']).toBeUndefined();
	});

	it('records the mod a refusal names, keeps it through another mod succeeding and does not toast', async () => {
		await modVersionSetCurrentHandler.handle(
			{
				mod_id: 'a',
				version_id: 'va',
				error: { code: 'subscribed_package', message: 'steam', mod_id: 'workshop-1' }
			},
			context
		);
		expect(state.lastErrorFor('mod_version_set_current')?.subject).toEqual({
			mod_id: 'a',
			version_id: 'va'
		});
		expect(state.lastErrorFor('mod_version_set_current')?.mod_id).toBe('workshop-1');
		expect(toast.add).not.toHaveBeenCalled();

		await modVersionSetCurrentHandler.handle({ mod_id: 'b', version_id: 'vb' }, context);
		expect(state.lastErrorFor('mod_version_set_current')).toBeDefined();

		await modVersionSetCurrentHandler.handle({ mod_id: 'a', version_id: 'va' }, context);
		expect(state.lastErrorFor('mod_version_set_current')).toBeUndefined();
	});

	it('keeps a version delete refusal until that version is deleted', async () => {
		await modVersionDeleteHandler.handle(
			{ version_id: 'v1', error: { code: 'version_in_use', message: 'in use' } },
			context
		);
		await modVersionDeleteHandler.handle({ version_id: 'v2', removed: true }, context);
		expect(state.lastErrorFor('mod_version_delete')?.subject).toEqual({ version_id: 'v1' });

		await modVersionDeleteHandler.handle({ version_id: 'v1', removed: true }, context);
		expect(state.lastErrorFor('mod_version_delete')).toBeUndefined();
		expect(toast.add).not.toHaveBeenCalled();
	});

	it('does not toast toggle or removal refusals the mod list shows inline', async () => {
		await profileSetModHandler.handle(
			{
				target_id: 'server-1',
				mod_id: 'a',
				enabled: true,
				error: { code: 'not_subscribed_on_target', message: 'missing' }
			},
			context
		);
		await modRemoveHandler.handle(
			{ mod_id: 'a', error: { code: 'version_in_use', message: 'in use' } },
			context
		);
		expect(state.lastErrorFor('profile_set_mod', 'server-1')?.subject).toEqual({ mod_id: 'a' });
		expect(toast.add).not.toHaveBeenCalled();
	});

	it('shows a saved selection at once, before the profile list reloads', async () => {
		state.profiles = {
			'server-1': [
				{
					id: 'server-1/default',
					target_id: 'server-1',
					name: 'Default',
					is_active: true,
					is_default: true,
					mods: [],
					ue4ss_control_mode: 'managed',
					force_order_ue4ss: false,
					force_order_palschema: false,
					created_at: '',
					updated_at: ''
				}
			]
		};

		await profileSetModHandler.handle(
			{
				target_id: 'server-1',
				profile_id: 'server-1/default',
				mod_id: 'cool-mod',
				enabled: true,
				mod_version_id: null,
				request_id: 'req-6',
				pending: false,
				apply: null
			},
			context
		);

		expect(state.enabledOn('server-1', 'cool-mod')).toBe(true);
	});

	it('clears a finished progress entry', async () => {
		await modProgressHandler.handle(
			{ request_id: 'req-6', target_id: 'server-1', stage: 'applying', pct: 50, message: '' },
			context
		);
		await modProgressHandler.handle(
			{ request_id: 'req-6', target_id: 'server-1', stage: 'done', pct: 100, message: 'done' },
			context
		);
		expect(state.progress).toEqual({});
	});

	it('does not toast analyze or install refusals the install modal shows, but still toasts untargeted ones', async () => {
		await modAnalyzeHandler.handle(
			{
				target_id: 'server-1',
				path: 'C:/x.zip',
				error: { code: 'extract_failed', message: 'bad' }
			},
			context
		);
		await modInstallHandler.handle(
			{
				target_id: 'server-1',
				path: 'C:/x.zip',
				error: { code: 'already_installed', message: 'dup' }
			},
			context
		);

		expect(toast.add).not.toHaveBeenCalled();
		expect(state.lastErrorFor('mod_analyze', 'server-1')?.code).toBe('extract_failed');
		expect(state.lastErrorFor('mod_install', 'server-1')?.code).toBe('already_installed');

		await modInstallHandler.handle({ error: 'Not available over a remote session' }, context);

		expect(toast.add).toHaveBeenCalledWith('Not available over a remote session', 'Mods', 'error');
	});

	it('records an install that succeeded but could not be enabled, without a toast or a plan', async () => {
		state.recordRefusal('mod_install', { code: 'io', message: 'old' }, 'server-1');
		state.install('server-1', 'C:/cool.zip');
		send.mockReset();
		const enableError = { code: 'not_supported_on_target', message: 'no workshop here' };
		await modInstallHandler.handle(
			{
				target_id: 'server-1',
				path: 'C:/cool.zip',
				mod_id: 'cool-mod',
				version_id: 'v1',
				manifest: { display_name: 'Cool Mod' },
				enable_error: enableError
			},
			context
		);
		expect(state.lastErrorFor('mod_install', 'server-1')).toBeUndefined();
		expect(state.lastInstall['server-1']).toEqual({
			seq: 1,
			path: 'C:/cool.zip',
			mod_id: 'cool-mod',
			version_id: 'v1',
			enable_error: enableError
		});
		expect(toast.add).not.toHaveBeenCalled();
		expect(send.mock.calls).toEqual([
			['mod_list', undefined],
			['profile_list', { target_id: 'server-1' }]
		]);
	});

	it('records no install on decisions or a refusal', async () => {
		state.install('server-1', 'C:/cool.zip');
		await modInstallHandler.handle(
			{ target_id: 'server-1', path: 'C:/cool.zip', needs_decisions: [] },
			context
		);
		await modInstallHandler.handle(
			{ target_id: 'server-1', path: 'C:/cool.zip', error: { code: 'io', message: 'no' } },
			context
		);
		expect(state.lastInstall).toEqual({});
	});

	it('stops analyzing on every analyze reply, a cancelled pick and a refusal included', async () => {
		state.analyze('server-1', '__select__');
		await modAnalyzeHandler.handle({ target_id: 'server-1', path: '', canceled: true }, context);
		expect(state.analyzing['server-1']).toBe(false);

		state.analyze('server-1', 'C:/a.zip');
		await modAnalyzeHandler.handle(
			{ target_id: 'server-1', path: 'C:/a.zip', error: { code: 'io', message: 'no' } },
			context
		);
		expect(state.analyzing['server-1']).toBe(false);

		state.analyze('server-1', 'C:/a.zip');
		state.analyze('server-2', 'C:/b.zip');
		await modAnalyzeHandler.handle({ error: 'Not available over a remote session' }, context);
		expect(state.analyzing).toEqual({});
	});

	it('stops deleting when a delete replies', async () => {
		state.deleteBackup('server-1', 'set');
		state.deleteBackup('server-2', 'set');
		await modBackupDeleteHandler.handle(
			{ target_id: 'server-1', set: 'set', error: { code: 'journal_open', message: 'open' } },
			context
		);
		expect(state.deleting).toEqual({ 'server-1': false, 'server-2': true });
		await modBackupDeleteHandler.handle(
			{ target_id: 'server-2', set: 'set', removed: true },
			context
		);
		expect(state.deleting['server-2']).toBe(false);
	});

	it('records whether the latest scan reply was full, and when the last full one arrived', async () => {
		vi.spyOn(Date, 'now').mockReturnValue(5000);
		state.scan('server-1', { full: true });
		await modTargetScanHandler.handle(scanReport(1), context);
		expect(state.scanReplyFull['server-1']).toBe(true);
		expect(state.lastFullScanAt['server-1']).toBe(5000);

		state.scan('server-1');
		await modTargetScanHandler.handle(scanReport(0), context);
		expect(state.scanReplyFull['server-1']).toBe(false);
		expect(state.lastFullScanAt['server-1']).toBe(5000);

		state.scan('server-1', { full: true });
		await modTargetScanHandler.handle(
			{ target_id: 'server-1', error: { code: 'scan_failed', message: 'broke' } },
			context
		);
		expect(state.scanReplyFull['server-1']).toBe(false);
		vi.restoreAllMocks();
	});

	it('attributes an adopt refusal to the candidate it names and shows it only inline', async () => {
		const candidate = {
			target_id: 'server-1',
			candidate_name: 'Foo.pak',
			source: 'local',
			root: 'C:/srv/Paks/~mods/Foo.pak'
		};
		state.adopting = { 'server-1': true };
		await modAdoptHandler.handle(
			{ ...candidate, error: { code: 'not_a_candidate', message: 'gone' } },
			context
		);

		const subject = { candidate_name: 'Foo.pak', source: 'local', root: candidate.root };
		expect(state.lastErrorFor('mod_adopt', 'server-1', subject)?.code).toBe('not_a_candidate');
		expect(
			state.lastErrorFor('mod_adopt', 'server-1', { ...subject, root: 'C:/srv/LogicMods/Foo.pak' })
		).toBeUndefined();
		expect(toast.add).not.toHaveBeenCalled();
	});
});

describe('modsHandler target add, list and detect replies', () => {
	const added = {
		id: 'client-abc',
		kind: 'client' as const,
		server_id: null,
		name: 'Palworld',
		root_path: 'C:/Games/Palworld',
		platform: 'win64',
		ue4ss_mode: 'none',
		layout_overrides: null,
		detected: null,
		last_scanned_at: null,
		layout: null
	};

	it('numbers every add reply and names the target only on success', async () => {
		await modTargetAddHandler.handle({ target: added }, context);
		expect(state.addReply).toEqual({ seq: 1, targetId: 'client-abc' });

		await modTargetAddHandler.handle({ root_path: '__select__', canceled: true }, context);
		expect(state.addReply).toEqual({ seq: 2, targetId: null });

		await modTargetAddHandler.handle(
			{ root_path: 'E:/X', error: { code: 'not_an_install', message: 'no' } },
			context
		);
		expect(state.addReply).toEqual({ seq: 3, targetId: null });
	});

	it('records an add refusal inline only', async () => {
		await modTargetAddHandler.handle(
			{ root_path: 'E:/X', error: { code: 'not_an_install', message: 'no' } },
			context
		);

		expect(state.lastErrorFor('mod_target_add')?.code).toBe('not_an_install');
		expect(toast.add).not.toHaveBeenCalled();
	});

	it('marks targets loaded once a list arrives', async () => {
		expect(state.targetsLoaded).toBe(false);
		await modTargetListHandler.handle({ targets: [added] }, context);
		expect(state.targetsLoaded).toBe(true);
	});

	it('stops detecting on any detect reply', async () => {
		state.detect();
		expect(state.detecting).toBe(true);
		await modTargetDetectHandler.handle({ error: { code: 'io', message: 'no' } }, context);
		expect(state.detecting).toBe(false);

		state.detect();
		await modTargetDetectHandler.handle({ detected: [] }, context);
		expect(state.detecting).toBe(false);
	});
});
