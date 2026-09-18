// @vitest-environment jsdom
import { fireEvent, render, screen, within } from '@testing-library/svelte';
import { tick } from 'svelte';
import { beforeEach, describe, expect, it, vi } from 'vitest';

const { holder, send, env } = vi.hoisted(() => ({
	holder: { state: undefined as unknown },
	send: vi.fn(),
	env: { desktop: 'true' }
}));

vi.mock('$lib/utils/websocketUtils', () => ({
	send: (type: unknown, data?: unknown) => send(type, data),
	sendAndWait: vi.fn(),
	sendBytes: vi.fn(),
	isReady: () => true,
	pushProgressMessage: vi.fn()
}));

vi.mock('$env/static/public', () => ({
	get PUBLIC_DESKTOP_MODE() {
		return env.desktop;
	},
	PUBLIC_WS_URL: 'localhost:0/ws'
}));

vi.mock('$states', async () => {
	const { ModsState } = await import('$lib/states/modsState.svelte');
	holder.state = new ModsState();
	return { getModsState: () => holder.state, getToastState: () => ({ add: vi.fn() }) };
});

import type { ModsState } from '$lib/states/modsState.svelte';
import { profileExportHandler } from '$lib/ws/handlers/modsHandler';
import ExportProfileModal from '../ExportProfileModal.svelte';
import { libraryMod, modVersion } from './fixtures';

const state = () => holder.state as ModsState;
const context = { goto: vi.fn() } as never;

function open(container?: HTMLElement) {
	const closeModal = vi.fn();
	render(ExportProfileModal, {
		target: container,
		props: {
			targetId: 'client-abc',
			profileId: 'client-abc/default',
			profileName: 'Default',
			closeModal
		}
	});
	return closeModal;
}

/** Stands in for the shared modal: its overlay, its close button and its Escape handler. */
function fakeDialog() {
	const dialog = document.createElement('div');
	dialog.setAttribute('role', 'dialog');
	const dialogClose = document.createElement('button');
	const content = document.createElement('div');
	dialog.append(dialogClose, content);
	document.body.append(dialog);
	const dismissed = vi.fn();
	dialogClose.addEventListener('click', dismissed);
	dialog.addEventListener('click', (event) => {
		if (event.target === dialog) dismissed();
	});
	const onKeydown = (event: KeyboardEvent) => {
		if (event.key === 'Escape') dismissed();
	};
	window.addEventListener('keydown', onKeydown);
	return {
		dialog,
		dialogClose,
		content,
		dismissed,
		cleanup: () => {
			window.removeEventListener('keydown', onKeydown);
			dialog.remove();
		}
	};
}

beforeEach(() => {
	send.mockReset();
	env.desktop = 'true';
	state().reset();
	state().mods = [
		libraryMod({
			id: 'mod-a',
			name: 'Alpha',
			versions: [modVersion({ id: 'mod-a@1.0', mod_id: 'mod-a' })],
			current_version_id: 'mod-a@1.0'
		}),
		libraryMod({ id: 'mod-b', name: 'Bravo', versions: [], current_version_id: null })
	];
});

