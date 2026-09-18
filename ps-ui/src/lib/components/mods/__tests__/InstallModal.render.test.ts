// @vitest-environment jsdom
import type { InstallManifest } from '$types';
import { MessageType } from '$types';
import { fireEvent, render, screen } from '@testing-library/svelte';
import { tick } from 'svelte';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

const { holder, send, env, toast, socket, remote } = vi.hoisted(() => ({
	holder: { state: undefined as unknown },
	send: vi.fn(),
	env: { desktop: 'true' },
	toast: { add: vi.fn() },
	socket: { connected: true },
	remote: { active: false }
}));

vi.mock('$lib/signal/remoteMode.svelte', () => ({ getRemoteMode: () => remote }));

vi.mock('$lib/utils/websocketUtils', () => ({
	send: (type: unknown, data?: unknown) => send(type, data),
	sendAndWait: vi.fn(),
	sendBytes: vi.fn(),
	isReady: () => true,
	pushProgressMessage: vi.fn()
}));

vi.mock('$lib/utils/modUpload', () => ({
	MAX_UPLOAD_BYTES: 512 * 1024 * 1024,
	sha256Hex: async () => 'ab'.repeat(32),
	readChunk: async () => new Uint8Array([1, 2]),
	bytesToBase64: () => 'AQI='
}));

vi.mock('$env/static/public', () => ({
	get PUBLIC_DESKTOP_MODE() {
		return env.desktop;
	}
}));

vi.mock('$states', async () => {
	const { ModsState } = await import('$lib/states/modsState.svelte');
	holder.state = new ModsState();
	return {
		getModsState: () => holder.state,
		getToastState: () => toast,
		getSocketState: () => socket
	};
});

import type { ModsState } from '$lib/states/modsState.svelte';
import {
	modAnalyzeHandler,
	modInstallHandler,
	modUploadBeginHandler,
	modUploadChunkHandler,
	modUploadEndHandler
} from '$lib/ws/handlers/modsHandler';
import InstallModal from '../InstallModal.svelte';
import { modTarget as target } from './fixtures';

const state = () => holder.state as ModsState;
const context = { goto: vi.fn() } as never;
const PICKED = 'C:/Downloads/CoolMod.zip';

function manifest(overrides: Partial<InstallManifest> = {}): InstallManifest {
	return {
		folder_name: 'CoolMod',
		display_name: 'Cool Mod',
		mod_type: 'ue4ss',
		version: '1.2.0',
		routes: [
			{
				archive_path: 'CoolMod/Scripts/main.lua',
				rel_path: 'CoolMod/Scripts/main.lua',
				kind: 'ue4ss'
			}
		],
		decisions: [],
		platform_filtered: null,
		source: {},
		...overrides
	};
}

function sent(type: MessageType): Record<string, unknown>[] {
	return send.mock.calls.filter(([sentType]) => sentType === type).map(([, data]) => data);
}

function open(targetId = 'client-abc', container?: HTMLElement) {
	const closeModal = vi.fn();
	render(InstallModal, { target: container, props: { targetId, closeModal } });
	return closeModal;
}

async function analyzed(targetId = 'client-abc', overrides: Partial<InstallManifest> = {}) {
	await modAnalyzeHandler.handle(
		{ target_id: targetId, path: PICKED, manifest: manifest(overrides) },
		context
	);
	await tick();
}

async function reachReview(targetId = 'client-abc', overrides: Partial<InstallManifest> = {}) {
	const closeModal = open(targetId);
	await fireEvent.click(screen.getByRole('button', { name: 'Choose archive…' }));
	await analyzed(targetId, overrides);
	return closeModal;
}

async function analyzeRefused(error: Record<string, unknown>) {
	await modAnalyzeHandler.handle({ target_id: 'client-abc', path: PICKED, error }, context);
	await tick();
}

async function installReply(data: Record<string, unknown>) {
	await modInstallHandler.handle({ target_id: 'client-abc', path: PICKED, ...data }, context);
	await tick();
}

