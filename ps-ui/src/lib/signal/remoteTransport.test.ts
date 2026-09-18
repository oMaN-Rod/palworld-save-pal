import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import type { WSHandlerContext } from '$lib/ws/types';
import type { SignalSession } from './session.svelte';
import { chunkFrame } from './ctlFraming';

const dispatcherMocks = vi.hoisted(() => ({ dispatch: vi.fn() }));
vi.mock('$lib/ws/dispatcher', () => ({
	getDispatcher: () => ({ dispatch: dispatcherMocks.dispatch })
}));

type CtlListener = (envelope: { type: string; data: unknown }) => void;

function createFakeSession(
	overrides: Partial<{ connected: boolean; channelOpen: boolean; state: string }> = {}
) {
	const listeners = new Set<CtlListener>();
	const sent: string[] = [];
	const session = {
		sent,
		listeners,
		connected: overrides.connected ?? true,
		channelOpen: overrides.channelOpen ?? overrides.connected ?? true,
		state: overrides.state ?? 'connected',
		message: {
			subscribe(fn: CtlListener) {
				listeners.add(fn);
				return () => listeners.delete(fn);
			}
		},
		sendRaw(text: string) {
			if (!session.channelOpen) {
				throw new Error('SignalSession: cannot send before the ctl channel is open');
			}
			sent.push(text);
		}
	};
	return session;
}

type FakeSession = ReturnType<typeof createFakeSession>;

function deliver(session: FakeSession, envelope: { type: string; data: unknown }) {
	for (const fn of session.listeners) fn(envelope);
}

function deliverChunked(session: FakeSession, frame: string) {
	let id = 0;
	for (const piece of chunkFrame(frame, () => id++)) {
		deliver(session, JSON.parse(piece));
	}
}

const frame = (type: string, data: unknown = null) => JSON.stringify({ type, data });

const fakeContext = { goto: vi.fn() } as unknown as WSHandlerContext;

