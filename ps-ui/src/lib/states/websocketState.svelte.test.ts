import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import type { WSHandlerContext } from '$lib/ws/types';

vi.mock('$env/static/public', () => ({ PUBLIC_WS_URL: 'localhost:9999' }));

class FakeWebSocket {
	static readonly CONNECTING = 0;
	static readonly OPEN = 1;
	static readonly CLOSING = 2;
	static readonly CLOSED = 3;

	readonly OPEN = FakeWebSocket.OPEN;
	readyState = FakeWebSocket.OPEN;
	onopen: (() => void) | null = null;
	onmessage: ((event: { data: string }) => void) | null = null;
	onclose: (() => void) | null = null;
	sent: string[] = [];

	constructor(readonly url: string) {}

	send(data: string) {
		this.sent.push(data);
	}

	receive(payload: unknown) {
		this.onmessage?.({ data: JSON.stringify(payload) });
	}

	close() {
		this.readyState = FakeWebSocket.CLOSED;
		this.onclose?.();
	}
}

let sockets: FakeWebSocket[] = [];

beforeEach(() => {
	sockets = [];
	vi.useFakeTimers();
	vi.stubGlobal(
		'WebSocket',
		vi.fn().mockImplementation((url: string) => {
			const ws = new FakeWebSocket(url);
			sockets.push(ws);
			return ws;
		})
	);
	vi.stubGlobal('window', { location: { protocol: 'https:' } });
});

afterEach(() => {
	vi.unstubAllGlobals();
	vi.useRealTimers();
	vi.resetModules();
});

const context: WSHandlerContext = { goto: (async () => {}) as WSHandlerContext['goto'] };

describe('SocketState.sendAndWait', () => {
	it('rejects a request in flight when the socket closes, instead of hanging forever', async () => {
		const { getSocketState } = await import('./websocketState.svelte');
		const socket = getSocketState();
		socket.connect(context);
		const ws = sockets[0];

		const pending = socket.sendAndWait({ type: 'ping' });
		let settled = false;
		const guard = pending.catch(() => {});
		guard.finally(() => {
			settled = true;
		});

		ws.close();
		await guard;

		expect(settled).toBe(true);
		await expect(pending).rejects.toThrow();
	});

	it('lets a caller with a catch observe the rejection cleanly', async () => {
		const { getSocketState } = await import('./websocketState.svelte');
		const socket = getSocketState();
		socket.connect(context);
		const ws = sockets[0];

		let caught: unknown;
		const handled = socket.sendAndWait({ type: 'ping' }).catch((err) => {
			caught = err;
		});

		ws.close();
		await handled;

		expect(caught).toBeInstanceOf(Error);
	});

	it('still resolves requests made against the reconnected socket', async () => {
		const { getSocketState } = await import('./websocketState.svelte');
		const socket = getSocketState();
		socket.connect(context);
		const first = sockets[0];

		const dropped = socket.sendAndWait({ type: 'ping' }).catch(() => {});
		first.close();
		await dropped;

		await vi.advanceTimersByTimeAsync(5000);
		expect(sockets.length).toBe(2);

		const second = sockets[1];
		const pending = socket.sendAndWait({ type: 'pong' });
		second.receive({ type: 'pong', ok: true });

		await expect(pending).resolves.toEqual({ type: 'pong', ok: true });
	});

	it('does not reject a request whose reply already arrived before the socket closes later', async () => {
		const { getSocketState } = await import('./websocketState.svelte');
		const socket = getSocketState();
		socket.connect(context);
		const ws = sockets[0];

		const pending = socket.sendAndWait({ type: 'ping' });
		ws.receive({ type: 'ping', ok: true });
		await expect(pending).resolves.toEqual({ type: 'ping', ok: true });

		ws.close();
	});
});