function installButton(name = 'Install'): HTMLButtonElement {
	return screen.getByRole('button', { name }) as HTMLButtonElement;
}

beforeEach(() => {
	send.mockReset();
	toast.add.mockReset();
	env.desktop = 'true';
	socket.connected = true;
	remote.active = false;
	state().reset();
	state().targets = [
		target(),
		target({ id: 'server-7', kind: 'server', server_id: 7, root_path: '/palworld' })
	];
});

describe('InstallModal', () => {
	describe('choosing', () => {
		it('browses for an archive on desktop and lists the supported kinds', async () => {
			open();

			expect(screen.queryByLabelText('Archive path')).toBeNull();
			expect(
				screen.getByText(
					'Supported: zip, 7z, rar, tar or tar.gz archives, or a single .pak, .lua or .dll file.'
				)
			).toBeTruthy();

			await fireEvent.click(screen.getByRole('button', { name: 'Choose archive…' }));

			expect(sent(MessageType.MOD_ANALYZE)).toEqual([
				{ target_id: 'client-abc', path: '__select__' }
			]);
		});

		it('allows no second pick until the analysis replies, without claiming to read while the picker is open', async () => {
			open();
			const choose = screen.getByRole('button', { name: 'Choose archive…' }) as HTMLButtonElement;

			await fireEvent.click(choose);
			await fireEvent.click(choose);

			expect(sent(MessageType.MOD_ANALYZE)).toHaveLength(1);
			expect(choose.disabled).toBe(true);
			expect(screen.queryByText('Reading the archive…')).toBeNull();

			await analyzed();
			expect(screen.getByText('v1.2.0')).toBeTruthy();
		});

		it('shows a reading state for a typed path until the analysis replies', async () => {
			env.desktop = 'false';
			open();
			await fireEvent.input(screen.getByLabelText('Archive path'), {
				target: { value: '/srv/mods/CoolMod.zip' }
			});
			await fireEvent.click(screen.getByRole('button', { name: 'Analyze' }));

			expect(screen.getByText('Reading the archive…')).toBeTruthy();

			await analyzed();
			expect(screen.queryByText('Reading the archive…')).toBeNull();
			expect(screen.getByText('v1.2.0')).toBeTruthy();
		});

		it('says the connection dropped when a typed analysis is never answered', async () => {
			env.desktop = 'false';
			open();
			await fireEvent.input(screen.getByLabelText('Archive path'), {
				target: { value: '/srv/mods/CoolMod.zip' }
			});
			await fireEvent.click(screen.getByRole('button', { name: 'Analyze' }));

			socket.connected = false;
			state().connectionChanged(true);
			state().connectionChanged(false);
			await tick();

			expect(screen.getByRole('alert').textContent).toContain(
				'The connection dropped while the archive was being read. Choose the archive again.'
			);
			expect(screen.queryByText('Reading the archive…')).toBeNull();
			expect((screen.getByRole('button', { name: 'Analyze' }) as HTMLButtonElement).disabled).toBe(
				false
			);
		});

		it('says the connection dropped when a pick is never answered', async () => {
			open();
			await fireEvent.click(screen.getByRole('button', { name: 'Choose archive…' }));

			socket.connected = false;
			state().connectionChanged(true);
			state().connectionChanged(false);
			await tick();

			expect(screen.getByRole('alert').textContent).toContain(
				'The connection dropped while the archive was being read.'
			);
			await fireEvent.click(screen.getByRole('button', { name: 'Choose archive…' }));
			expect(screen.queryByRole('alert')).toBeNull();
		});

		it('takes a path on the server machine outside desktop mode', async () => {
			env.desktop = 'false';
			open();

			expect(screen.queryByRole('button', { name: 'Choose archive…' })).toBeNull();
			const analyze = screen.getByRole('button', { name: 'Analyze' }) as HTMLButtonElement;
			expect(analyze.disabled).toBe(true);

			await fireEvent.input(screen.getByLabelText('Archive path'), {
				target: { value: '  /srv/mods/CoolMod.zip ' }
			});
			await fireEvent.click(analyze);

			expect(sent(MessageType.MOD_ANALYZE)).toEqual([
				{ target_id: 'client-abc', path: '/srv/mods/CoolMod.zip' }
			]);
			expect(analyze.disabled).toBe(true);
		});

		it('stays on the first step, silently, when the dialog is cancelled', async () => {
			open();
			await fireEvent.click(screen.getByRole('button', { name: 'Choose archive…' }));

			await modAnalyzeHandler.handle(
				{ target_id: 'client-abc', path: '__select__', canceled: true },
				context
			);
			await tick();

			const choose = screen.getByRole('button', { name: 'Choose archive…' }) as HTMLButtonElement;
			expect(choose.disabled).toBe(false);
			expect(screen.queryByText('Reading the archive…')).toBeNull();
			expect(screen.queryByRole('alert')).toBeNull();
			expect(toast.add).not.toHaveBeenCalled();
		});

		it('shows an analyze refusal under the first step, without a toast', async () => {
			open();
			await fireEvent.click(screen.getByRole('button', { name: 'Choose archive…' }));

			await analyzeRefused({ code: 'nothing_routed', message: 'nothing routed' });

			expect(screen.getByRole('alert').textContent).toContain(
				'Nothing in this archive has a place to go.'
			);
			expect(screen.getByRole('button', { name: 'Choose archive…' })).toBeTruthy();
			expect(toast.add).not.toHaveBeenCalled();
		});

		it('explains an archive that could not be opened, keeping the detail', async () => {
			open();
			await fireEvent.click(screen.getByRole('button', { name: 'Choose archive…' }));

			await analyzeRefused({ code: 'extract_failed', message: 'bad header' });

			expect(screen.getByRole('alert').textContent).toContain(
				'The archive could not be opened. (bad header)'
			);
		});

		it('explains a path that is not a file on the server machine', async () => {
			env.desktop = 'false';
			open();
			await fireEvent.input(screen.getByLabelText('Archive path'), {
				target: { value: 'relative.zip' }
			});
			await fireEvent.click(screen.getByRole('button', { name: 'Analyze' }));

			await analyzeRefused({ code: 'invalid_path', message: 'relative.zip is not absolute' });

			const alert = screen.getByRole('alert').textContent ?? '';
			expect(alert).toContain("PalStudio can't find that file.");
			expect(alert).toContain('(relative.zip is not absolute)');
		});

		it('offers the path field when the server cannot browse for a file', async () => {
			open();
			await fireEvent.click(screen.getByRole('button', { name: 'Choose archive…' }));

			await analyzeRefused({ code: 'desktop_only', message: 'desktop mode is required' });

			expect(screen.getByRole('alert').textContent).toContain(
				"Browsing for a file isn't available here. Enter the archive's path instead."
			);
			await fireEvent.input(screen.getByLabelText('Archive path'), {
				target: { value: 'C:/Downloads/CoolMod.zip' }
			});
			await fireEvent.click(screen.getByRole('button', { name: 'Analyze' }));

			expect(sent(MessageType.MOD_ANALYZE).at(-1)).toEqual({
				target_id: 'client-abc',
				path: 'C:/Downloads/CoolMod.zip'
			});
			expect(screen.getByLabelText('Archive path')).toBeTruthy();
		});

		it('ignores an analysis left from before it opened', async () => {
			await analyzed();
			open();

			expect(screen.getByRole('button', { name: 'Choose archive…' })).toBeTruthy();
			expect(screen.queryByText('Cool Mod')).toBeNull();
		});

		it("ignores another target's analysis", async () => {
			open();
			await fireEvent.click(screen.getByRole('button', { name: 'Choose archive…' }));
			await analyzed('server-7');

			expect(screen.queryByText('Cool Mod')).toBeNull();
		});

		describe('in a remote session', () => {
			async function pick(name = 'CoolMod.zip') {
				const input = screen.getByLabelText('Archive on this device') as HTMLInputElement;
				Object.defineProperty(input, 'files', {
					value: [new File([new Uint8Array(2)], name)],
					configurable: true
				});
				await fireEvent.change(input);
			}

			async function startUpload() {
				await pick();
				await fireEvent.click(screen.getByRole('button', { name: 'Upload' }));
				await vi.waitFor(() => expect(sent(MessageType.MOD_UPLOAD_BEGIN)).toHaveLength(1));
				await modUploadBeginHandler.handle(
					{ upload_id: 'u1', chunk_size: 1048576, name: 'CoolMod.zip' },
					context
				);
				await vi.waitFor(() => expect(sent(MessageType.MOD_UPLOAD_CHUNK)).toHaveLength(1));
				await tick();
			}

			beforeEach(() => {
				remote.active = true;
			});

			it('uploads the chosen archive, then analyzes the uploaded copy', async () => {
				open();
				expect(screen.queryByRole('button', { name: 'Choose archive…' })).toBeNull();
				expect(screen.queryByLabelText('Archive path')).toBeNull();

				await startUpload();
				expect(sent(MessageType.MOD_UPLOAD_BEGIN)).toEqual([
					{ name: 'CoolMod.zip', size: 2, sha256: 'ab'.repeat(32) }
				]);
				expect(sent(MessageType.MOD_UPLOAD_CHUNK)).toEqual([
					{ upload_id: 'u1', seq: 0, data_b64: 'AQI=' }
				]);
				expect(screen.getByText('Uploading CoolMod.zip: 0%')).toBeTruthy();

				await modUploadChunkHandler.handle({ upload_id: 'u1', seq: 0, received: 2 }, context);
				await vi.waitFor(() =>
					expect(sent(MessageType.MOD_UPLOAD_END)).toEqual([{ upload_id: 'u1' }])
				);
				await modUploadEndHandler.handle(
					{ upload_id: 'u1', path: 'C:/app/downloads/CoolMod.zip' },
					context
				);
				await tick();

				expect(sent(MessageType.MOD_ANALYZE)).toEqual([
					{ target_id: 'client-abc', path: 'C:/app/downloads/CoolMod.zip' }
				]);
				expect(state().uploadStatus).toBeNull();
			});

			it('shows a refused upload in words and lets the user try again', async () => {
				open();
				await pick();
				await fireEvent.click(screen.getByRole('button', { name: 'Upload' }));
				await vi.waitFor(() => expect(sent(MessageType.MOD_UPLOAD_BEGIN)).toHaveLength(1));

				await modUploadBeginHandler.handle(
					{ name: 'CoolMod.zip', error: { code: 'unsupported_type', message: 'raw' } },
					context
				);
				await tick();

				expect(screen.getByRole('alert').textContent).toContain(
					"This kind of file can't be uploaded."
				);
				expect((screen.getByRole('button', { name: 'Upload' }) as HTMLButtonElement).disabled).toBe(
					false
				);
			});

			it('cancels an upload and sends nothing more', async () => {
				open();
				await startUpload();

				await fireEvent.click(screen.getByRole('button', { name: 'Cancel upload' }));
				await modUploadChunkHandler.handle({ upload_id: 'u1', seq: 0, received: 2 }, context);
				await new Promise((resolve) => setTimeout(resolve, 20));

				expect(sent(MessageType.MOD_UPLOAD_END)).toEqual([]);
				expect(state().uploadStatus).toBeNull();
				expect(screen.getByRole('button', { name: 'Upload' })).toBeTruthy();
			});

			it('cancels the upload when the modal closes', async () => {
				const view = render(InstallModal, {
					props: { targetId: 'client-abc', closeModal: vi.fn() }
				});
				await startUpload();

				view.unmount();

				expect(state().uploadStatus).toBeNull();
			});
		});
	});

	describe('reviewing', () => {
		it('shows the manifest with the name prefilled and enabling on', async () => {
			await reachReview();

			expect(screen.getByText('v1.2.0')).toBeTruthy();
			expect((screen.getByLabelText('Name') as HTMLInputElement).value).toBe('Cool Mod');
			const enable = screen.getByRole('checkbox', {
				name: 'Enable on this install'
			}) as HTMLInputElement;
			expect(enable.checked).toBe(true);
		});

		it('keeps Enable on this install reachable from the keyboard', async () => {
			await reachReview();
			const enable = screen.getByRole('checkbox', {
				name: 'Enable on this install'
			}) as HTMLInputElement;

			expect(enable.getAttribute('aria-hidden')).toBeNull();
			expect(enable.style.display).not.toBe('none');
			enable.focus();
			expect(document.activeElement).toBe(enable);
		});

		it('moves focus into the review when the analysis arrives', async () => {
			await reachReview();
			await new Promise((resolve) => setTimeout(resolve, 0));

			const dialog = screen.getByRole('heading', { name: 'Install a mod' }).parentElement;
			expect(dialog?.contains(document.activeElement)).toBe(true);
			expect(document.activeElement).not.toBe(document.body);
		});

		it('goes back to choosing another archive', async () => {
			await reachReview();

			await fireEvent.click(screen.getByRole('button', { name: 'Choose another archive' }));

			expect(screen.getByRole('button', { name: 'Choose archive…' })).toBeTruthy();
			expect(screen.queryByText('v1.2.0')).toBeNull();
		});

		it('warns on a server target when a dedicated server will not load the mod', async () => {
			await reachReview('server-7', { source: { server_capable: false } });

			expect(
				screen.getByText(
					'This mod has no server install rule, so a dedicated server will not load it.'
				)
			).toBeTruthy();
		});

		it('does not warn a game client about server loading', async () => {
			await reachReview('client-abc', { source: { server_capable: false } });

			expect(screen.queryByText(/dedicated server will not load it/)).toBeNull();
		});

		it('shows the decisions found by the analysis before installing, and accepts them on install', async () => {
			await reachReview('client-abc', {
				decisions: [{ kind: 'unplaced_files', files: ['readme.md'] }]
			});

			expect(
				screen.getByText('These files have no known destination and will be skipped: readme.md')
			).toBeTruthy();
			expect(screen.getByRole('status').textContent).toContain('Read the notes above');
			expect(screen.queryByRole('button', { name: 'Install' })).toBeNull();

			await fireEvent.click(installButton('Install with these defaults'));

			expect(sent(MessageType.MOD_INSTALL)).toEqual([
				{ target_id: 'client-abc', path: PICKED, accept_defaults: true, enable: true }
			]);
		});

		it('explains a disabled Install while another install on the target runs', async () => {
			state().install('client-abc', 'C:/Downloads/Earlier.zip');
			await reachReview();

			expect(installButton().disabled).toBe(true);
			expect(
				screen.getByText('Another install on this game install is still running.')
			).toBeTruthy();
		});
	});

	describe('Enter', () => {
		let stop: () => void;

		beforeEach(() => {
			const onKeydown = (event: KeyboardEvent) => {
				if (event.key !== 'Enter') return;
				event.preventDefault();
				const primary = document.querySelector<HTMLButtonElement>('[data-modal-primary]');
				if (primary && !primary.disabled) primary.click();
			};
			window.addEventListener('keydown', onKeydown);
			stop = () => window.removeEventListener('keydown', onKeydown);
		});

		afterEach(() => stop());

		it('installs from the name field', async () => {
			await reachReview();

			await fireEvent.keyDown(screen.getByLabelText('Name'), { key: 'Enter' });

			expect(sent(MessageType.MOD_INSTALL)).toHaveLength(1);
		});

		it.each([
			['a group toggle', () => screen.getByRole('button', { name: /UE4SS Mods/ })],
			[
				'Choose another archive',
				() => screen.getByRole('button', { name: 'Choose another archive' })
			],
			['Cancel', () => screen.getByRole('button', { name: 'Cancel' })],
			[
				'Enable on this install',
				() => screen.getByRole('checkbox', { name: 'Enable on this install' })
			]
		])('does not install from %s', async (_, control) => {
			await reachReview();

			await fireEvent.keyDown(control(), { key: 'Enter' });

			expect(sent(MessageType.MOD_INSTALL)).toEqual([]);
		});
	});

	describe('installing', () => {
		it('installs the picked path with the chosen name and enabling', async () => {
			await reachReview();

			await fireEvent.input(screen.getByLabelText('Name'), { target: { value: ' My Cool Mod ' } });
			await fireEvent.click(screen.getByText('Enable on this install'));
			const button = installButton();
			await fireEvent.click(button);

			expect(sent(MessageType.MOD_INSTALL)).toEqual([
				{
					target_id: 'client-abc',
					path: PICKED,
					accept_defaults: false,
					enable: false,
					custom_name: 'My Cool Mod'
				}
			]);
			expect(button.disabled).toBe(true);
		});

		it('sends no custom name when the name is left as analysed or cleared', async () => {
			await reachReview();

			await fireEvent.click(installButton());
			await installReply({ error: { code: 'io', message: 'retry' } });
			await fireEvent.input(screen.getByLabelText('Name'), { target: { value: '   ' } });
			await fireEvent.click(installButton());

			expect(sent(MessageType.MOD_INSTALL)).toEqual([
				{ target_id: 'client-abc', path: PICKED, accept_defaults: false, enable: true },
				{ target_id: 'client-abc', path: PICKED, accept_defaults: false, enable: true }
			]);
		});

		it('highlights the decisions and re-sends with the defaults accepted', async () => {
			await reachReview();
			await fireEvent.click(installButton());

			await installReply({
				needs_decisions: [{ kind: 'unplaced_files', files: ['readme.md'] }]
			});

			expect(
				screen.getByText('These files have no known destination and will be skipped: readme.md')
			).toBeTruthy();
			expect(screen.queryByRole('button', { name: 'Install' })).toBeNull();

			await fireEvent.click(installButton('Install with these defaults'));

			expect(sent(MessageType.MOD_INSTALL)[1]).toEqual({
				target_id: 'client-abc',
				path: PICKED,
				accept_defaults: true,
				enable: true
			});
		});

		it('shows its own success state instead of a toast, and closes from it', async () => {
			const closeModal = await reachReview();
			await fireEvent.click(installButton());
			expect(closeModal).not.toHaveBeenCalled();

			await installReply({
				mod_id: 'cool-mod',
				version_id: 'cool-mod@1.2.0',
				manifest: manifest()
			});

			expect(closeModal).not.toHaveBeenCalled();
			expect(screen.getByText('Installed Cool Mod')).toBeTruthy();
			expect(
				screen.getByText('It is enabled on this install. Apply your changes to put it in the game.')
			).toBeTruthy();
			expect(toast.add).not.toHaveBeenCalled();

			await fireEvent.click(screen.getByRole('button', { name: 'Close' }));
			expect(closeModal).toHaveBeenCalledWith(true);
		});

		it('says nothing about enabling after an install that was not asked to enable', async () => {
			await reachReview();
			await fireEvent.click(screen.getByText('Enable on this install'));
			await fireEvent.click(installButton());

			await installReply({ mod_id: 'cool-mod', version_id: 'cool-mod@1.2.0' });

			expect(screen.getByText('Installed Cool Mod')).toBeTruthy();
			expect(screen.queryByText(/enabled on this install/)).toBeNull();
		});

		it('does not take the record of an install for another path as its own success', async () => {
			const closeModal = await reachReview();
			await fireEvent.click(installButton());

			await installReply({
				path: 'C:/Downloads/Other.zip',
				mod_id: 'other',
				version_id: 'other@1'
			});

			expect(closeModal).not.toHaveBeenCalled();
			expect(screen.queryByText('Installed Cool Mod')).toBeNull();
			expect(screen.getByRole('alert')).toBeTruthy();
		});

		it('shows an install refusal under the review with its next step, without a toast', async () => {
			const closeModal = await reachReview();
			await fireEvent.click(installButton());

			await installReply({
				error: { code: 'already_installed', message: 'x', mod_id: 'cool-mod' }
			});

			expect(screen.getByRole('alert').textContent).toContain(
				'This version is already in your library. Enable it from the mod list.'
			);
			expect(closeModal).not.toHaveBeenCalled();
			expect(installButton().disabled).toBe(false);
			expect(toast.add).not.toHaveBeenCalled();
		});

		it('shows a Steam subscription already carrying the package in words', async () => {
			await reachReview();
			await fireEvent.click(installButton());

			await installReply({ error: { code: 'already_managed', message: 'x', mod_id: 'ws-1' } });

			expect(screen.getByRole('alert').textContent).toContain(
				'A Steam Workshop subscription already provides this package. Manage it through Steam, or adopt it from the Scan tab.'
			);
		});

		it('warns, without failing or a toast, when the mod installed but could not be enabled', async () => {
			const closeModal = await reachReview();
			await fireEvent.click(installButton());

			await installReply({
				mod_id: 'cool-mod',
				version_id: 'cool-mod@1.2.0',
				manifest: manifest(),
				enable_error: { code: 'not_supported_on_target', message: 'x', kind: 'nativedll' }
			});

			expect(closeModal).not.toHaveBeenCalled();
			expect(
				screen.getByText('Cool Mod was installed but not enabled on this install.')
			).toBeTruthy();
			expect(screen.getByText("Game clients can't load native DLL mods")).toBeTruthy();
			expect(screen.queryByRole('button', { name: /^Install/ })).toBeNull();
			expect(toast.add).not.toHaveBeenCalled();

			await fireEvent.click(screen.getByRole('button', { name: 'Close' }));
			expect(closeModal).toHaveBeenCalledWith(true);
		});

		it('ignores an unrelated refusal recorded for the target while installing', async () => {
			await reachReview();
			await fireEvent.click(installButton());
			state().recordRefusal(
				'profile_plan',
				{ code: 'no_active_profile', message: 'unrelated' },
				'client-abc'
			);

			await installReply({ mod_id: 'cool-mod', version_id: 'cool-mod@1.2.0' });

			expect(screen.getByText('Installed Cool Mod')).toBeTruthy();
			expect(screen.queryByText(/was installed but not enabled/)).toBeNull();
		});

		it('says the connection dropped instead of closing when an install is never answered', async () => {
			const closeModal = await reachReview();
			await fireEvent.click(installButton());

			socket.connected = false;
			state().connectionChanged(true);
			state().connectionChanged(false);
			await tick();

			expect(closeModal).not.toHaveBeenCalled();
			expect(screen.getByRole('alert').textContent).toContain(
				'The connection dropped before the install finished.'
			);
		});

		it('cannot be dismissed with Escape, Cancel or the dialog close while its install is unanswered', async () => {
			const dialog = document.createElement('div');
			dialog.setAttribute('role', 'dialog');
			const dialogClose = document.createElement('button');
			const content = document.createElement('div');
			dialog.append(dialogClose, content);
			document.body.append(dialog);
			const dismissed = vi.fn();
			dialogClose.addEventListener('click', dismissed);
			const onKeydown = (event: KeyboardEvent) => {
				if (event.key === 'Escape') dismissed();
			};
			window.addEventListener('keydown', onKeydown);
			try {
				open('client-abc', content);
				await fireEvent.click(screen.getByRole('button', { name: 'Choose archive…' }));
				await analyzed();
				await fireEvent.click(installButton());
				const name = screen.getByLabelText('Name');

				await fireEvent.keyDown(name, { key: 'Escape' });
				await fireEvent.click(dialogClose);
				expect(dismissed).not.toHaveBeenCalled();
				expect((screen.getByRole('button', { name: 'Cancel' }) as HTMLButtonElement).disabled).toBe(
					true
				);

				await installReply({ error: { code: 'io', message: 'no' } });
				await fireEvent.keyDown(name, { key: 'Escape' });
				await fireEvent.click(dialogClose);
				expect(dismissed).toHaveBeenCalledTimes(2);
				expect((screen.getByRole('button', { name: 'Cancel' }) as HTMLButtonElement).disabled).toBe(
					false
				);
			} finally {
				window.removeEventListener('keydown', onKeydown);
				dialog.remove();
			}
		});
	});
});
