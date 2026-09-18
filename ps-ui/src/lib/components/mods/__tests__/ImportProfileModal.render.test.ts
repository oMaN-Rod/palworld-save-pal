// @vitest-environment jsdom
import type { ImportReply } from '$types';
import { fireEvent, render, screen } from '@testing-library/svelte';
import { tick } from 'svelte';
import { beforeEach, describe, expect, it, vi } from 'vitest';

const { holder, send, env, remote } = vi.hoisted(() => ({
	holder: { state: undefined as unknown },
	send: vi.fn(),
	env: { desktop: 'true' },
	remote: { active: false }
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

vi.mock('$lib/signal/remoteMode.svelte', () => ({ getRemoteMode: () => remote }));

vi.mock('$lib/utils/modUpload', () => ({
	MAX_UPLOAD_BYTES: 512 * 1024 * 1024,
	sha256Hex: async () => 'cd'.repeat(32),
	readChunk: async () => new Uint8Array([7]),
	bytesToBase64: () => 'Bw=='
}));

vi.mock('$states', async () => {
	const { ModsState } = await import('$lib/states/modsState.svelte');
	holder.state = new ModsState();
	return { getModsState: () => holder.state, getToastState: () => ({ add: vi.fn() }) };
});

import type { ModsState } from '$lib/states/modsState.svelte';
import {
	modUploadBeginHandler,
	modUploadChunkHandler,
	modUploadEndHandler,
	profileImportHandler
} from '$lib/ws/handlers/modsHandler';
import ImportProfileModal from '../ImportProfileModal.svelte';
import { libraryMod } from './fixtures';

const state = () => holder.state as ModsState;
const context = { goto: vi.fn() } as never;
const imports = () =>
	send.mock.calls.filter(([type]) => type === 'profile_import').map(([, data]) => data);

function reply(overrides: Partial<ImportReply> = {}): ImportReply {
	return {
		target_id: 'client-abc',
		path: 'C:/p/Default.psmods',
		profile: {
			id: 'client-abc/default-2',
			target_id: 'client-abc',
			name: 'Default (2)',
			is_active: false,
			is_default: false,
			mods: [],
			ue4ss_control_mode: 'enabled_txt',
			force_order_ue4ss: false,
			force_order_palschema: false,
			created_at: '',
			updated_at: ''
		},
		pinned: ['mod-a'],
		following_current: ['mod-b'],
		installed: [{ mod_id: 'mod-a', version_id: 'mod-a@1.0' }],
		missing: [
			{ mod_id: 'gone', name: 'Gone Mod', version: '2.1', reason: 'not_in_library' },
			{ mod_id: 'odd', name: 'Odd', version: '1', reason: 'id_mismatch' },
			{ mod_id: 'bad', name: 'Bad', version: '3', reason: 'extract_failed' }
		],
		disabled: [{ mod_id: 'mod-b', code: 'not_supported_on_target' }],
		frameworks: [{ framework: 'ue4ss', mod_id: 'ue4ss', version: '3.0', in_library: false }],
		...overrides
	};
}

function open(container?: HTMLElement) {
	const closeModal = vi.fn();
	render(ImportProfileModal, { target: container, props: { targetId: 'client-abc', closeModal } });
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
	remote.active = false;
	state().reset();
	state().mods = [
		libraryMod({ id: 'mod-a', name: 'Alpha' }),
		libraryMod({ id: 'mod-b', name: 'Bravo' })
	];
});

describe('ImportProfileModal', () => {
	it('picks a file on desktop, with an optional name', async () => {
		open();
		await fireEvent.input(screen.getByLabelText('Name (optional)'), {
			target: { value: 'Shared' }
		});
		await fireEvent.click(screen.getByRole('button', { name: 'Choose a .psmods file…' }));
		expect(imports()).toEqual([{ target_id: 'client-abc', path: '__select__', name: 'Shared' }]);
		expect(screen.getByText('Importing…')).toBeTruthy();
	});

	it('takes a typed path outside desktop mode', async () => {
		env.desktop = 'false';
		open();
		expect(screen.queryByRole('button', { name: 'Choose a .psmods file…' })).toBeNull();
		await fireEvent.input(screen.getByLabelText('Profile file'), {
			target: { value: 'D:/share/Default.psmods' }
		});
		await fireEvent.click(screen.getByRole('button', { name: 'Import' }));
		expect(imports()).toEqual([{ target_id: 'client-abc', path: 'D:/share/Default.psmods' }]);
	});

	it('uploads the file first in a remote session, then imports the uploaded copy', async () => {
		env.desktop = 'false';
		remote.active = true;
		open();
		expect(screen.queryByLabelText('Profile file')).toBeNull();
		expect(screen.queryByRole('button', { name: 'Choose a .psmods file…' })).toBeNull();

		const input = screen.getByLabelText('Profile file on this device') as HTMLInputElement;
		expect(input.accept).toBe('.psmods');
		Object.defineProperty(input, 'files', {
			value: [new File([new Uint8Array(1)], 'Default.psmods')],
			configurable: true
		});
		await fireEvent.change(input);
		await fireEvent.click(screen.getByRole('button', { name: 'Upload' }));
		await vi.waitFor(() =>
			expect(send).toHaveBeenCalledWith('mod_upload_begin', {
				name: 'Default.psmods',
				size: 1,
				sha256: 'cd'.repeat(32)
			})
		);
		await modUploadBeginHandler.handle(
			{ upload_id: 'u9', chunk_size: 1048576, name: 'Default.psmods' },
			context
		);
		await vi.waitFor(() =>
			expect(send).toHaveBeenCalledWith('mod_upload_chunk', {
				upload_id: 'u9',
				seq: 0,
				data_b64: 'Bw=='
			})
		);
		await modUploadChunkHandler.handle({ upload_id: 'u9', seq: 0, received: 1 }, context);
		await vi.waitFor(() =>
			expect(send).toHaveBeenCalledWith('mod_upload_end', { upload_id: 'u9' })
		);
		await modUploadEndHandler.handle(
			{ upload_id: 'u9', path: 'C:/app/downloads/Default.psmods' },
			context
		);
		await tick();

		expect(imports()).toEqual([
			{ target_id: 'client-abc', path: 'C:/app/downloads/Default.psmods' }
		]);
		expect(state().uploadStatus).toBeNull();
	});

	it('shows every part of the import result', async () => {
		const closeModal = open();
		await fireEvent.click(screen.getByRole('button', { name: 'Choose a .psmods file…' }));
		await profileImportHandler.handle(reply(), context);
		await tick();

		expect(
			screen.getByText("Imported Default (2). It isn't active yet — activate it to apply its mods.")
		).toBeTruthy();
		expect(screen.getByText('Pinned to the exported version: 1')).toBeTruthy();
		expect(
			screen.getByText('Using your current version instead of the exported one:')
		).toBeTruthy();
		expect(screen.getByText('Bravo')).toBeTruthy();
		expect(screen.getByText('Installed from the file:')).toBeTruthy();
		expect(screen.getByText('Alpha')).toBeTruthy();
		expect(
			screen.getByText("Gone Mod v2.1: not in your library, and the file doesn't include it")
		).toBeTruthy();
		expect(
			screen.getByText('Odd v1: the included archive installed as a different mod')
		).toBeTruthy();
		expect(screen.getByText('Bad v3: the included archive could not be unpacked')).toBeTruthy();
		expect(screen.getByText('Bravo: this install has no place for this kind of mod')).toBeTruthy();
		expect(screen.getByText('ue4ss v3.0 (not in your library)')).toBeTruthy();
		expect(state().viewedProfile('client-abc')?.id).toBe('client-abc/default-2');
		expect(screen.queryByText('Importing…')).toBeNull();

		await fireEvent.click(screen.getByRole('button', { name: 'Close' }));
		expect(closeModal).toHaveBeenCalled();
	});

	it('explains every other reason a mod was not imported or was turned off', async () => {
		open();
		await fireEvent.click(screen.getByRole('button', { name: 'Choose a .psmods file…' }));
		await profileImportHandler.handle(
			reply({
				pinned: [],
				following_current: [],
				installed: [],
				missing: [
					'io_error',
					'nothing_routed',
					'library_error',
					'already_managed',
					'something_new'
				].map((reason) => ({ mod_id: reason, name: reason, version: '1', reason })),
				disabled: [{ mod_id: 'mod-a', code: 'not_subscribed_on_target' }],
				frameworks: [{ framework: 'palschema', mod_id: 'ps', version: '1', in_library: true }]
			}),
			context
		);
		await tick();

		for (const text of [
			'io_error v1: the included archive could not be read',
			'nothing_routed v1: nothing in the included archive could be installed',
			'library_error v1: the included archive could not be added to your library',
			'already_managed v1: a Steam-subscribed package with the same name is already in your library',
			'something_new v1: the included archive could not be installed (something_new)'
		]) {
			expect(screen.getByText(text)).toBeTruthy();
		}
		expect(
			screen.getByText(
				"Alpha: This server's Steam Workshop folder does not have this mod. Subscribe to it there first."
			)
		).toBeTruthy();
		expect(screen.getByText('palschema v1 (in your library)')).toBeTruthy();
		expect(screen.queryByText('Pinned to the exported version: 0')).toBeNull();
		expect(screen.queryByText('Installed from the file:')).toBeNull();
	});

	it('stays on the choice quietly when the file dialog is cancelled', async () => {
		open();
		await fireEvent.click(screen.getByRole('button', { name: 'Choose a .psmods file…' }));
		await profileImportHandler.handle(
			{ target_id: 'client-abc', path: '__select__', canceled: true },
			context
		);
		await tick();

		expect(
			(screen.getByRole('button', { name: 'Choose a .psmods file…' }) as HTMLButtonElement).disabled
		).toBe(false);
		expect(screen.queryByRole('status')).toBeNull();
		expect(screen.queryByRole('alert')).toBeNull();
	});

	it.each([
		['invalid_archive', {}, "This file isn't a PalStudio mod profile."],
		['unsupported_format', { format: 'x', format_version: 2 }, 'made by a newer PalStudio'],
		['invalid_path', {}, "PalStudio can't find that file."],
		['invalid_name', {}, 'Profile names need 1 to 64 characters.'],
		['io', {}, "PalStudio couldn't read this file: raw"],
		['import_failed', { reason: 'disk full' }, 'The profile could not be imported: disk full'],
		[
			'import_failed',
			{ reason: 'disk full', profile_id: 'client-abc/x' },
			'The profile was created but not finished: disk full'
		],
		['desktop_only', {}, "Browsing for a file isn't available here."],
		['remote_denied', {}, 'only an uploaded file can be imported'],
		['target_not_found', {}, 'This game install is no longer managed by PalStudio'],
		['db', {}, 'raw']
	])('explains %s', async (code, detail, text) => {
		open();
		await fireEvent.click(screen.getByRole('button', { name: 'Choose a .psmods file…' }));
		await profileImportHandler.handle(
			{ target_id: 'client-abc', path: '__select__', error: { code, message: 'raw', ...detail } },
			context
		);
		await tick();
		expect(screen.getByRole('alert').textContent).toContain(text);
	});

	it('cannot be closed while its import is unanswered, and closes once it is', async () => {
		const { dialog, dialogClose, content, dismissed, cleanup } = fakeDialog();
		try {
			const closeModal = open(content);
			await fireEvent.click(screen.getByRole('button', { name: 'Choose a .psmods file…' }));
			const close = screen.getByRole('button', { name: 'Close' }) as HTMLButtonElement;
			const heading = screen.getByRole('heading', { name: 'Import a profile' });

			expect(close.disabled).toBe(true);
			await fireEvent.keyDown(heading, { key: 'Escape' });
			await fireEvent.click(dialogClose);
			await fireEvent.click(dialog);
			expect(closeModal).not.toHaveBeenCalled();
			expect(dismissed).not.toHaveBeenCalled();

			await profileImportHandler.handle(
				{
					target_id: 'client-abc',
					path: '__select__',
					error: { code: 'invalid_archive', message: 'no' }
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
