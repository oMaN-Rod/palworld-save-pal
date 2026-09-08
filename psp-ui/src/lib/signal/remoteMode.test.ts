import { describe, expect, it, vi } from 'vitest';
import type { RemoteModeDeps } from './remoteMode.svelte';

const dispatcherMocks = vi.hoisted(() => ({ dispatch: vi.fn() }));
vi.mock('$lib/ws/dispatcher', () => ({
	getDispatcher: () => ({ dispatch: dispatcherMocks.dispatch })
}));

type CtlListener = (envelope: { type: string; data: unknown }) => void;

function createFakeSignalSession() {
	const listeners = new Set<CtlListener>();
	return {
		listeners,
		connected: true,
		sendRaw() {},
		message: {
			subscribe(fn: CtlListener) {
				listeners.add(fn);
				return () => listeners.delete(fn);
			}
		}
	};
}

function createFakeDeps(overrides: Partial<{ connected: boolean }> = {}) {
	const session = { connected: overrides.connected ?? true };
	const fakeTransport = { kind: 'remote' as const, dispose: vi.fn() };
	const setTransportDelegate = vi.fn();
	const resetTransportDelegate = vi.fn();
	const createTransport = vi.fn(() => fakeTransport);
	const deps: RemoteModeDeps = {
		getSession: () => session as any,
		createTransport: createTransport as any,
		setTransportDelegate,
		resetTransportDelegate
	};
	return { session, fakeTransport, setTransportDelegate, resetTransportDelegate, createTransport, deps };
}

describe('RemoteModeState', () => {
	it('throws and does not swap the transport when entering while disconnected', async () => {
		const { RemoteModeState } = await import('./remoteMode.svelte');
		const { deps, setTransportDelegate, createTransport } = createFakeDeps({ connected: false });
		const remoteMode = new RemoteModeState(deps);

		expect(() => remoteMode.enter()).toThrow();

		expect(setTransportDelegate).not.toHaveBeenCalled();
		expect(createTransport).not.toHaveBeenCalled();
		expect(remoteMode.active).toBe(false);
		expect(remoteMode.transport).toBeNull();
	});

	it('swaps the transport and flips active when entering while connected', async () => {
		const { RemoteModeState } = await import('./remoteMode.svelte');
		const { deps, fakeTransport, setTransportDelegate } = createFakeDeps({ connected: true });
		const remoteMode = new RemoteModeState(deps);

		remoteMode.enter();

		expect(remoteMode.active).toBe(true);
		expect(remoteMode.transport).toBe(fakeTransport);
		expect(setTransportDelegate).toHaveBeenCalledWith(fakeTransport);
	});

	it('disposes the transport before resetting the delegate on exit', async () => {
		const { RemoteModeState } = await import('./remoteMode.svelte');
		const { deps, fakeTransport, resetTransportDelegate } = createFakeDeps({ connected: true });
		const remoteMode = new RemoteModeState(deps);
		remoteMode.enter();

		remoteMode.exit();

		expect(fakeTransport.dispose).toHaveBeenCalledTimes(1);
		expect(resetTransportDelegate).toHaveBeenCalled();
		expect(fakeTransport.dispose.mock.invocationCallOrder[0]).toBeLessThan(
			resetTransportDelegate.mock.invocationCallOrder[0]
		);
		expect(remoteMode.active).toBe(false);
		expect(remoteMode.transport).toBeNull();
	});

	it('is a no-op when exiting while not active', async () => {
		const { RemoteModeState } = await import('./remoteMode.svelte');
		const { deps, resetTransportDelegate, fakeTransport } = createFakeDeps({ connected: true });
		const remoteMode = new RemoteModeState(deps);

		remoteMode.exit();

		expect(resetTransportDelegate).not.toHaveBeenCalled();
		expect(fakeTransport.dispose).not.toHaveBeenCalled();
	});

	it('is a no-op on a second enter call', async () => {
		const { RemoteModeState } = await import('./remoteMode.svelte');
		const { deps, setTransportDelegate, createTransport } = createFakeDeps({ connected: true });
		const remoteMode = new RemoteModeState(deps);

		remoteMode.enter();
		remoteMode.enter();

		expect(createTransport).toHaveBeenCalledTimes(1);
		expect(setTransportDelegate).toHaveBeenCalledTimes(1);
		expect(remoteMode.active).toBe(true);
	});

	it('an exit-then-reenter over one session dispatches each frame exactly once', async () => {
		const { RemoteModeState } = await import('./remoteMode.svelte');
		const { RemoteTransport } = await import('./remoteTransport.svelte');
		dispatcherMocks.dispatch.mockClear();

		const session = createFakeSignalSession();
		const remoteMode = new RemoteModeState({
			getSession: () => session as any,
			createTransport: (session) => new RemoteTransport(session),
			setTransportDelegate: vi.fn(),
			resetTransportDelegate: vi.fn()
		});
		const fakeContext = { goto: vi.fn() } as any;

		remoteMode.enter();
		const first = remoteMode.transport!;
		first.connect(fakeContext);
		const pendingOnFirst = first.sendAndWait({ type: 'list_servers' });

		remoteMode.exit();
		await expect(pendingOnFirst).rejects.toThrow(/disposed/i);
		expect(session.listeners.size).toBe(0);

		remoteMode.enter();
		const second = remoteMode.transport!;
		second.connect(fakeContext);
		expect(session.listeners.size).toBe(1);

		for (const listener of session.listeners) {
			listener({ type: 'source-status', data: { fps: 1 } });
		}

		expect(dispatcherMocks.dispatch).toHaveBeenCalledTimes(1);
		expect(dispatcherMocks.dispatch).toHaveBeenCalledWith(
			{ type: 'source-status', data: { fps: 1 } },
			fakeContext
		);
	});
});
