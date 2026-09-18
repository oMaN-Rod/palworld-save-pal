// @vitest-environment jsdom
import type { ApplyResult as ApplyReply, LibraryMod, ModError } from '$types';
import { MessageType } from '$types';
import { fireEvent, render, screen } from '@testing-library/svelte';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import type { ModsTab } from '../TargetPanel.svelte';

const { holder, modal, send, remote } = vi.hoisted(() => ({
	holder: { state: undefined as unknown },
	modal: { showModal: vi.fn(), showConfirmModal: vi.fn() },
	send: vi.fn(),
	remote: { active: false }
}));

vi.mock('$lib/utils/websocketUtils', () => ({
	send: (type: unknown, data?: unknown) => send(type, data),
	sendAndWait: vi.fn(),
	sendBytes: vi.fn(),
	isReady: () => true,
	pushProgressMessage: vi.fn()
}));

vi.mock('$lib/signal/remoteMode.svelte', () => ({ getRemoteMode: () => remote }));

vi.mock('$states', async () => {
	const { ModsState } = await import('$lib/states/modsState.svelte');
	holder.state = new ModsState();
	return {
		getModsState: () => holder.state,
		getModalState: () => modal
	};
});

import type { ModsState } from '$lib/states/modsState.svelte';
import ApplyResult from '../ApplyResult.svelte';
import { applyResult, noOps as zero } from './fixtures';

const state = () => holder.state as ModsState;
const targetId = 'client-abc';

function result(overrides: Partial<ApplyReply> = {}): ApplyReply {
	return applyResult({ target_id: targetId, ...overrides });
}

function refused(error: Partial<ModError> & { code: string }, overrides: Partial<ApplyReply> = {}) {
	return result({ error: { message: `raw ${error.code}`, ...error }, ...overrides });
}

function libraryMod(id: string, name: string, versions: string[]): LibraryMod {
	return {
		id,
		name,
		custom_name: null,
		mod_type: 'pak',
		author: null,
		summary: null,
		source_kind: 'local',
		source_ref: '{}',
		nexus_mod_id: null,
		ignored_version: null,
		notes: null,
		created_at: '2026-09-01T10:00:00Z',
		updated_at: '2026-09-01T10:00:00Z',
		versions: versions.map((version) => ({
			id: `${id}@${version}`,
			mod_id: id,
			version,
			archive_path: null,
			library_dir: `C:/lib/${id}/${version}`,
			source_ref: '{}',
			installed_at: '2026-09-01T10:00:00Z',
			is_current: true,
			size_bytes: null,
			manifest: null
		})),
		current_version_id: `${id}@${versions[0]}`
	};
}

const flush = () => new Promise((resolve) => setTimeout(resolve, 0));

let onShowTab: ReturnType<typeof vi.fn<(tab: ModsTab) => void>>;

function show(reply: ApplyReply, onDismiss?: () => void) {
	return render(ApplyResult, { props: { result: reply, targetId, onShowTab, onDismiss } });
}

beforeEach(() => {
	send.mockClear();
	modal.showConfirmModal.mockReset();
	onShowTab = vi.fn();
	remote.active = false;
	state().reset();
});

