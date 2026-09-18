// @vitest-environment jsdom
import type { BackupSet } from '$types';
import { MessageType } from '$types';
import { fireEvent, render, screen, within } from '@testing-library/svelte';
import { tick } from 'svelte';
import { beforeEach, describe, expect, it, vi } from 'vitest';

const { holder, modal, remote, send } = vi.hoisted(() => ({
	holder: { state: undefined as unknown },
	modal: { showModal: vi.fn(), showConfirmModal: vi.fn() },
	remote: { active: false },
	send: vi.fn()
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
import BackupsPanel, { backupDate } from '../BackupsPanel.svelte';

const state = () => holder.state as ModsState;
const targetId = 'client-abc';

const set: BackupSet = {
	name: '20260913-142233-op42',
	size_bytes: 2048,
	entries: [
		{ original_path: 'C:/Palworld/Pal/Content/Paks/~mods/a.pak', backup_key: 'k1', hash: 'h1' },
		{ original_path: 'C:/Palworld/Mods/b.lua', backup_key: 'k2', hash: 'h2' }
	]
};

const stamp = new Date(Date.UTC(2026, 8, 13, 14, 22, 33)).toLocaleString();

function setBackups(sets: BackupSet[]) {
	state().backups = { ...state().backups, [targetId]: sets };
}

function show() {
	return render(BackupsPanel, { props: { targetId } });
}

function row(): HTMLElement {
	return screen.getByText(stamp).closest('li') as HTMLElement;
}

function deleteButton(): HTMLButtonElement {
	return within(row()).getByRole('button', { name: 'Delete' }) as HTMLButtonElement;
}

beforeEach(() => {
	send.mockClear();
	modal.showConfirmModal.mockReset().mockResolvedValue(false);
	remote.active = false;
	state().reset();
});

describe('backupDate', () => {
	it('reads the UTC stamp at the start of a set name', () => {
		expect(backupDate('20260913-142233-op42')?.getTime()).toBe(Date.UTC(2026, 8, 13, 14, 22, 33));
	});

	it('gives nothing for a name without a stamp', () => {
		expect(backupDate('manual-copy')).toBeNull();
	});

	it('rejects components that would roll over', () => {
		expect(backupDate('20261399-142233-op')).toBeNull();
		expect(backupDate('20260913-240000-op')).toBeNull();
		expect(backupDate('20260913-126000-op')).toBeNull();
		expect(backupDate('20260913-120060-op')).toBeNull();
	});
});

describe('BackupsPanel', () => {
	it('loads the backups of its target when shown', () => {
		show();
		expect(send).toHaveBeenCalledWith(MessageType.MOD_BACKUP_LIST, { target_id: targetId });
	});

	it('loads the backups once on mount and again after a reconnect', async () => {
		state().connectionChanged(true);
		show();
		await tick();
		expect(send.mock.calls).toEqual([[MessageType.MOD_BACKUP_LIST, { target_id: targetId }]]);

		state().connectionChanged(false);
		state().connectionChanged(true);
		await tick();

		expect(send.mock.calls).toEqual([
			[MessageType.MOD_BACKUP_LIST, { target_id: targetId }],
			[MessageType.MOD_BACKUP_LIST, { target_id: targetId }]
		]);
	});

	it('explains backups are local-only in a remote session and sends nothing', () => {
		remote.active = true;
		setBackups([set]);
		show();

		expect(
			screen.getByText('Backups are only available on the computer running PalStudio.')
		).toBeTruthy();
		expect(send).not.toHaveBeenCalled();
		expect(screen.queryByText(stamp)).toBeNull();
	});

	it('says when an install has no backups yet', () => {
		setBackups([]);
		show();
		expect(screen.getByText('No backups for this install yet.')).toBeTruthy();
	});

	it('shows a set with its date, size and file count', () => {
		setBackups([set]);
		show();

		expect(within(row()).getByText('2.0 KiB')).toBeTruthy();
		expect(within(row()).getByText('Files: 2')).toBeTruthy();
	});

	it('restores a whole set or one entry', async () => {
		setBackups([set]);
		show();
		send.mockClear();

		await fireEvent.click(within(row()).getByRole('button', { name: 'Restore all' }));
		expect(send).toHaveBeenCalledWith(MessageType.MOD_BACKUP_RESTORE, {
			target_id: targetId,
			set: set.name
		});
		state().finishBusy('restoring', targetId);
		await tick();

		await fireEvent.click(within(row()).getByRole('button', { name: 'Show files' }));
		const entry = screen.getByText('C:/Palworld/Mods/b.lua').closest('li') as HTMLElement;
		send.mockClear();
		await fireEvent.click(within(entry).getByRole('button', { name: 'Restore' }));
		expect(send).toHaveBeenCalledWith(MessageType.MOD_BACKUP_RESTORE, {
			target_id: targetId,
			set: set.name,
			paths: ['C:/Palworld/Mods/b.lua']
		});
	});

	it('deletes a set only after confirming', async () => {
		setBackups([set]);
		show();
		send.mockClear();

		await fireEvent.click(deleteButton());
		await tick();
		expect(modal.showConfirmModal).toHaveBeenCalledTimes(1);
		expect(send).not.toHaveBeenCalled();

		modal.showConfirmModal.mockResolvedValue(true);
		await fireEvent.click(deleteButton());
		await tick();
		expect(send).toHaveBeenCalledWith(MessageType.MOD_BACKUP_DELETE, {
			target_id: targetId,
			set: set.name
		});
	});

	it('opens one confirmation however often Delete is clicked', async () => {
		setBackups([set]);
		show();
		let answer: (value: boolean) => void = () => {};
		modal.showConfirmModal.mockImplementation(
			() => new Promise<boolean>((resolve) => (answer = resolve))
		);
		send.mockClear();

		await fireEvent.click(deleteButton());
		await tick();
		expect(deleteButton().disabled).toBe(true);
		deleteButton().disabled = false;
		await fireEvent.click(deleteButton());
		expect(modal.showConfirmModal).toHaveBeenCalledTimes(1);

		answer(true);
		await tick();
		await tick();
		expect(send.mock.calls.filter(([type]) => type === MessageType.MOD_BACKUP_DELETE)).toHaveLength(
			1
		);
		expect(deleteButton().disabled).toBe(false);
	});

	it('disables Delete while a restore runs for the target', async () => {
		setBackups([set]);
		show();

		await fireEvent.click(within(row()).getByRole('button', { name: 'Restore all' }));
		await tick();

		expect(deleteButton().disabled).toBe(true);
	});

	it('disables Delete and holds back its old refusal while a delete runs for the target', async () => {
		setBackups([set]);
		show();
		state().recordRefusal(
			MessageType.MOD_BACKUP_DELETE,
			{ code: 'journal_open', message: 'raw' },
			targetId
		);
		state().deleteBackup(targetId, set.name);
		await tick();

		expect(deleteButton().disabled).toBe(true);
		expect(screen.queryByRole('alert')).toBeNull();

		state().finishBusy('deleting', targetId);
		await tick();
		expect(deleteButton().disabled).toBe(false);
	});

	it('sends no delete when remote mode starts while the confirmation is open', async () => {
		setBackups([set]);
		show();
		let answer: (value: boolean) => void = () => {};
		modal.showConfirmModal.mockImplementation(
			() => new Promise<boolean>((resolve) => (answer = resolve))
		);
		send.mockClear();

		await fireEvent.click(deleteButton());
		remote.active = true;
		answer(true);
		await tick();
		await tick();

		expect(send).not.toHaveBeenCalled();
	});

	it('lists skipped entries with their reasons after a restore', async () => {
		setBackups([set]);
		show();
		state().lastRestore = {
			[targetId]: {
				target_id: targetId,
				set: set.name,
				restored: [],
				skipped: [
					{ path: 'C:/Palworld/Pal/Content/Paks/~mods/a.pak', reason: 'occupied' },
					{ path: 'C:/Palworld/Mods/b.lua', reason: 'changed' },
					{ path: 'C:/Palworld/Mods/c.lua', reason: 'invalid_backup_key' },
					{ path: 'D:/elsewhere/d.lua', reason: 'outside_target' },
					{ path: 'C:/Palworld/Mods/e.lua', reason: 'Access is denied. (os error 5)' }
				]
			}
		};
		await tick();

		const reasonOf = (path: string) =>
			(screen.getByText(path).closest('li') as HTMLElement).textContent;
		expect(reasonOf('C:/Palworld/Pal/Content/Paks/~mods/a.pak')).toContain(
			'Something is already at this path, so it was left alone.'
		);
		expect(reasonOf('C:/Palworld/Mods/b.lua')).toContain(
			'The backup copy no longer matches what was saved.'
		);
		expect(reasonOf('C:/Palworld/Mods/c.lua')).toContain(
			'This backup entry is damaged, so it was left alone.'
		);
		expect(reasonOf('D:/elsewhere/d.lua')).toContain(
			'This path is outside the game install, so it was not restored.'
		);
		expect(reasonOf('C:/Palworld/Mods/e.lua')).toContain('Access is denied. (os error 5)');
	});

	it('explains a delete refused while an apply is unfinished', async () => {
		setBackups([set]);
		show();
		state().recordRefusal(
			MessageType.MOD_BACKUP_DELETE,
			{ code: 'journal_open', message: 'target client-abc has an open apply' },
			targetId
		);
		await tick();

		expect(screen.getByRole('alert').textContent).toContain(
			'An unfinished apply still needs these backups.'
		);
		expect(screen.queryByText('target client-abc has an open apply')).toBeNull();
	});

	it('explains a restore refused while an apply is running', async () => {
		setBackups([set]);
		show();
		state().recordRefusal(
			MessageType.MOD_BACKUP_RESTORE,
			{ code: 'apply_in_progress', message: 'raw' },
			targetId
		);
		await tick();

		expect(screen.getByRole('alert').textContent).toContain(
			'An apply is running for this install. Try again when it finishes.'
		);
	});

	it('ignores a refusal for another target', async () => {
		setBackups([set]);
		show();
		state().recordRefusal(
			MessageType.MOD_BACKUP_DELETE,
			{ code: 'journal_open', message: 'raw' },
			'client-other'
		);
		await tick();

		expect(screen.queryByRole('alert')).toBeNull();
	});
});