describe('ExportProfileModal', () => {
	it('chooses where to save on desktop, including archives by default', async () => {
		open();
		expect(screen.getByRole('heading', { name: 'Export Default' })).toBeTruthy();

		await fireEvent.click(screen.getByRole('button', { name: 'Choose where to save…' }));

		expect(send).toHaveBeenCalledWith('profile_export', {
			target_id: 'client-abc',
			profile_id: 'client-abc/default',
			include_archives: true,
			path: '__select__'
		});
	});

	it('leaves archives out when unchecked', async () => {
		open();
		await fireEvent.click(screen.getByRole('checkbox', { name: 'Include mod archives' }));
		await fireEvent.click(screen.getByRole('button', { name: 'Choose where to save…' }));
		expect(send.mock.calls[0][1]).toMatchObject({ include_archives: false });
	});

	it('takes a typed path outside desktop mode', async () => {
		env.desktop = 'false';
		open();
		expect(screen.queryByRole('button', { name: 'Choose where to save…' })).toBeNull();
		expect((screen.getByRole('button', { name: 'Export' }) as HTMLButtonElement).disabled).toBe(
			true
		);

		await fireEvent.input(screen.getByLabelText('Save as'), {
			target: { value: ' D:/share/Default.psmods ' }
		});
		await fireEvent.click(screen.getByRole('button', { name: 'Export' }));

		expect(send.mock.calls[0][1]).toMatchObject({ path: 'D:/share/Default.psmods' });
	});

	it('sends nothing more while an export is running', async () => {
		open();
		await fireEvent.click(screen.getByRole('button', { name: 'Choose where to save…' }));
		const choose = screen.getByRole('button', {
			name: 'Choose where to save…'
		}) as HTMLButtonElement;
		expect(choose.disabled).toBe(true);

		await fireEvent.click(choose);
		expect(send).toHaveBeenCalledTimes(1);
	});

	it('shows what was exported, naming missing archives and left-out mods', async () => {
		const closeModal = open();
		await fireEvent.click(screen.getByRole('button', { name: 'Choose where to save…' }));

		await profileExportHandler.handle(
			{
				target_id: 'client-abc',
				profile_id: 'client-abc/default',
				path: 'C:/Users/me/Default.psmods',
				entries: 2,
				archives_included: 1,
				missing_archives: ['mod-a@1.0'],
				unresolved: ['mod-b']
			},
			context
		);
		await tick();

		const status = screen.getByRole('status');
		expect(within(status).getByText('Exported 2 mods to C:/Users/me/Default.psmods')).toBeTruthy();
		expect(within(status).getByText('Archives included: 1')).toBeTruthy();
		expect(within(status).getByText('Alpha')).toBeTruthy();
		expect(within(status).getByText('Bravo')).toBeTruthy();

		await fireEvent.click(screen.getByRole('button', { name: 'Close' }));
		expect(closeModal).toHaveBeenCalled();
	});

	it('stays on the choice quietly when the save dialog is cancelled', async () => {
		open();
		await fireEvent.click(screen.getByRole('button', { name: 'Choose where to save…' }));
		await profileExportHandler.handle(
			{ target_id: 'client-abc', profile_id: 'client-abc/default', canceled: true },
			context
		);
		await tick();

		expect(
			(screen.getByRole('button', { name: 'Choose where to save…' }) as HTMLButtonElement).disabled
		).toBe(false);
		expect(screen.queryByRole('status')).toBeNull();
		expect(screen.queryByRole('alert')).toBeNull();
	});

	it.each([
		['invalid_path', {}, 'Enter a full path ending in .psmods'],
		['export_failed', { reason: 'disk full' }, 'The profile could not be exported: disk full'],
		['desktop_only', {}, "Choosing a location isn't available here."],
		['remote_denied', {}, 'Profiles can only be exported on the computer running PalStudio.'],
		['profile_not_found', {}, 'That profile no longer exists.'],
		['target_not_found', {}, 'This game install is no longer managed by PalStudio'],
		['db', {}, 'raw']
	])('explains %s', async (code, detail, text) => {
		open();
		await fireEvent.click(screen.getByRole('button', { name: 'Choose where to save…' }));
		await profileExportHandler.handle(
			{
				target_id: 'client-abc',
				profile_id: 'client-abc/default',
				error: { code, message: 'raw', ...detail }
			},
			context
		);
		await tick();
		expect(screen.getByRole('alert').textContent).toContain(text);
	});

	it('cannot be closed while its export is unanswered, and closes once it is', async () => {
		const { dialog, dialogClose, content, dismissed, cleanup } = fakeDialog();
		try {
			const closeModal = open(content);
			await fireEvent.click(screen.getByRole('button', { name: 'Choose where to save…' }));
			const close = screen.getByRole('button', { name: 'Close' }) as HTMLButtonElement;
			const heading = screen.getByRole('heading', { name: 'Export Default' });

			expect(close.disabled).toBe(true);
			await fireEvent.keyDown(heading, { key: 'Escape' });
			await fireEvent.click(dialogClose);
			await fireEvent.click(dialog);
			expect(closeModal).not.toHaveBeenCalled();
			expect(dismissed).not.toHaveBeenCalled();

			await profileExportHandler.handle(
				{
					target_id: 'client-abc',
					profile_id: 'client-abc/default',
					error: { code: 'invalid_path', message: 'no' }
				},
				context
			);
			await tick();

			expect(close.disabled).toBe(false);
			await fireEvent.keyDown(heading, { key: 'Escape' });
			await fireEvent.click(dialogClose);
			await fireEvent.click(dialog);
			expect(dismissed).toHaveBeenCalledTimes(3);
			await fireEvent.click(close);
			expect(closeModal).toHaveBeenCalled();
		} finally {
			cleanup();
		}
	});
});
