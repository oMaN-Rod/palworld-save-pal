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
import type { ExportReply, ImportReply, ModProfile } from '$types';
import { profileExportHandler, profileImportHandler } from './modsHandler';

const context = { goto: vi.fn() };
let state: ModsState;

const imported: ModProfile = {
	id: 'client-a/default-2',
	target_id: 'client-a',
	name: 'Default (2)',
	is_active: false,
	is_default: false,
	mods: [],
	ue4ss_control_mode: 'enabled_txt',
	force_order_ue4ss: false,
	force_order_palschema: false,
	created_at: '',
	updated_at: ''
};

function importReply(overrides: Partial<ImportReply> = {}): ImportReply {
	return {
		target_id: 'client-a',
		path: 'C:/p/Default.psmods',
		profile: imported,
		pinned: [],
		following_current: [],
		installed: [],
		missing: [],
		disabled: [],
		frameworks: [],
		...overrides
	};
}

const exported: ExportReply = {
	target_id: 'client-a',
	profile_id: 'client-a/default',
	path: 'C:/p/Default.psmods',
	entries: 0,
	archives_included: 0,
	missing_archives: [],
	unresolved: []
};

beforeEach(async () => {
	await import('$states');
	const { ModsState } = await import('$lib/states/modsState.svelte');
	holder.state = new ModsState();
	state = holder.state as ModsState;
	send.mockReset();
	toast.add.mockReset();
});

describe('export and import requests', () => {
	it('sends each with its wire payload and marks it busy', () => {
		state.exportProfile('client-a', 'client-a/default', true, '__select__');
		state.importProfile('client-a', '__select__');
		state.importProfile('client-a', 'C:/p/x.psmods', '  Shared  ');
		state.importProfile('client-a', 'C:/p/x.psmods', '   ');

		expect(send.mock.calls).toEqual([
			[
				'profile_export',
				{
					target_id: 'client-a',
					profile_id: 'client-a/default',
					include_archives: true,
					path: '__select__'
				}
			],
			['profile_import', { target_id: 'client-a', path: '__select__' }],
			['profile_import', { target_id: 'client-a', path: 'C:/p/x.psmods', name: 'Shared' }],
			['profile_import', { target_id: 'client-a', path: 'C:/p/x.psmods' }]
		]);
		expect(state.exporting['client-a']).toBe(true);
		expect(state.importing['client-a']).toBe(true);
	});

	it('forgets busy sharing on a drop and results on reset', () => {
		state.connectionChanged(true);
		state.exportProfile('client-a', 'p', false, '__select__');
		state.importProfile('client-a', '__select__');
		state.lastImport = { 'client-a': importReply() };

		state.connectionChanged(false);
		expect(state.exporting).toEqual({});
		expect(state.importing).toEqual({});

		state.reset();
		expect(state.lastImport).toEqual({});
		expect(state.lastExport).toEqual({});
	});

	it('forgets everything about sharing on a removed target', () => {
		state.exportProfile('client-a', 'p', false, '__select__');
		state.importProfile('client-a', '__select__');
		state.lastExport = { 'client-a': exported };
		state.lastImport = { 'client-a': importReply() };

		state.forgetTarget('client-a');

		expect(state.exporting).toEqual({});
		expect(state.importing).toEqual({});
		expect(state.lastExport).toEqual({});
		expect(state.lastImport).toEqual({});
	});

	it('drops the previous result and refusal when sending again', () => {
		state.lastExport = { 'client-a': exported };
		state.lastImport = { 'client-a': importReply() };
		state.recordRefusal('profile_export', { code: 'invalid_path', message: 'no' }, 'client-a');
		state.recordRefusal('profile_import', { code: 'invalid_archive', message: 'no' }, 'client-a');

		state.exportProfile('client-a', 'p', false, '__select__');
		state.importProfile('client-a', '__select__');

		expect(state.lastExport['client-a']).toBeUndefined();
		expect(state.lastImport['client-a']).toBeUndefined();
		expect(state.lastErrorFor('profile_export', 'client-a')).toBeUndefined();
		expect(state.lastErrorFor('profile_import', 'client-a')).toBeUndefined();
	});
});