describe('ApplyResult', () => {
	it('renders nothing for a plain success', () => {
		const { container } = show(result({ counts: { ...zero, add: 3 } }));
		expect(container.textContent?.trim()).toBe('');
	});

	it('explains target_locked', () => {
		show(refused({ code: 'target_locked' }));
		expect(
			screen.getByText(
				'Close the game (or stop the server) before applying. Your selections are saved.'
			)
		).toBeTruthy();
		expect(screen.queryByText('raw target_locked')).toBeNull();
	});

	it('explains apply_in_progress without blaming another apply or promising a duration', () => {
		show(refused({ code: 'apply_in_progress' }));
		expect(
			screen.getByText(
				"PalStudio is already changing this install's files (an apply, a server restart or a move). Try again when it finishes."
			)
		).toBeTruthy();
	});

	it('explains a Workshop package refused by the target', () => {
		show(refused({ code: 'not_supported_on_target', kind: 'workshop' }));
		expect(
			screen.getByText(
				"This kind of mod can't be used on this server. Docker servers can't load Workshop packages."
			)
		).toBeTruthy();
	});

	it('names the kind for any other unsupported route', () => {
		show(refused({ code: 'not_supported_on_target', kind: 'nativedll' }));
		expect(screen.getByText('This game install has no place for Native DLL files')).toBeTruthy();
	});

	it('explains no_active_profile', () => {
		show(refused({ code: 'no_active_profile' }));
		expect(screen.getByText('This target has no active profile.')).toBeTruthy();
	});

	it('names both mods of a destination_conflict from the library', () => {
		state().mods = [
			libraryMod('mod-a', 'Alpha', ['1.0.0']),
			libraryMod('mod-b', 'Beta', ['2.0.0', '2.1.0'])
		];
		show(
			refused({
				code: 'destination_conflict',
				path: 'C:/Palworld/Pal/Content/Paks/~mods/shared.pak',
				mod_version_ids: ['mod-a@1.0.0', 'mod-b@2.1.0']
			})
		);
		expect(
			screen.getByText(
				'Alpha and Beta both install C:/Palworld/Pal/Content/Paks/~mods/shared.pak. Disable one of them.'
			)
		).toBeTruthy();
	});

	it('falls back to the raw version id for a mod the library does not have', () => {
		state().mods = [libraryMod('mod-a', 'Alpha', ['1.0.0'])];
		show(
			refused({
				code: 'destination_conflict',
				path: 'C:/x.pak',
				mod_version_ids: ['mod-a@1.0.0', 'gone@9']
			})
		);
		expect(
			screen.getByText('Alpha and gone@9 both install C:/x.pak. Disable one of them.')
		).toBeTruthy();
	});

	describe('unmanaged_occupant', () => {
		const paths = ['C:/Palworld/Mods/Stray/main.lua', 'C:/Palworld/Paks/~mods/stray.pak'];

		it('lists the paths in the way', () => {
			show(refused({ code: 'unmanaged_occupant', paths }));
			expect(
				screen.getByText('These files are in the way and were not installed by PalStudio:')
			).toBeTruthy();
			for (const path of paths) expect(screen.getByText(path)).toBeTruthy();
		});

		it('Adopt them scans and shows the scan tab', async () => {
			show(refused({ code: 'unmanaged_occupant', paths }));
			await fireEvent.click(screen.getByRole('button', { name: 'Adopt them' }));
			expect(send).toHaveBeenCalledWith(MessageType.MOD_TARGET_SCAN, {
				target_id: targetId,
				candidates_only: true
			});
			expect(onShowTab).toHaveBeenCalledWith('scan');
		});

		it('Replace them applies with exactly the paths after confirmation', async () => {
			modal.showConfirmModal.mockResolvedValue(true);
			show(refused({ code: 'unmanaged_occupant', paths }));
			await fireEvent.click(screen.getByRole('button', { name: 'Replace them' }));
			await vi.waitFor(() =>
				expect(send).toHaveBeenCalledWith(MessageType.PROFILE_APPLY, {
					target_id: targetId,
					replace_occupants: paths
				})
			);
			expect(modal.showConfirmModal).toHaveBeenCalledWith(
				expect.objectContaining({
					message: 'They will be moved into a backup you can restore.'
				})
			);
		});

		it('Replace them does nothing when the confirmation is declined', async () => {
			modal.showConfirmModal.mockResolvedValue(false);
			show(refused({ code: 'unmanaged_occupant', paths }));
			await fireEvent.click(screen.getByRole('button', { name: 'Replace them' }));
			await flush();
			expect(modal.showConfirmModal).toHaveBeenCalled();
			expect(send).not.toHaveBeenCalled();
		});

		it('points a remote session at the computer running PalStudio for the backup', async () => {
			remote.active = true;
			modal.showConfirmModal.mockResolvedValue(false);
			show(refused({ code: 'unmanaged_occupant', paths }));
			await fireEvent.click(screen.getByRole('button', { name: 'Replace them' }));
			expect(modal.showConfirmModal).toHaveBeenCalledWith(
				expect.objectContaining({
					message:
						'They will be moved into a backup. Backups can be restored on the computer running PalStudio.'
				})
			);
		});
	});

	describe('replace_partial', () => {
		const reply = (overrides: Partial<ApplyReply> = {}, cause = 'io') =>
			refused(
				{
					code: 'replace_partial',
					moved: ['C:/Palworld/Mods/Stray/main.lua'],
					cause
				},
				{ backup_dir: 'C:/app/mods/_backups/client-abc/20260913-000000-abc', ...overrides }
			);

		it('lists what was moved with one backups link', async () => {
			show(reply());
			expect(
				screen.getByText('Some files were moved into a backup before the apply stopped:')
			).toBeTruthy();
			expect(screen.getByText('C:/Palworld/Mods/Stray/main.lua')).toBeTruthy();
			expect(screen.queryByText('Replaced files were backed up.')).toBeNull();
			const links = screen.getAllByRole('button', { name: 'View backups' });
			expect(links).toHaveLength(1);
			await fireEvent.click(links[0]);
			expect(onShowTab).toHaveBeenCalledWith('backups');
		});

		it('Apply again applies without occupants', async () => {
			show(reply());
			await fireEvent.click(screen.getByRole('button', { name: 'Apply again' }));
			expect(send).toHaveBeenCalledWith(MessageType.PROFILE_APPLY, { target_id: targetId });
		});

		it('offers a single Apply again when the apply also stopped midway', () => {
			show(reply({ mid_apply: true }));
			expect(screen.getAllByRole('button', { name: 'Apply again' })).toHaveLength(1);
		});

		it('explains why the apply stopped when the cause has its own text', () => {
			show(reply({}, 'target_locked'));
			expect(
				screen.getByText(
					'Close the game (or stop the server) before applying. Your selections are saved.'
				)
			).toBeTruthy();
		});
	});

	it('shows the message for any other code', () => {
		show(refused({ code: 'io', message: 'access denied writing C:/x.pak' }));
		expect(screen.getByText('access denied writing C:/x.pak')).toBeTruthy();
	});

	it('offers Apply again after a mid-apply stop', async () => {
		show(result({ mid_apply: true }));
		expect(screen.getByText('Some changes were not finished.')).toBeTruthy();
		await fireEvent.click(screen.getByRole('button', { name: 'Apply again' }));
		expect(send).toHaveBeenCalledWith(MessageType.PROFILE_APPLY, { target_id: targetId });
	});

	it('explains an unreadable file', () => {
		show(result({ needs_attention: [{ path: 'C:/locked.pak', reason: 'unreadable' }] }));
		expect(screen.getByText('C:/locked.pak')).toBeTruthy();
		expect(
			screen.getByText(
				"PalStudio can't read this file. Check that it isn't open in another program and that you have permission."
			)
		).toBeTruthy();
	});

	it('explains a drifted file', () => {
		show(result({ needs_attention: [{ path: 'C:/moved.pak', reason: 'drift' }] }));
		expect(screen.getByText('C:/moved.pak')).toBeTruthy();
		expect(
			screen.getByText(
				'This file is different at its old and new location. Keep the copy you want and delete the other, then apply again.'
			)
		).toBeTruthy();
	});

	it('lists preserved edits, new copies and skipped new copies', () => {
		show(
			result({
				preserved: ['C:/Mods/A/config.lua'],
				new_copies: ['C:/Mods/A/config.lua.new'],
				skipped_new_copies: ['C:/Mods/B/settings.ini.new']
			})
		);
		expect(screen.getByText('Kept your edits to:')).toBeTruthy();
		expect(screen.getByText('C:/Mods/A/config.lua')).toBeTruthy();
		expect(
			screen.getByText('The new version of each was saved beside it as a .new file:')
		).toBeTruthy();
		expect(screen.getByText('C:/Mods/A/config.lua.new')).toBeTruthy();
		expect(screen.getByText('A .new file you created was left untouched:')).toBeTruthy();
		expect(screen.getByText('C:/Mods/B/settings.ini.new')).toBeTruthy();
	});

	it('links a backed-up apply to the backups tab', async () => {
		show(result({ backup_dir: 'C:/app/mods/_backups/client-abc/20260913-000000-abc' }));
		expect(screen.getByText('Replaced files were backed up.')).toBeTruthy();
		await fireEvent.click(screen.getByRole('button', { name: 'View backups' }));
		expect(onShowTab).toHaveBeenCalledWith('backups');
	});

	it('announces an informational outcome politely and a failure assertively', () => {
		const first = show(result({ preserved: ['C:/Mods/A/config.lua'] }));
		expect(screen.getByRole('status').textContent).toContain('Kept your edits to:');
		expect(screen.queryByRole('alert')).toBeNull();
		first.unmount();
		show(result({ mid_apply: true }));
		expect(screen.getByRole('alert').textContent).toContain('Some changes were not finished.');
	});

	it('dismisses through the callback', async () => {
		const onDismiss = vi.fn();
		show(result({ mid_apply: true }), onDismiss);
		await fireEvent.click(screen.getByRole('button', { name: 'Dismiss' }));
		expect(onDismiss).toHaveBeenCalledOnce();
	});
});
