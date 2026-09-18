// @vitest-environment jsdom
import type { Server } from '$types';
import { fireEvent, render, screen } from '@testing-library/svelte';
import { beforeEach, describe, expect, it, vi } from 'vitest';

const { holder, toast, modal, sendAndWait } = vi.hoisted(() => ({
	holder: { server: undefined as unknown },
	toast: { add: vi.fn() },
	modal: { showModal: vi.fn(), showConfirmModal: vi.fn() },
	sendAndWait: vi.fn()
}));

vi.mock('$lib/utils/websocketUtils', () => ({
	send: vi.fn(),
	sendAndWait: (type: unknown, data?: unknown) => sendAndWait(type, data)
}));

vi.mock('svelte-jsoneditor', () => ({ JSONEditor: () => {} }));

vi.mock('$states', async () => {
	const { ServerState } = await import('$lib/states/serverState.svelte');
	holder.server = new ServerState();
	return {
		getServerState: () => holder.server,
		getToastState: () => toast,
		getModalState: () => modal
	};
});

import { ServerState } from '$lib/states/serverState.svelte';
import ServerConsole from '../ServerConsole.svelte';

const state = () => holder.server as ServerState;

function server(overrides: Partial<Server> = {}): Server {
	return {
		id: 7,
		name: 'Main World',
		server_type: 'native',
		launch_args: '',
		status: { status: 'running', running: true },
		...overrides
	} as Server;
}

async function enable() {
	render(ServerConsole, { server: server() });
	await fireEvent.click(screen.getByRole('button', { name: 'Enable' }));
	await vi.waitFor(() => expect(toast.add).toHaveBeenCalled());
}

beforeEach(() => {
	toast.add.mockReset();
	sendAndWait.mockReset();
	modal.showConfirmModal.mockReset().mockResolvedValue(true);
	holder.server = new ServerState();
	state().servers = [server()];
	state().selectedServer = server();
});

describe('ServerConsole enabling the world data endpoint', () => {
	it('stores the saved launch argument of a coded refusal and explains it', async () => {
		sendAndWait.mockResolvedValue({
			...server({ launch_args: '-enable-gamedata-api' }),
			error: { code: 'apply_in_progress', message: 'raw', target_id: 'server-7' }
		});

		await enable();

		expect(state().servers[0].launch_args).toBe('-enable-gamedata-api');
		expect(state().servers[0]).not.toHaveProperty('error');
		expect(state().selectedServer?.launch_args).toBe('-enable-gamedata-api');
		expect(toast.add).toHaveBeenCalledWith(
			'Your changes were saved, but the server was not restarted because PalStudio is changing its mods. Restart it when that finishes.',
			'Warning',
			'warning'
		);
		expect(toast.add.mock.calls.some(([text]) => String(text).includes('[object Object]'))).toBe(
			false
		);
	});

	it('shows a refusal given as a message and stores nothing', async () => {
		sendAndWait.mockResolvedValue({ error: 'Server not found' });

		await enable();

		expect(toast.add).toHaveBeenCalledWith('Server not found', 'Error', 'error');
		expect(state().servers).toEqual([server()]);
	});
});
