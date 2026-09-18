// @vitest-environment jsdom
import type { NexusFile } from '$types';
import { MessageType } from '$types';
import { fireEvent, render, screen } from '@testing-library/svelte';
import { tick } from 'svelte';
import { beforeEach, describe, expect, it, vi } from 'vitest';

const { holder, send, env } = vi.hoisted(() => ({
	holder: { mods: undefined as unknown, nexus: undefined as unknown },
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
	}
}));

vi.mock('$states', async () => {
	const { ModsState } = await import('$lib/states/modsState.svelte');
	const { NexusState } = await import('$lib/states/nexusState.svelte');
	holder.mods = new ModsState();
	holder.nexus = new NexusState();
	return {
		getModsState: () => holder.mods,
		getNexusState: () => holder.nexus,
		downloadKey: (modId: number, fileId: number) => `${modId}:${fileId}`,
		getToastState: () => ({ add: vi.fn() })
	};
});

import type { ModsState } from '$lib/states/modsState.svelte';
import type { NexusState } from '$lib/states/nexusState.svelte';
import { nexusDownloadHandler, nexusModFilesHandler } from '$lib/ws/handlers/nexusHandler';
import NexusDetailDrawer from '../NexusDetailDrawer.svelte';

const context = {} as never;
const modsState = () => holder.mods as ModsState;
const nexusState = () => holder.nexus as NexusState;
const TARGET_ID = 'client-abc';
const MOD_ID = 4821;

function file(overrides: Partial<NexusFile> = {}): NexusFile {
	return {
		file_id: 99001,
		name: 'Cool Mod',
		version: '1.2.0',
		category: 'MAIN',
		date: 1700000000,
		size_in_bytes: 1024,
		uri: 'nxm://palworld/mods/4821/files/99001',
		primary: true,
		description: null,
		...overrides
	};
}

function sent(type: MessageType) {
	return send.mock.calls.filter((call) => call[0] === type);
}

function setPremiumAccount(): void {
	nexusState().hasKey = true;
	nexusState().account = {
		user_id: 1,
		name: 'Tester',
		is_premium: true,
		is_supporter: false,
		profile_url: null
	};
}

async function applyFiles(files: NexusFile[], latestFileId: number | null = null) {
	await nexusModFilesHandler.handle(
		{ mod_id: MOD_ID, files, latest_file_id: latestFileId },
		context
	);
	await tick();
}

beforeEach(() => {
	send.mockReset();
	env.desktop = 'true';
	modsState().reset();
	nexusState().reset();
});