describe('profile_export', () => {
	it('stores the result', async () => {
		state.exportProfile('client-a', 'client-a/default', true, '__select__');
		await profileExportHandler.handle(
			{
				target_id: 'client-a',
				profile_id: 'client-a/default',
				path: 'C:/p/Default.psmods',
				entries: 2,
				archives_included: 1,
				missing_archives: ['m@1'],
				unresolved: []
			},
			context
		);
		expect(state.exporting['client-a']).toBe(false);
		expect(state.lastExport['client-a'].entries).toBe(2);
	});

	it('does nothing but release the busy flag for a cancelled dialog', async () => {
		state.exportProfile('client-a', 'client-a/default', true, '__select__');
		await profileExportHandler.handle(
			{ target_id: 'client-a', profile_id: 'client-a/default', canceled: true },
			context
		);
		expect(state.exporting['client-a']).toBe(false);
		expect(state.lastExport['client-a']).toBeUndefined();
		expect(state.lastErrorFor('profile_export', 'client-a')).toBeUndefined();
	});

	it('records a refusal without a toast', async () => {
		state.exportProfile('client-a', 'client-a/default', true, 'C:/nope/x.psmods');
		await profileExportHandler.handle(
			{
				target_id: 'client-a',
				profile_id: 'client-a/default',
				error: { code: 'invalid_path', message: 'no' }
			},
			context
		);
		expect(state.exporting['client-a']).toBe(false);
		expect(state.lastErrorFor('profile_export', 'client-a')?.code).toBe('invalid_path');
		expect(toast.add).not.toHaveBeenCalled();
	});
});

describe('profile_import', () => {
	it('stores the result, adds and views the new profile, and reloads the library when it installed mods', async () => {
		state.importProfile('client-a', '__select__');
		send.mockReset();

		await profileImportHandler.handle(
			importReply({ installed: [{ mod_id: 'cool', version_id: 'cool@1' }] }),
			context
		);

		expect(state.importing['client-a']).toBe(false);
		expect(state.lastImport['client-a'].profile.id).toBe('client-a/default-2');
		expect(state.viewedProfile('client-a')?.id).toBe('client-a/default-2');
		expect(send.mock.calls).toEqual([
			['profile_list', { target_id: 'client-a' }],
			['mod_list', undefined]
		]);
	});

	it('does not reload the library when nothing was installed', async () => {
		await profileImportHandler.handle(importReply(), context);
		expect(send).not.toHaveBeenCalledWith('mod_list', undefined);
	});

	it('does nothing but release the busy flag for a cancelled dialog', async () => {
		state.importProfile('client-a', '__select__');
		send.mockReset();

		await profileImportHandler.handle(
			{ target_id: 'client-a', path: '__select__', canceled: true },
			context
		);

		expect(state.importing['client-a']).toBe(false);
		expect(state.lastImport['client-a']).toBeUndefined();
		expect(state.lastErrorFor('profile_import', 'client-a')).toBeUndefined();
		expect(send).not.toHaveBeenCalled();
	});

	it('reloads profiles when a failed import left a profile behind', async () => {
		state.importProfile('client-a', 'C:/p/x.psmods');
		await profileImportHandler.handle(
			{
				target_id: 'client-a',
				path: 'C:/p/x.psmods',
				error: {
					code: 'import_failed',
					message: 'db',
					reason: 'disk full',
					profile_id: 'client-a/x'
				}
			},
			context
		);
		expect(state.importing['client-a']).toBe(false);
		expect(state.lastErrorFor('profile_import', 'client-a')?.code).toBe('import_failed');
		expect(send).toHaveBeenCalledWith('profile_list', { target_id: 'client-a' });
		expect(toast.add).not.toHaveBeenCalled();
	});

	it('leaves profiles alone when a refused import created nothing', async () => {
		state.importProfile('client-a', 'C:/p/x.psmods');
		send.mockReset();
		await profileImportHandler.handle(
			{
				target_id: 'client-a',
				path: 'C:/p/x.psmods',
				error: { code: 'invalid_archive', message: 'no' }
			},
			context
		);
		expect(state.importing['client-a']).toBe(false);
		expect(send).not.toHaveBeenCalled();
		expect(toast.add).not.toHaveBeenCalled();
	});
});
