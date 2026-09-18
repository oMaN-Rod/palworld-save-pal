import { MessageType } from '$types';
import { beforeEach, describe, expect, it, vi } from 'vitest';

const send = vi.fn();

vi.mock('$lib/utils/websocketUtils', () => ({
	send: (type: unknown, data?: unknown) => send(type, data),
	sendAndWait: vi.fn()
}));

import { ServerState } from './serverState.svelte';

function startSends(): number {
	return send.mock.calls.filter(([type]) => type === MessageType.START_SERVER).length;
}

beforeEach(() => {
	send.mockReset();
});

describe('ServerState connection', () => {
	it('lets a start be sent again after the connection drops', async () => {
		const state = new ServerState();
		state.connectionChanged(true);
		await state.startServer(7);
		state.saving = true;

		state.connectionChanged(false);

		expect(state.starting).toEqual({});
		expect(state.saving).toBe(false);
		await state.startServer(7);
		expect(startSends()).toBe(2);
	});

	it('clears a start sent during a remote gap once the remote transport reconnects', async () => {
		const state = new ServerState();
		state.connectionChanged(true, 'remote');
		state.connectionChanged(false, 'remote');
		await state.startServer(7);
		state.saving = true;

		state.connectionChanged(true, 'remote');

		expect(state.starting).toEqual({});
		expect(state.saving).toBe(false);
	});

	it('keeps a start queued during a desktop gap after the reconnect', async () => {
		const state = new ServerState();
		state.connectionChanged(true);
		state.connectionChanged(false);
		await state.startServer(7);

		state.connectionChanged(true);

		expect(state.starting).toEqual({ 7: true });
	});

	it('keeps a start on the first connection', async () => {
		const state = new ServerState();
		await state.startServer(7);

		state.connectionChanged(false, 'remote');
		state.connectionChanged(true, 'remote');

		expect(state.starting[7]).toBe(true);
	});

	it('clears starts and saving when the transport changes, never on the first transport', async () => {
		const state = new ServerState();
		state.transportChanged(null);
		await state.startServer(7);
		state.saving = true;

		state.transportChanged(null);
		expect(state.starting[7]).toBe(true);
		expect(state.saving).toBe(true);

		state.transportChanged({});
		expect(state.starting).toEqual({});
		expect(state.saving).toBe(false);
	});
});
