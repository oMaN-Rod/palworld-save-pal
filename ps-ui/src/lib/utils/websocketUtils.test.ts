import { afterEach, describe, expect, it, vi } from 'vitest';

const socket = vi.hoisted(() => ({ send: vi.fn(), sendBytes: vi.fn() }));

vi.mock('$states/websocketState.svelte', () => ({
	getSocketState: () => socket
}));

import { MessageType } from '$types';
import { send, sendBytes } from './websocketUtils';

async function expectLoggedNotUnhandled(run: () => void, failure: Error, type: string) {
	const logged = vi.spyOn(console, 'error').mockImplementation(() => {});
	const unhandled = vi.fn();
	process.on('unhandledRejection', unhandled);
	try {
		run();
		await new Promise((resolve) => setTimeout(resolve, 0));
	} finally {
		process.off('unhandledRejection', unhandled);
	}
	expect(logged).toHaveBeenCalledWith(expect.stringContaining(type), failure);
	expect(unhandled).not.toHaveBeenCalled();
}

describe('fire-and-forget sends', () => {
	afterEach(() => {
		vi.restoreAllMocks();
		socket.send.mockReset();
		socket.sendBytes.mockReset();
	});

	it('logs a transport that rejects instead of leaving the rejection unhandled', async () => {
		const failure = new Error('RemoteTransport disposed');
		socket.send.mockImplementation(() => Promise.reject(failure));

		await expectLoggedNotUnhandled(() => send(MessageType.MOD_LIST), failure, 'mod_list');

		expect(socket.send).toHaveBeenCalledWith(JSON.stringify({ type: 'mod_list' }));
	});

	it('logs a rejected byte transfer the same way', async () => {
		const failure = new Error('File transfer is not available over a remote session');
		socket.sendBytes.mockImplementation(() => Promise.reject(failure));
		const bytes = new Uint8Array([1, 2, 3]);

		await expectLoggedNotUnhandled(
			() => sendBytes(MessageType.MOD_LIST, bytes),
			failure,
			'mod_list'
		);

		expect(socket.sendBytes).toHaveBeenCalledWith('mod_list', bytes);
	});
});
