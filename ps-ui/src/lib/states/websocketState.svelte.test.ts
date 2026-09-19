import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import type { WSHandlerContext } from '$lib/ws/types';

// Mutable so individual tests can exercise the empty (same-origin) build.
// Real deployments bake host:port/ws (see dev.sh write_web_env).
let publicWsUrl = 'localhost:9999/ws';
vi.mock('$env/static/public', () => ({ get PUBLIC_WS_URL() { return publicWsUrl; } }));

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
	publicWsUrl = 'localhost:9999/ws';
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
	it('dials the baked address when PUBLIC_WS_URL is set', async () => {
		const { getSocketState } = await import('./websocketState.svelte');
		getSocketState().connect(context);

		expect(sockets[0].url.startsWith('wss://localhost:9999/ws/')).toBe(true);
	});

	it('falls back to the page origin when PUBLIC_WS_URL is empty (server build)', async () => {
		// The installed/webapp SPA is served BY the server, so the websocket
		// must follow whatever host served the page — localhost, a LAN IP, or
		// a tailscale Funnel domain — never a build-time address.
		publicWsUrl = '';
		vi.stubGlobal('window', { location: { protocol: 'https:', host: 'pal.example.ts.net' } });
		const { getSocketState } = await import('./websocketState.svelte');
		getSocketState().connect(context);

		expect(sockets[0].url.startsWith('wss://pal.example.ts.net/ws/')).toBe(true);
	});

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
