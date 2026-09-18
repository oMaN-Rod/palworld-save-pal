// @vitest-environment jsdom
import type { LibraryMod, ModUpdate } from '$types';
import { MessageType } from '$types';
import { fireEvent, render, screen, within } from '@testing-library/svelte';
import { tick } from 'svelte';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import '../../live/__tests__/fixtures/animatePolyfill';

const { holder, send, remote, env } = vi.hoisted(() => ({
	holder: { mods: undefined as unknown, nexus: undefined as unknown },
	send: vi.fn(),
	remote: { active: false },
	env: { desktop: 'true' }
}));

vi.mock('$lib/signal/remoteMode.svelte', () => ({ getRemoteMode: () => remote }));

vi.mock('$env/static/public', () => ({
	get PUBLIC_DESKTOP_MODE() {
		return env.desktop;
	}
}));

vi.mock('$lib/utils/websocketUtils', () => ({
	send: (type: unknown, data?: unknown) => send(type, data),
	sendAndWait: vi.fn(),
	sendBytes: vi.fn(),
	isReady: () => true,
	pushProgressMessage: vi.fn()
}));

vi.mock('$states', async () => {
	const { ModsState } = await import('$lib/states/modsState.svelte');
	const { NexusState } = await import('$lib/states/nexusState.svelte');
	holder.mods = new ModsState();
	holder.nexus = new NexusState();
	return {
		getModsState: () => holder.mods,
		getNexusState: () => holder.nexus,
		getModalState: () => ({ showModal: vi.fn(), showConfirmModal: vi.fn() }),
		getServerState: () => ({ servers: [] }),
		downloadKey: (modId: number, fileId: number) => `${modId}:${fileId}`
	};
});

import type { ModsState } from '$lib/states/modsState.svelte';
import type { NexusState } from '$lib/states/nexusState.svelte';
import { nexusDownloadHandler, modUpdateIgnoreHandler } from '$lib/ws/handlers/nexusHandler';
import ModCard from '../ModCard.svelte';
import { applyResult, libraryMod, modTarget, modVersion } from './fixtures';

const context = {} as never;
const modsState = () => holder.mods as ModsState;
const nexusState = () => holder.nexus as NexusState;
const TARGET_ID = 'client-abc';
const NEXUS_MOD_ID = 4821;

function update(overrides: Partial<ModUpdate> = {}): ModUpdate {
	return {
		mod_id: 'nexus-4821',
		nexus_mod_id: NEXUS_MOD_ID,
		installed_version: '1.2.0',
		latest: {
			file_id: 99002,
			name: 'Enhanced Visuals',
			version: '1.3.0',
			category: 'MAIN',
			date: 0,
			size_in_bytes: null,
			uri: 'file.zip',
			primary: true,
			description: null
		},
		state: 'available',
		ignored_version: null,
		...overrides
	};
}

function mod(overrides: Partial<LibraryMod> = {}): LibraryMod {
	return libraryMod({
		id: 'nexus-4821',
		name: 'Enhanced Visuals',
		versions: [modVersion({ id: 'nexus-4821-v1', mod_id: 'nexus-4821' })],
		current_version_id: 'nexus-4821-v1',
		...overrides
	});
}

function sent(type: MessageType) {
	return send.mock.calls.filter((call) => call[0] === type);
}

async function openMenu() {
	await fireEvent.click(screen.getByRole('button', { name: /Actions for/ }));
}

beforeEach(() => {
	send.mockReset();
	remote.active = false;
	env.desktop = 'true';
	modsState().reset();
	nexusState().reset();
	modsState().targets = [modTarget({ id: TARGET_ID })];
});

function renderCard(overrides: Partial<LibraryMod> = {}) {
	return render(ModCard, {
		mod: mod(overrides),
		targetId: TARGET_ID,
		layout: 'list',
		onDetails: vi.fn()
	});
}