describe('RemoteTransport', () => {
	let fakeSession: FakeSession;

	beforeEach(async () => {
		vi.useRealTimers();
		dispatcherMocks.dispatch.mockClear();
		fakeSession = createFakeSession();
	});

	afterEach(() => {
		vi.useRealTimers();
	});

	async function makeTransport() {
		const { RemoteTransport } = await import('./remoteTransport.svelte');
		return new RemoteTransport(fakeSession as unknown as SignalSession);
	}

	it('sends a small frame verbatim', async () => {
		const transport = await makeTransport();
		transport.connect(fakeContext);

		await transport.send(JSON.stringify({ type: 'ping', data: null }));

		expect(fakeSession.sent).toEqual([JSON.stringify({ type: 'ping', data: null })]);
	});

	it('splits a large frame into chunk envelopes each under 64 KiB', async () => {
		const transport = await makeTransport();
		transport.connect(fakeContext);

		const payload = 'x'.repeat(200 * 1024);
		await transport.send(JSON.stringify({ type: 'list_servers', data: payload }));

		expect(fakeSession.sent.length).toBeGreaterThan(1);
		for (const piece of fakeSession.sent) {
			expect(piece.length).toBeLessThanOrEqual(64 * 1024);
			expect(JSON.parse(piece).type).toBe('chunk');
		}
	});

	it('reassembles a chunked inbound frame and resolves a matching sendAndWait', async () => {
		const transport = await makeTransport();
		transport.connect(fakeContext);

		const waiting = transport.sendAndWait({ type: 'list_servers' });
		fakeSession.sent = [];

		const payload = 'y'.repeat(200 * 1024);
		const responseFrame = JSON.stringify({ type: 'list_servers', data: payload });
		deliverChunked(fakeSession, responseFrame);

		await expect(waiting).resolves.toEqual({ type: 'list_servers', data: payload });
	});

	it('sets transport.message and calls the dispatcher when nothing is waiting', async () => {
		const transport = await makeTransport();
		transport.connect(fakeContext);

		deliver(fakeSession, { type: 'source-status', data: { fps: 30 } });

		expect(transport.message).toEqual({ type: 'source-status', data: { fps: 30 } });
		expect(dispatcherMocks.dispatch).toHaveBeenCalledWith(
			{ type: 'source-status', data: { fps: 30 } },
			fakeContext
		);
	});

	it('rejects sendAndWait with a timeout after 30 seconds', async () => {
		vi.useFakeTimers();
		const transport = await makeTransport();
		transport.connect(fakeContext);

		const waiting = transport.sendAndWait({ type: 'list_servers' });
		const assertion = expect(waiting).rejects.toEqual(new Error('timeout: list_servers'));
		await vi.advanceTimersByTimeAsync(30_000);
		await assertion;
	});

	it('rejects sendAndWait promptly when sendRaw throws, leaving no timers behind', async () => {
		vi.useFakeTimers();
		fakeSession.sendRaw = () => {
			throw new Error('SignalSession: cannot send before the ctl channel is open');
		};
		const transport = await makeTransport();
		transport.connect(fakeContext);

		await expect(transport.sendAndWait({ type: 'list_servers' })).rejects.toThrow(
			'SignalSession: cannot send before the ctl channel is open'
		);
		expect(vi.getTimerCount()).toBe(0);
	});

	it("does not let an older call's timeout delete a newer call's waiter", async () => {
		vi.useFakeTimers();
		const transport = await makeTransport();
		transport.connect(fakeContext);

		const waitingA = transport.sendAndWait({ type: 'list_servers' });
		const rejectionA = expect(waitingA).rejects.toEqual(new Error('timeout: list_servers'));
		await vi.advanceTimersByTimeAsync(1_000);
		const waitingB = transport.sendAndWait({ type: 'list_servers' });

		await vi.advanceTimersByTimeAsync(29_000);
		await rejectionA;

		deliver(fakeSession, { type: 'list_servers', data: { servers: [] } });
		await expect(waitingB).resolves.toEqual({ type: 'list_servers', data: { servers: [] } });
	});

	it('rejects sendBytes', async () => {
		const transport = await makeTransport();
		transport.connect(fakeContext);

		await expect(transport.sendBytes('upload', new Uint8Array())).rejects.toThrow(
			'File transfer is not available over a remote session'
		);
	});

	it('mirrors the fake session connected flag', async () => {
		fakeSession = createFakeSession({ connected: false });
		const transport = await makeTransport();
		transport.connect(fakeContext);

		expect(transport.connected).toBe(false);
		fakeSession.connected = true;
		expect(transport.connected).toBe(true);
	});

	it('reports kind "remote"', async () => {
		const transport = await makeTransport();
		expect(transport.kind).toBe('remote');
	});

	describe('when the ctl channel is closed', () => {
		it('rejects a send at once and writes nothing, even while the session still reads connected', async () => {
			const transport = await makeTransport();
			transport.connect(fakeContext);
			fakeSession.channelOpen = false;

			await expect(transport.send(frame('mod_list'))).rejects.toThrow(
				'not sent: mod_list: channel closed'
			);

			expect(fakeSession.connected).toBe(true);
			expect(fakeSession.sent).toEqual([]);
		});

		it('writes a send made after the channel reopens exactly once and never the rejected one', async () => {
			vi.useFakeTimers();
			const transport = await makeTransport();
			transport.connect(fakeContext);
			fakeSession.channelOpen = false;
			await transport.send(frame('profile_apply', { target_id: 'server-1' })).catch(() => {});

			fakeSession.channelOpen = true;
			await transport.send(frame('mod_list'));
			await vi.advanceTimersByTimeAsync(60_000);

			expect(fakeSession.sent).toEqual([frame('mod_list')]);
			expect(vi.getTimerCount()).toBe(0);
		});

		it('rejects sendAndWait at once without writing and leaves no timers behind', async () => {
			vi.useFakeTimers();
			const transport = await makeTransport();
			transport.connect(fakeContext);
			fakeSession.channelOpen = false;

			await expect(transport.sendAndWait({ type: 'list_servers' })).rejects.toThrow(
				'not sent: list_servers: channel closed'
			);

			expect(fakeSession.sent).toEqual([]);
			expect(vi.getTimerCount()).toBe(0);
		});
	});

	describe('lastSessionId', () => {
		it('updates from a loaded_save_files frame', async () => {
			const transport = await makeTransport();
			transport.connect(fakeContext);

			expect(transport.lastSessionId).toBeNull();

			deliver(fakeSession, {
				type: 'loaded_save_files',
				data: { session_id: 'abc', level: 'Level.sav' }
			});

			expect(transport.lastSessionId).toBe('abc');
		});

		it('forgets a session the desktop no longer has, and only that one', async () => {
			const transport = await makeTransport();
			transport.connect(fakeContext);
			deliver(fakeSession, { type: 'loaded_save_files', data: { session_id: 'abc' } });

			deliver(fakeSession, { type: 'session_not_found', data: 'other' });
			expect(transport.lastSessionId).toBe('abc');

			deliver(fakeSession, { type: 'session_not_found', data: 'abc' });
			expect(transport.lastSessionId).toBeNull();
		});

		it('forgets the session once an eject is written, but not when the eject is refused', async () => {
			const transport = await makeTransport();
			transport.connect(fakeContext);
			deliver(fakeSession, { type: 'loaded_save_files', data: { session_id: 'abc' } });

			fakeSession.channelOpen = false;
			await transport.send(frame('eject_session', { session_id: 'abc' })).catch(() => {});
			expect(transport.lastSessionId).toBe('abc');

			fakeSession.channelOpen = true;
			await transport.send(frame('eject_session', { session_id: 'abc' }));
			expect(transport.lastSessionId).toBeNull();
		});
	});

	describe('dispose', () => {
		it('rejects every send after it and writes nothing', async () => {
			const transport = await makeTransport();
			transport.connect(fakeContext);

			transport.dispose();

			await expect(transport.send(frame('mod_list'))).rejects.toThrow(
				'not sent: mod_list: RemoteTransport disposed'
			);
			await expect(transport.sendAndWait({ type: 'list_servers' })).rejects.toThrow(/disposed/i);
			expect(fakeSession.sent).toEqual([]);
		});

		it('unsubscribes from the session so a later frame is never dispatched', async () => {
			const transport = await makeTransport();
			transport.connect(fakeContext);
			expect(fakeSession.listeners.size).toBe(1);

			transport.dispose();

			expect(fakeSession.listeners.size).toBe(0);
			deliver(fakeSession, { type: 'source-status', data: { fps: 30 } });
			expect(dispatcherMocks.dispatch).not.toHaveBeenCalled();
		});

		it('rejects a pending sendAndWait promptly and clears its timer', async () => {
			vi.useFakeTimers();
			const transport = await makeTransport();
			transport.connect(fakeContext);

			const waiting = transport.sendAndWait({ type: 'list_servers' });
			transport.dispose();

			await expect(waiting).rejects.toThrow(/disposed/i);
			expect(vi.getTimerCount()).toBe(0);
		});

		it('is safe to call twice', async () => {
			const transport = await makeTransport();
			transport.connect(fakeContext);

			expect(() => {
				transport.dispose();
				transport.dispose();
			}).not.toThrow();
		});
	});

	describe('resetFraming', () => {
		it('discards a pending partial id so a full frame reusing that id reassembles cleanly', async () => {
			const transport = await makeTransport();
			transport.connect(fakeContext);

			deliver(fakeSession, { type: 'chunk', data: { id: 0, part: 0, parts: 3, data: 'stale' } });

			transport.resetFraming();

			deliver(fakeSession, {
				type: 'chunk',
				data: { id: 0, part: 0, parts: 2, data: '{"type":"list_servers",' }
			});
			deliver(fakeSession, {
				type: 'chunk',
				data: { id: 0, part: 1, parts: 2, data: '"data":{"servers":[]}}' }
			});

			expect(transport.message).toEqual({ type: 'list_servers', data: { servers: [] } });
		});
	});
});