describe('NexusDetailDrawer', () => {
	it('sends nexus_mod_files once for this mod, and not again once the files are cached', async () => {
		render(NexusDetailDrawer, { targetId: TARGET_ID, modId: MOD_ID, onClose: vi.fn() });

		expect(sent(MessageType.NEXUS_MOD_FILES)).toHaveLength(1);
		expect(sent(MessageType.NEXUS_MOD_FILES)[0][1]).toEqual({ mod_id: MOD_ID });

		await applyFiles([file()], 99001);

		modsState().resets += 1;
		await tick();
		expect(sent(MessageType.NEXUS_MOD_FILES)).toHaveLength(1);
	});

	it('renders one row per file with a uniquely named download button per file', async () => {
		setPremiumAccount();
		render(NexusDetailDrawer, { targetId: TARGET_ID, modId: MOD_ID, onClose: vi.fn() });
		await applyFiles([
			file({ file_id: 1, name: 'Main pack', version: '1.0' }),
			file({ file_id: 2, name: 'Optional extras', version: '1.0', category: 'OPTIONAL' })
		]);

		expect(screen.getByRole('button', { name: 'Download Main pack 1.0' })).toBeTruthy();
		expect(screen.getByRole('button', { name: 'Download Optional extras 1.0' })).toBeTruthy();
	});

	it('sends nexus_download with target_id/mod_id/file_id and no key or expires for a premium account', async () => {
		setPremiumAccount();
		render(NexusDetailDrawer, { targetId: TARGET_ID, modId: MOD_ID, onClose: vi.fn() });
		await applyFiles([file({ file_id: 99001, name: 'Cool Mod', version: '1.2.0' })]);

		await fireEvent.click(screen.getByRole('button', { name: 'Download Cool Mod 1.2.0' }));

		expect(sent(MessageType.NEXUS_DOWNLOAD)).toHaveLength(1);
		expect(sent(MessageType.NEXUS_DOWNLOAD)[0][1]).toEqual({
			target_id: TARGET_ID,
			mod_id: MOD_ID,
			file_id: 99001
		});
	});

	it('shows the browser sentence and sends no nexus_download with no key stored at all', async () => {
		render(NexusDetailDrawer, { targetId: TARGET_ID, modId: MOD_ID, onClose: vi.fn() });
		await applyFiles([file({ file_id: 99001, name: 'Cool Mod', version: '1.2.0' })]);

		expect(screen.getAllByText(/Nexus Mods Premium/).length).toBeGreaterThan(0);
		expect(
			screen.getByRole('button', { name: 'Download Cool Mod 1.2.0 in your browser' })
		).toBeTruthy();
		expect(sent(MessageType.NEXUS_DOWNLOAD)).toHaveLength(0);
	});

	it('shows the browser sentence and sends no nexus_download for a non-premium account', async () => {
		nexusState().account = {
			user_id: 1,
			name: 'Tester',
			is_premium: false,
			is_supporter: false,
			profile_url: null
		};
		render(NexusDetailDrawer, { targetId: TARGET_ID, modId: MOD_ID, onClose: vi.fn() });
		await applyFiles([file({ file_id: 99001, name: 'Cool Mod', version: '1.2.0' })]);

		expect(screen.getAllByText(/Nexus Mods Premium/).length).toBeGreaterThan(0);
		expect(
			screen.getByRole('button', { name: 'Download Cool Mod 1.2.0 in your browser' })
		).toBeTruthy();
		expect(sent(MessageType.NEXUS_DOWNLOAD)).toHaveLength(0);
	});

	it('disables the button and shows the Downloading line while downloadingFile is true', async () => {
		setPremiumAccount();
		render(NexusDetailDrawer, { targetId: TARGET_ID, modId: MOD_ID, onClose: vi.fn() });
		await applyFiles([file({ file_id: 99001, name: 'Cool Mod', version: '1.2.0' })]);

		const button = screen.getByRole('button', {
			name: 'Download Cool Mod 1.2.0'
		}) as HTMLButtonElement;
		await fireEvent.click(button);
		await tick();

		expect(button.disabled).toBe(true);
		expect(screen.getByText('Downloading Cool Mod 1.2.0…')).toBeTruthy();
	});

	it('renders a premium_required refusal as the browser sentence, not a raw error', async () => {
		setPremiumAccount();
		render(NexusDetailDrawer, { targetId: TARGET_ID, modId: MOD_ID, onClose: vi.fn() });
		await applyFiles([file({ file_id: 99001, name: 'Cool Mod', version: '1.2.0' })]);

		await fireEvent.click(screen.getByRole('button', { name: 'Download Cool Mod 1.2.0' }));
		await nexusDownloadHandler.handle(
			{
				target_id: TARGET_ID,
				nexus_mod_id: MOD_ID,
				file_id: 99001,
				error: { code: 'premium_required', message: 'This download needs Nexus Mods Premium' }
			},
			context
		);
		await tick();

		expect(screen.queryByRole('alert')).toBeNull();
		expect(screen.getAllByText(/Nexus Mods Premium/).length).toBeGreaterThan(0);
	});

	it('renders a nexus_mod_files refusal in place of the list', async () => {
		render(NexusDetailDrawer, { targetId: TARGET_ID, modId: MOD_ID, onClose: vi.fn() });
		await nexusModFilesHandler.handle(
			{ mod_id: MOD_ID, error: { code: 'network', message: 'no route' } },
			context
		);
		await tick();

		expect(screen.getByRole('alert').textContent).toContain('Nexus Mods could not be reached.');
		expect(screen.queryByRole('button', { name: /Download/ })).toBeNull();
	});

	it("keeps a cached mod's file list visible despite a stale refusal recorded for a different mod", async () => {
		setPremiumAccount();
		const OTHER_MOD_ID = 55;
		const { rerender } = render(NexusDetailDrawer, {
			targetId: TARGET_ID,
			modId: OTHER_MOD_ID,
			onClose: vi.fn()
		});
		await nexusModFilesHandler.handle(
			{
				mod_id: OTHER_MOD_ID,
				files: [file({ file_id: 1, name: 'Bravo', version: '1.0' })],
				latest_file_id: 1
			},
			context
		);
		await tick();
		expect(screen.getByRole('button', { name: 'Download Bravo 1.0' })).toBeTruthy();

		// Open a different mod (A) whose file list fails.
		await rerender({ targetId: TARGET_ID, modId: MOD_ID, onClose: vi.fn() });
		await nexusModFilesHandler.handle(
			{ mod_id: MOD_ID, error: { code: 'network', message: 'no route' } },
			context
		);
		await tick();
		expect(screen.getByRole('alert')).toBeTruthy();

		// Reopen the first mod (B): its files are already cached, so nothing re-requests them,
		// but A's stale refusal must not hide B's perfectly good list.
		await rerender({ targetId: TARGET_ID, modId: OTHER_MOD_ID, onClose: vi.fn() });
		await tick();

		expect(screen.queryByRole('alert')).toBeNull();
		expect(screen.getByRole('button', { name: 'Download Bravo 1.0' })).toBeTruthy();
	});

	it('disables every file while one download is outstanding, and attributes a refusal to the right file', async () => {
		setPremiumAccount();
		render(NexusDetailDrawer, { targetId: TARGET_ID, modId: MOD_ID, onClose: vi.fn() });
		await applyFiles([
			file({ file_id: 1, name: 'File One', version: '1.0' }),
			file({ file_id: 2, name: 'File Two', version: '1.0' })
		]);

		const button1 = screen.getByRole('button', {
			name: 'Download File One 1.0'
		}) as HTMLButtonElement;
		const button2 = screen.getByRole('button', {
			name: 'Download File Two 1.0'
		}) as HTMLButtonElement;

		await fireEvent.click(button1);
		await tick();

		expect(button1.disabled).toBe(true);
		expect(button2.disabled).toBe(true);

		send.mockClear();
		await fireEvent.click(button2);
		expect(sent(MessageType.NEXUS_DOWNLOAD)).toHaveLength(0);

		await nexusDownloadHandler.handle(
			{
				target_id: TARGET_ID,
				nexus_mod_id: MOD_ID,
				file_id: 1,
				error: { code: 'network', message: 'no route' }
			},
			context
		);
		await tick();

		const rows = screen.getAllByRole('listitem');
		const rowOne = rows.find((row) => row.textContent?.includes('File One'));
		const rowTwo = rows.find((row) => row.textContent?.includes('File Two'));
		expect(rowOne?.querySelector('[role="alert"]')).toBeTruthy();
		expect(rowTwo?.querySelector('[role="alert"]')).toBeNull();

		expect(button1.disabled).toBe(false);
		expect(button2.disabled).toBe(false);
	});
});