describe('ModCard update pill and actions', () => {
	it('renders the pill and menu item for an available update, and sends nexus_download for the latest file when pressed', async () => {
		nexusState().updates = { 'nexus-4821': update() };
		renderCard();

		expect(screen.getByText('Update available: 1.3.0')).toBeTruthy();

		await openMenu();
		await fireEvent.click(
			screen.getByRole('menuitem', { name: 'Update Enhanced Visuals to 1.3.0' })
		);

		expect(sent(MessageType.NEXUS_DOWNLOAD)).toHaveLength(1);
		expect(sent(MessageType.NEXUS_DOWNLOAD)[0][1]).toEqual({
			target_id: TARGET_ID,
			mod_id: NEXUS_MOD_ID,
			file_id: 99002
		});
	});

	it('offers Ignore for an available update, sending mod_update_ignore with the version', async () => {
		nexusState().updates = { 'nexus-4821': update() };
		renderCard();

		await openMenu();
		await fireEvent.click(
			screen.getByRole('menuitem', { name: 'Ignore 1.3.0 for Enhanced Visuals' })
		);

		expect(sent(MessageType.MOD_UPDATE_IGNORE)).toHaveLength(1);
		expect(sent(MessageType.MOD_UPDATE_IGNORE)[0][1]).toEqual({
			mod_id: 'nexus-4821',
			version: '1.3.0'
		});
	});

	it('shows the quiet ignored line and offers to stop ignoring, sending mod_update_ignore with null', async () => {
		nexusState().updates = { 'nexus-4821': update({ state: 'ignored', ignored_version: '1.3.0' }) };
		renderCard();

		expect(screen.getByText('Ignoring 1.3.0')).toBeTruthy();
		expect(screen.queryByText('Update available: 1.3.0')).toBeNull();

		await openMenu();
		await fireEvent.click(
			screen.getByRole('menuitem', { name: 'Stop ignoring updates for Enhanced Visuals' })
		);

		expect(sent(MessageType.MOD_UPDATE_IGNORE)).toHaveLength(1);
		expect(sent(MessageType.MOD_UPDATE_IGNORE)[0][1]).toEqual({
			mod_id: 'nexus-4821',
			version: null
		});
	});

	it('applies a real mod_update_ignore reply through the store', async () => {
		nexusState().updates = { 'nexus-4821': update() };
		renderCard();

		await modUpdateIgnoreHandler.handle(
			{ mod_id: 'nexus-4821', ignored_version: '1.3.0' },
			context
		);
		await tick();

		expect(screen.getByText('Ignoring 1.3.0')).toBeTruthy();
	});

	it('renders none of the update UI in a remote session', async () => {
		remote.active = true;
		nexusState().updates = { 'nexus-4821': update() };
		renderCard();

		expect(screen.queryByText('Update available: 1.3.0')).toBeNull();
		await openMenu();
		expect(screen.queryByRole('menuitem', { name: /Update Enhanced Visuals/ })).toBeNull();
	});

	it('renders none of the update UI outside the desktop app', async () => {
		env.desktop = 'false';
		nexusState().updates = { 'nexus-4821': update() };
		renderCard();

		expect(screen.queryByText('Update available: 1.3.0')).toBeNull();
		await openMenu();
		expect(screen.queryByRole('menuitem', { name: /Update Enhanced Visuals/ })).toBeNull();
	});

	it('chains set-current and apply once the download installs, and shows the pending line until apply resolves', async () => {
		nexusState().updates = { 'nexus-4821': update() };
		const setCurrentVersion = vi.spyOn(modsState(), 'setCurrentVersion').mockImplementation(() => {});
		const apply = vi.spyOn(modsState(), 'apply').mockImplementation(() => {});
		renderCard();

		await openMenu();
		await fireEvent.click(
			screen.getByRole('menuitem', { name: 'Update Enhanced Visuals to 1.3.0' })
		);

		await nexusDownloadHandler.handle(
			{
				target_id: TARGET_ID,
				nexus_mod_id: NEXUS_MOD_ID,
				file_id: 99002,
				version: '1.3.0',
				file_name: 'file.zip',
				mod_id: 'nexus-4821',
				version_id: 'nexus-4821@1.3.0',
				manifest: undefined
			},
			context
		);
		await tick();

		expect(setCurrentVersion).toHaveBeenCalledWith('nexus-4821', 'nexus-4821@1.3.0');
		expect(apply).toHaveBeenCalledWith(TARGET_ID);
		expect(screen.getByText('Updating Enhanced Visuals…')).toBeTruthy();
		expect(screen.queryByText('Update available: 1.3.0')).toBeNull();

		modsState().lastApply = {
			[TARGET_ID]: applyResult({ target_id: TARGET_ID, request_id: 'apply-1' })
		};
		await tick();

		expect(screen.queryByText('Updating Enhanced Visuals…')).toBeNull();
	});

	it('chains a download this row did not request, started elsewhere for the same mod and file', async () => {
		nexusState().updates = { 'nexus-4821': update() };
		const setCurrentVersion = vi.spyOn(modsState(), 'setCurrentVersion').mockImplementation(() => {});
		const apply = vi.spyOn(modsState(), 'apply').mockImplementation(() => {});
		renderCard();

		// No click here: e.g. the Discover drawer's own Download button started this.
		await nexusDownloadHandler.handle(
			{
				target_id: TARGET_ID,
				nexus_mod_id: NEXUS_MOD_ID,
				file_id: 99002,
				version: '1.3.0',
				file_name: 'file.zip',
				mod_id: 'nexus-4821',
				version_id: 'nexus-4821@1.3.0',
				manifest: undefined
			},
			context
		);
		await tick();

		expect(setCurrentVersion).toHaveBeenCalledWith('nexus-4821', 'nexus-4821@1.3.0');
		expect(apply).toHaveBeenCalledWith(TARGET_ID);
	});

	it('chains a download that completes after the row is unmounted and remounted mid-download', async () => {
		nexusState().updates = { 'nexus-4821': update() };
		const setCurrentVersion = vi.spyOn(modsState(), 'setCurrentVersion').mockImplementation(() => {});
		const apply = vi.spyOn(modsState(), 'apply').mockImplementation(() => {});
		const first = renderCard();

		await openMenu();
		await fireEvent.click(
			screen.getByRole('menuitem', { name: 'Update Enhanced Visuals to 1.3.0' })
		);

		first.unmount();
		renderCard();

		await nexusDownloadHandler.handle(
			{
				target_id: TARGET_ID,
				nexus_mod_id: NEXUS_MOD_ID,
				file_id: 99002,
				version: '1.3.0',
				file_name: 'file.zip',
				mod_id: 'nexus-4821',
				version_id: 'nexus-4821@1.3.0',
				manifest: undefined
			},
			context
		);
		await tick();

		expect(setCurrentVersion).toHaveBeenCalledWith('nexus-4821', 'nexus-4821@1.3.0');
		expect(apply).toHaveBeenCalledWith(TARGET_ID);
	});

	it('renders a mod_update_ignore refusal for this mod beside the other refusals', async () => {
		nexusState().updates = { 'nexus-4821': update() };
		renderCard();

		await openMenu();
		await fireEvent.click(
			screen.getByRole('menuitem', { name: 'Ignore 1.3.0 for Enhanced Visuals' })
		);

		await modUpdateIgnoreHandler.handle(
			{ mod_id: 'nexus-4821', error: { code: 'mod_not_found', message: 'no library mod nexus-4821' } },
			context
		);
		await tick();

		expect(await screen.findByText('no library mod nexus-4821')).toBeTruthy();
	});

	it('renders a nexus_download refusal for this mod beside the other refusals', async () => {
		nexusState().updates = { 'nexus-4821': update() };
		renderCard();

		await openMenu();
		await fireEvent.click(
			screen.getByRole('menuitem', { name: 'Update Enhanced Visuals to 1.3.0' })
		);

		await nexusDownloadHandler.handle(
			{
				target_id: TARGET_ID,
				nexus_mod_id: NEXUS_MOD_ID,
				file_id: 99002,
				error: { code: 'network', message: 'no route' }
			},
			context
		);
		await tick();

		expect(await screen.findByText('Nexus Mods could not be reached.')).toBeTruthy();
	});
});
