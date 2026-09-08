import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { deriveKeys, normalizeCode, seal } from './crypto';
import { loadStoredDesktop, saveStoredDesktop, type StoredDesktop } from './credentialStore';
import {
	createSignalSession,
	parsePairingFragment,
	type BrokerSocketLike,
	type DataChannelLike,
	type PeerConnectionLike,
	type SignalSession
} from './session.svelte';

const HOST_AAD = new TextEncoder().encode('host');

function bytesToBase64(bytes: Uint8Array): string {
	let binary = '';
	for (const byte of bytes) binary += String.fromCharCode(byte);
	return btoa(binary);
}

describe('parsePairingFragment', () => {
	it('extracts the code from a v1 fragment', () => {
		expect(parsePairingFragment('#v1.ABCD-EFGH-JKMN')).toBe('ABCD-EFGH-JKMN');
	});

	it('round-trips a fragment built from the extracted code', () => {
		const code = 'XY23-4567-89AB';
		const fragment = `#v1.${code}`;
		expect(parsePairingFragment(fragment)).toBe(code);
	});

	it('rejects an unknown fragment version', () => {
		expect(parsePairingFragment('#v2.ABCD-EFGH')).toBeNull();
	});

	it('rejects an empty string', () => {
		expect(parsePairingFragment('')).toBeNull();
	});

	it('rejects garbage with no version marker', () => {
		expect(parsePairingFragment('#not-a-pairing-code')).toBeNull();
	});

	it('rejects a v1 fragment with no code after the dot', () => {
		expect(parsePairingFragment('#v1.')).toBeNull();
	});
});

function createFakeDataChannel(label: string): DataChannelLike & { sentMessages: string[] } {
	return {
		label,
		readyState: 'connecting',
		sentMessages: [],
		send(data: string) {
			this.sentMessages.push(data);
		},
		close() {
			this.readyState = 'closed';
		},
		onopen: null,
		onclose: null,
		onmessage: null
	};
}

function createFakePeerConnection(channels: {
	ctl: DataChannelLike;
	live: DataChannelLike;
}): PeerConnectionLike {
	return {
		iceConnectionState: 'new',
		localDescription: null,
		onicecandidate: null,
		oniceconnectionstatechange: null,
		createDataChannel(label: string) {
			return label === 'ctl' ? channels.ctl : channels.live;
		},
		async createOffer() {
			return { type: 'offer', sdp: 'fake-offer-sdp' };
		},
		async setLocalDescription(desc) {
			this.localDescription = desc;
		},
		async setRemoteDescription() {},
		async addIceCandidate() {},
		close() {}
	};
}

function createFakeSocket(): BrokerSocketLike & { sent: string[] } {
	return {
		sent: [],
		onopen: null,
		onclose: null,
		onerror: null,
		onmessage: null,
		send(data: string) {
			this.sent.push(data);
		},
		close() {}
	};
}

type StubDocument = {
	hidden: boolean;
	addEventListener(type: string, fn: () => void): void;
	removeEventListener(type: string, fn: () => void): void;
	visibilityChanged(): void;
};

function stubDocument(): StubDocument {
	const listeners = new Set<() => void>();
	const doc: StubDocument = {
		hidden: false,
		addEventListener(type, fn) {
			if (type === 'visibilitychange') listeners.add(fn);
		},
		removeEventListener(type, fn) {
			if (type === 'visibilitychange') listeners.delete(fn);
		},
		visibilityChanged() {
			for (const fn of [...listeners]) fn();
		}
	};
	(globalThis as unknown as { document?: unknown }).document = doc;
	return doc;
}

function restoreDocument() {
	delete (globalThis as unknown as { document?: unknown }).document;
}

function stubLocalStorage() {
	const store = new Map<string, string>();
	(globalThis as unknown as { localStorage?: unknown }).localStorage = {
		getItem: (key: string) => store.get(key) ?? null,
		setItem: (key: string, value: string) => {
			store.set(key, value);
		},
		removeItem: (key: string) => {
			store.delete(key);
		}
	};
}

function restoreLocalStorage() {
	delete (globalThis as unknown as { localStorage?: unknown }).localStorage;
}

const BUSY_ROOM_HINT =
	'Another device is currently connected to this PC. Disconnect it first, or wait for it to drop.';

const PAIRING_BUSY_HINT =
	'Someone else is already pairing with this PC. Finish or stop that pairing first.';

async function expectNoFurtherDial(sockets: unknown[], expected: number) {
	let dialed = false;
	try {
		await vi.waitFor(() => expect(sockets.length).toBeGreaterThan(expected), {
			timeout: 300,
			interval: 20
		});
		dialed = true;
	} catch {
	}
	expect(dialed).toBe(false);
	expect(sockets).toHaveLength(expected);
}

const STORED_DESKTOP: StoredDesktop = {
	meetRoom: 'a'.repeat(32),
	deviceId: 'b'.repeat(32),
	deviceSecret: 'c'.repeat(64),
	desktopName: 'Living Room PC',
	pairedAtMs: 1_700_000_000_000
};

function createTrackingFactories() {
	const sockets: Array<BrokerSocketLike & { sent: string[] }> = [];
	const urls: string[] = [];
	const socketFactory = (url: string) => {
		urls.push(url);
		const socket = createFakeSocket();
		sockets.push(socket);
		return socket;
	};

	const pcs: PeerConnectionLike[] = [];
	const configs: RTCConfiguration[] = [];
	const channelsList: Array<{ ctl: ReturnType<typeof createFakeDataChannel>; live: ReturnType<typeof createFakeDataChannel> }> = [];
	const rtcFactory = (config: RTCConfiguration) => {
		configs.push(config);
		const ctl = createFakeDataChannel('ctl');
		const live = createFakeDataChannel('live');
		channelsList.push({ ctl, live });
		const pc = createFakePeerConnection({ ctl, live });
		pcs.push(pc);
		return pc;
	};

	return { socketFactory, rtcFactory, sockets, urls, pcs, configs, channelsList };
}

function trackedSession(factories: ReturnType<typeof createTrackingFactories>) {
	return createSignalSession({
		brokerUrl: 'ws://test-broker',
		socketFactory: factories.socketFactory,
		rtcFactory: factories.rtcFactory
	});
}

function autoResumeOnMount(session: SignalSession, desktop: StoredDesktop): boolean {
	if (!session.canAutoResume) return false;
	session.resumeStored(desktop);
	return true;
}

async function connectAndOpen(
	factories: ReturnType<typeof createTrackingFactories>,
	attemptIndex: number
) {
	const socket = factories.sockets[attemptIndex];
	socket.onopen?.();
	await vi.waitFor(() => expect(socket.sent.length).toBeGreaterThan(0));
	const { ctl } = factories.channelsList[attemptIndex];
	ctl.readyState = 'open';
	ctl.onopen?.();
}

function buildConnectedSession() {
	const ctl = createFakeDataChannel('ctl');
	const live = createFakeDataChannel('live');
	const socket = createFakeSocket();
	const pc = createFakePeerConnection({ ctl, live });
	const session = createSignalSession({
		brokerUrl: 'ws://test-broker',
		socketFactory: () => socket,
		rtcFactory: () => pc
	});
	return { session, ctl, live, socket, pc };
}

describe('ctl channel correlation', () => {
	it('resolves sendAndWait when a matching {type,data} reply arrives on ctl', async () => {
		const { session, ctl, socket } = buildConnectedSession();

		await session.connect('TEST-CODE-1234');
		socket.onopen?.();
		await vi.waitFor(() => expect(socket.sent.length).toBeGreaterThan(0));

		ctl.readyState = 'open';
		ctl.onopen?.();
		expect(session.state).toBe('connected');

		const waitPromise = session.sendAndWait('status');
		expect(JSON.parse(ctl.sentMessages[0])).toEqual({ type: 'status' });

		ctl.onmessage?.({ data: JSON.stringify({ type: 'status', data: { running: true } }) });

		await expect(waitPromise).resolves.toEqual({ running: true });
	});

	it('routes an uncorrelated ctl message to message subscribers', async () => {
		const { session, ctl, socket } = buildConnectedSession();

		await session.connect('TEST-CODE-5678');
		socket.onopen?.();
		await vi.waitFor(() => expect(socket.sent.length).toBeGreaterThan(0));
		ctl.readyState = 'open';
		ctl.onopen?.();

		const received: Array<{ type: string; data: unknown }> = [];
		const unsubscribe = session.message.subscribe((frame) => received.push(frame));

		ctl.onmessage?.({ data: JSON.stringify({ type: 'source-status', data: { fps: 30 } }) });

		expect(received).toEqual([{ type: 'source-status', data: { fps: 30 } }]);
		unsubscribe();
	});

	it('throws from send when the ctl channel is not open', () => {
		const { session } = buildConnectedSession();
		expect(() => session.send('status')).toThrow();
	});

	it('throws NotSupported from sendBytes in the MVP', () => {
		const { session } = buildConnectedSession();
		expect(() => session.sendBytes('blob', new Uint8Array([1, 2, 3]))).toThrow();
	});

	it('rejects a pending sendAndWait when disconnect tears the session down', async () => {
		const { session, ctl, socket } = buildConnectedSession();

		await session.connect('TEST-CODE-4242');
		socket.onopen?.();
		await vi.waitFor(() => expect(socket.sent.length).toBeGreaterThan(0));
		ctl.readyState = 'open';
		ctl.onopen?.();

		const waitPromise = session.sendAndWait('status');
		session.disconnect();

		await expect(waitPromise).rejects.toThrow();
		expect(session.state).toBe('idle');
	});
});

describe('per-stage timeouts', () => {
	beforeEach(() => {
		vi.useFakeTimers();
	});

	afterEach(() => {
		vi.useRealTimers();
	});

	it('fails with pairing-failed when no answer arrives before the 20s deadline', async () => {
		const { session, socket } = buildConnectedSession();

		await session.connect('WRONG-CODE-0000');
		socket.onopen?.();

		expect(session.state).toBe('connecting');
		expect(session.failure).toBeNull();

		await vi.advanceTimersByTimeAsync(20_000);

		expect(session.state).toBe('failed');
		expect(session.failure).toBe('pairing-failed');
		expect(session.failureHint).toMatch(/didn't reach a waiting app/);
	});

	it('fails a stored dial with the "PC not reachable" hint, not the code-entry hint', async () => {
		const { socketFactory, rtcFactory, sockets } = createTrackingFactories();
		const session = createSignalSession({
			brokerUrl: 'ws://test-broker',
			socketFactory,
			rtcFactory
		});

		await session.connectStored(STORED_DESKTOP);
		sockets[0].onopen?.();

		expect(session.state).toBe('connecting');
		expect(session.failure).toBeNull();

		await vi.advanceTimersByTimeAsync(20_000);

		expect(session.state).toBe('failed');
		expect(session.failure).toBe('pairing-failed');
		expect(session.failureHint).toBe(
			"Your PC isn't reachable — make sure Palworld Save Pal is running with remote access armed."
		);
	});

	it('fails with broker-unreachable when the socket never opens within 10s', async () => {
		const { session } = buildConnectedSession();

		await session.connect('SOME-CODE-0000');

		await vi.advanceTimersByTimeAsync(10_000);

		expect(session.state).toBe('failed');
		expect(session.failure).toBe('broker-unreachable');
	});

	it('maps direct-connect-failed to its own distinct hint, never the pairing-failed one', async () => {
		const { session, socket } = buildConnectedSession();

		await session.connect('SOME-CODE-1111');
		socket.onopen?.();
		await vi.advanceTimersByTimeAsync(20_000);
		expect(session.failure).toBe('pairing-failed');

		const { session: other, socket: otherSocket, pc: otherPc } = buildConnectedSession();
		await other.connect('SOME-CODE-2222');
		otherSocket.onopen?.();
		await vi.waitFor(() => expect(otherSocket.sent.length).toBeGreaterThan(0));
		otherPc.iceConnectionState = 'failed';
		otherPc.oniceconnectionstatechange?.();

		expect(other.failure).toBe('direct-connect-failed');
		expect(other.failureHint).not.toBe(session.failureHint);
		expect(other.failureHint).toMatch(/direct connection couldn't be established/);
	});

	it('disconnect clears every pending timer so a later tick cannot resurrect a stale failure', async () => {
		const { session, socket } = buildConnectedSession();

		await session.connect('SOME-CODE-3333');
		socket.onopen?.();
		session.disconnect();

		await vi.advanceTimersByTimeAsync(30_000);

		expect(session.state).toBe('idle');
		expect(session.failure).toBeNull();
	});
});

describe('live frame reassembly', () => {
	it('applies a complete single-message frame', async () => {
		const { session, live, socket } = buildConnectedSession();
		await session.connect('SOME-CODE-4444');
		socket.onopen?.();
		await Promise.resolve();

		const frame = { seq: 1, source: 'test', capturedAtMs: 1000, actors: [] };
		live.onmessage?.({ data: JSON.stringify(frame) });

		expect(session.lastFrame).toEqual(frame);
	});

	it('delivers each applied frame to onFrame listeners until unsubscribed', async () => {
		const { session, live, socket } = buildConnectedSession();
		await session.connect('SOME-CODE-7777');
		socket.onopen?.();
		await Promise.resolve();

		const seen: number[] = [];
		const off = session.onFrame((f) => seen.push(f.seq));

		live.onmessage?.({ data: JSON.stringify({ seq: 10, source: 'test', capturedAtMs: 1, actors: [] }) });
		live.onmessage?.({ data: JSON.stringify({ seq: 11, source: 'test', capturedAtMs: 2, actors: [] }) });
		expect(seen).toEqual([10, 11]);

		off();
		live.onmessage?.({ data: JSON.stringify({ seq: 12, source: 'test', capturedAtMs: 3, actors: [] }) });
		expect(seen).toEqual([10, 11]);
	});

	it('reassembles a frame split across two parts before applying it', async () => {
		const { session, live, socket } = buildConnectedSession();
		await session.connect('SOME-CODE-5555');
		socket.onopen?.();
		await Promise.resolve();

		const frame = { seq: 2, source: 'test', capturedAtMs: 2000, actors: [] };
		const text = JSON.stringify(frame);
		const half = Math.ceil(text.length / 2);

		live.onmessage?.({ data: JSON.stringify({ seq: 2, part: 0, parts: 2, data: text.slice(0, half) }) });
		expect(session.lastFrame).toBeNull();

		live.onmessage?.({ data: JSON.stringify({ seq: 2, part: 1, parts: 2, data: text.slice(half) }) });
		expect(session.lastFrame).toEqual(frame);
	});

	it('discards an incomplete older seq once a newer seq starts arriving', async () => {
		const { session, live, socket } = buildConnectedSession();
		await session.connect('SOME-CODE-6666');
		socket.onopen?.();
		await Promise.resolve();

		live.onmessage?.({ data: JSON.stringify({ seq: 3, part: 0, parts: 2, data: '{"seq":3,' }) });

		const frame = { seq: 4, source: 'test', capturedAtMs: 4000, actors: [] };
		live.onmessage?.({ data: JSON.stringify(frame) });
		expect(session.lastFrame).toEqual(frame);

		live.onmessage?.({ data: JSON.stringify({ seq: 3, part: 1, parts: 2, data: '"actors":[]}' }) });
		expect(session.lastFrame).toEqual(frame);
	});
});

describe('connection loss after connect', () => {
	it('treats ctl channel onclose as terminal and rejects pending waits', async () => {
		const { session, ctl, socket } = buildConnectedSession();

		await session.connect('TEST-CODE-9001');
		socket.onopen?.();
		await vi.waitFor(() => expect(socket.sent.length).toBeGreaterThan(0));
		ctl.readyState = 'open';
		ctl.onopen?.();
		expect(session.state).toBe('connected');

		const waitPromise = session.sendAndWait('status');
		ctl.onclose?.();

		await expect(waitPromise).rejects.toThrow();
		expect(session.state).toBe('failed');
		expect(session.failure).toBe('direct-connect-failed');
	});

	it('treats live channel onclose as terminal and rejects pending waits', async () => {
		const { session, ctl, live, socket } = buildConnectedSession();

		await session.connect('TEST-CODE-9002');
		socket.onopen?.();
		await vi.waitFor(() => expect(socket.sent.length).toBeGreaterThan(0));
		ctl.readyState = 'open';
		ctl.onopen?.();
		expect(session.state).toBe('connected');

		const waitPromise = session.sendAndWait('status');
		live.onclose?.();

		await expect(waitPromise).rejects.toThrow();
		expect(session.state).toBe('failed');
		expect(session.failure).toBe('direct-connect-failed');
	});

	it('treats ICE closed as terminal', async () => {
		const { session, ctl, socket, pc } = buildConnectedSession();

		await session.connect('TEST-CODE-9003');
		socket.onopen?.();
		await vi.waitFor(() => expect(socket.sent.length).toBeGreaterThan(0));
		ctl.readyState = 'open';
		ctl.onopen?.();
		expect(session.state).toBe('connected');

		pc.iceConnectionState = 'closed';
		pc.oniceconnectionstatechange?.();

		expect(session.state).toBe('failed');
		expect(session.failure).toBe('direct-connect-failed');
	});
});

describe('ICE disconnect grace period', () => {
	beforeEach(() => {
		vi.useFakeTimers();
	});

	afterEach(() => {
		vi.useRealTimers();
	});

	it('stays connected when ICE recovers from disconnected within 5s', async () => {
		const { session, ctl, socket, pc } = buildConnectedSession();

		await session.connect('SOME-CODE-9101');
		socket.onopen?.();
		await vi.waitFor(() => expect(socket.sent.length).toBeGreaterThan(0));
		ctl.readyState = 'open';
		ctl.onopen?.();
		expect(session.state).toBe('connected');

		pc.iceConnectionState = 'disconnected';
		pc.oniceconnectionstatechange?.();
		await vi.advanceTimersByTimeAsync(4_000);
		expect(session.state).toBe('connected');

		pc.iceConnectionState = 'connected';
		pc.oniceconnectionstatechange?.();
		await vi.advanceTimersByTimeAsync(5_000);

		expect(session.state).toBe('connected');
	});

	it('fails after 5s when ICE stays disconnected with no recovery', async () => {
		const { session, ctl, socket, pc } = buildConnectedSession();

		await session.connect('SOME-CODE-9102');
		socket.onopen?.();
		await vi.waitFor(() => expect(socket.sent.length).toBeGreaterThan(0));
		ctl.readyState = 'open';
		ctl.onopen?.();
		expect(session.state).toBe('connected');

		pc.iceConnectionState = 'disconnected';
		pc.oniceconnectionstatechange?.();
		await vi.advanceTimersByTimeAsync(5_000);

		expect(session.state).toBe('failed');
		expect(session.failure).toBe('direct-connect-failed');
	});
});

describe('sendAndWait same-type correlation (last-wins by design)', () => {
	beforeEach(() => {
		vi.useFakeTimers();
	});

	afterEach(() => {
		vi.useRealTimers();
	});

	it('resolves the newer call when a stale reply for an already-timed-out call arrives', async () => {
		const { session, ctl, socket } = buildConnectedSession();

		await session.connect('SOME-CODE-9301');
		socket.onopen?.();
		await vi.waitFor(() => expect(socket.sent.length).toBeGreaterThan(0));
		ctl.readyState = 'open';
		ctl.onopen?.();

		const waitA = session.sendAndWait('status');
		const waitARejection = expect(waitA).rejects.toThrow('timeout: status');
		await vi.advanceTimersByTimeAsync(10_000);
		await waitARejection;

		const waitB = session.sendAndWait('status');

		ctl.onmessage?.({ data: JSON.stringify({ type: 'status', data: { from: 'late-reply' } }) });

		await expect(waitB).resolves.toEqual({ from: 'late-reply' });
	});

	it("does not let call A's expired timeout clear or reject a same-type call B registered afterward", async () => {
		const { session, ctl, socket } = buildConnectedSession();

		await session.connect('SOME-CODE-9302');
		socket.onopen?.();
		await vi.waitFor(() => expect(socket.sent.length).toBeGreaterThan(0));
		ctl.readyState = 'open';
		ctl.onopen?.();

		const waitA = session.sendAndWait('status');
		const waitARejection = expect(waitA).rejects.toThrow('timeout: status');

		await vi.advanceTimersByTimeAsync(4_000);
		const waitB = session.sendAndWait('status');

		await vi.advanceTimersByTimeAsync(6_000);
		await waitARejection;

		ctl.onmessage?.({ data: JSON.stringify({ type: 'status', data: { from: 'B-reply' } }) });
		await expect(waitB).resolves.toEqual({ from: 'B-reply' });
	});
});

describe('sendAndWait timeout', () => {
	beforeEach(() => {
		vi.useFakeTimers();
	});

	afterEach(() => {
		vi.useRealTimers();
	});

	it('rejects with a timeout error when no reply arrives before the deadline', async () => {
		const { session, ctl, socket } = buildConnectedSession();

		await session.connect('SOME-CODE-9201');
		socket.onopen?.();
		await vi.waitFor(() => expect(socket.sent.length).toBeGreaterThan(0));
		ctl.readyState = 'open';
		ctl.onopen?.();

		const waitPromise = session.sendAndWait('status');
		const assertion = expect(waitPromise).rejects.toThrow('timeout: status');
		await vi.advanceTimersByTimeAsync(10_000);

		await assertion;
	});

	it('ignores a late reply after the deadline without an unhandled rejection', async () => {
		const { session, ctl, socket } = buildConnectedSession();

		await session.connect('SOME-CODE-9202');
		socket.onopen?.();
		await vi.waitFor(() => expect(socket.sent.length).toBeGreaterThan(0));
		ctl.readyState = 'open';
		ctl.onopen?.();

		const waitPromise = session.sendAndWait('status');
		const assertion = expect(waitPromise).rejects.toThrow('timeout: status');
		await vi.advanceTimersByTimeAsync(10_000);
		await assertion;

		expect(() => {
			ctl.onmessage?.({ data: JSON.stringify({ type: 'status', data: { running: true } }) });
		}).not.toThrow();
	});
});

describe('trickle ICE ordering', () => {
	it('buffers a candidate that arrives before the answer, then flushes it after setRemoteDescription', async () => {
		const ctl = createFakeDataChannel('ctl');
		const live = createFakeDataChannel('live');
		const socket = createFakeSocket();

		let remoteDescriptionSet = false;
		const addedCandidates: Array<{ candidate: unknown; afterAnswer: boolean }> = [];
		const pc: PeerConnectionLike = {
			iceConnectionState: 'new',
			localDescription: null,
			onicecandidate: null,
			oniceconnectionstatechange: null,
			createDataChannel(label: string) {
				return label === 'ctl' ? ctl : live;
			},
			async createOffer() {
				return { type: 'offer', sdp: 'fake-offer-sdp' };
			},
			async setLocalDescription(desc) {
				this.localDescription = desc;
			},
			async setRemoteDescription() {
				remoteDescriptionSet = true;
			},
			async addIceCandidate(candidate) {
				addedCandidates.push({ candidate, afterAnswer: remoteDescriptionSet });
			},
			close() {}
		};

		const session = createSignalSession({
			brokerUrl: 'ws://test-broker',
			socketFactory: () => socket,
			rtcFactory: () => pc
		});

		const code = 'SOME-CODE-8888';
		const { sealKey } = await deriveKeys(normalizeCode(code));

		await session.connect(code);
		socket.onopen?.();
		await vi.waitFor(() => expect(socket.sent.length).toBeGreaterThan(0));

		const candidate = { candidate: 'candidate:1 1 UDP 1 1.2.3.4 5000 typ host', sdpMid: '0' };
		const iceEnvelope = await seal(
			sealKey,
			new TextEncoder().encode(JSON.stringify({ t: 'ice', c: JSON.stringify(candidate) })),
			HOST_AAD
		);
		socket.onmessage?.({ data: bytesToBase64(iceEnvelope) });

		await new Promise((resolve) => setTimeout(resolve, 20));
		expect(addedCandidates).toHaveLength(0);

		const answerEnvelope = await seal(
			sealKey,
			new TextEncoder().encode(JSON.stringify({ t: 'answer', sdp: 'fake-answer-sdp' })),
			HOST_AAD
		);
		socket.onmessage?.({ data: bytesToBase64(answerEnvelope) });

		await vi.waitFor(() => expect(addedCandidates.length).toBeGreaterThan(0));
		expect(addedCandidates[0]).toEqual({ candidate, afterAnswer: true });
	});
});

describe('connectStored', () => {
	it('dials the meet room with kind=meet and wraps outbound signaling as {v,device,blob}', async () => {
		const { socketFactory, rtcFactory, sockets, urls, pcs } = createTrackingFactories();
		const session = createSignalSession({
			brokerUrl: 'ws://test-broker',
			socketFactory,
			rtcFactory
		});

		await session.connectStored(STORED_DESKTOP);
		const socket = sockets[0];
		socket.onopen?.();
		await vi.waitFor(() => expect(socket.sent.length).toBeGreaterThan(0));

		expect(urls[0]).toContain(`room=${STORED_DESKTOP.meetRoom}`);
		expect(urls[0]).toContain('kind=meet');

		const offerFrame = JSON.parse(socket.sent[0]);
		expect(offerFrame).toMatchObject({ v: 1, device: STORED_DESKTOP.deviceId });
		expect(typeof offerFrame.blob).toBe('string');

		const pc = pcs[0];
		pc.onicecandidate?.({
			candidate: { toJSON: () => ({ candidate: 'candidate:1 1 UDP 1 1.2.3.4 5000 typ host' }) }
		});
		await vi.waitFor(() => expect(socket.sent.length).toBeGreaterThan(1));

		const iceFrame = JSON.parse(socket.sent[1]);
		expect(iceFrame).toMatchObject({ v: 1, device: STORED_DESKTOP.deviceId });
		expect(typeof iceFrame.blob).toBe('string');
	});
});

describe('a room whose guest role is already taken', () => {
	it('fails a dial closed with 4010 and says which PC is busy, not which code is wrong', async () => {
		const factories = createTrackingFactories();
		const session = createSignalSession({
			brokerUrl: 'ws://test-broker',
			socketFactory: factories.socketFactory,
			rtcFactory: factories.rtcFactory
		});

		await session.connectStored(STORED_DESKTOP);
		const socket = factories.sockets[0];
		socket.onopen?.();
		socket.onclose?.({ code: 4010 });

		expect(session.state).toBe('failed');
		expect(session.failure).toBe('pairing-failed');
		expect(session.failureHint).toBe(BUSY_ROOM_HINT);
	});

	it('leaves an ordinary close on the generic hint', async () => {
		const factories = createTrackingFactories();
		const session = createSignalSession({
			brokerUrl: 'ws://test-broker',
			socketFactory: factories.socketFactory,
			rtcFactory: factories.rtcFactory
		});

		await session.connectStored(STORED_DESKTOP);
		factories.sockets[0].onclose?.({ code: 1006 });

		expect(session.state).toBe('failed');
		expect(session.failureHint).not.toBe(BUSY_ROOM_HINT);
	});

	it('gives pairing-specific copy for a 4010 on a plain pairing-code dial', async () => {
		const factories = createTrackingFactories();
		const session = createSignalSession({
			brokerUrl: 'ws://test-broker',
			socketFactory: factories.socketFactory,
			rtcFactory: factories.rtcFactory
		});

		await session.connect('SOME-CODE-4010');
		factories.sockets[0].onopen?.();
		factories.sockets[0].onclose?.({ code: 4010 });

		expect(session.state).toBe('failed');
		expect(session.failure).toBe('pairing-failed');
		expect(session.failureHint).toBe(PAIRING_BUSY_HINT);
		expect(session.failureHint).not.toBe(BUSY_ROOM_HINT);
	});

	it('does not let a stale busy flag survive a disconnect into a dial that never reaches #dial\'s clear', async () => {
		const factories = createTrackingFactories();
		const session = createSignalSession({
			brokerUrl: 'ws://test-broker',
			socketFactory: factories.socketFactory,
			rtcFactory: factories.rtcFactory
		});

		await session.connectStored(STORED_DESKTOP);
		factories.sockets[0].onopen?.();
		factories.sockets[0].onclose?.({ code: 4010 });
		expect(session.failureHint).toBe(BUSY_ROOM_HINT);

		session.disconnect();

		const importKeySpy = vi
			.spyOn(crypto.subtle, 'importKey')
			.mockRejectedValueOnce(new Error('boom'));
		try {
			await session.connect('SOME-CODE-9999');
		} finally {
			importKeySpy.mockRestore();
		}

		expect(session.state).toBe('failed');
		expect(session.failure).toBe('broker-unreachable');
		expect(session.failureHint).not.toBe(BUSY_ROOM_HINT);
		expect(session.failureHint).not.toBe(PAIRING_BUSY_HINT);
		expect(session.failureHint).toMatch(/Couldn't reach the pairing server/);
	});
});

describe('reconnect ladder', () => {
	beforeEach(() => {
		vi.useFakeTimers();
	});

	afterEach(() => {
		vi.useRealTimers();
	});

	it('enters reconnecting when a connected stored session drops, and re-dials at 1s/2s/4s', async () => {
		const factories = createTrackingFactories();
		const session = createSignalSession({
			brokerUrl: 'ws://test-broker',
			socketFactory: factories.socketFactory,
			rtcFactory: factories.rtcFactory
		});

		await session.connectStored(STORED_DESKTOP);
		await connectAndOpen(factories, 0);
		expect(session.state).toBe('connected');

		factories.channelsList[0].ctl.onclose?.();
		expect(session.state).toBe('reconnecting');
		expect(session.reconnectAttempt).toBe(1);
		expect(factories.sockets).toHaveLength(1);

		await vi.advanceTimersByTimeAsync(999);
		expect(factories.sockets).toHaveLength(1);
		await vi.advanceTimersByTimeAsync(1);
		await vi.waitFor(() => expect(factories.sockets).toHaveLength(2));
		expect(factories.urls[1]).toContain('kind=meet');
		expect(session.state).toBe('reconnecting');

		await vi.advanceTimersByTimeAsync(10_000);
		expect(session.state).toBe('reconnecting');
		expect(session.reconnectAttempt).toBe(2);
		expect(factories.sockets).toHaveLength(2);

		await vi.advanceTimersByTimeAsync(1_999);
		expect(factories.sockets).toHaveLength(2);
		await vi.advanceTimersByTimeAsync(1);
		await vi.waitFor(() => expect(factories.sockets).toHaveLength(3));

		await vi.advanceTimersByTimeAsync(10_000);
		expect(session.reconnectAttempt).toBe(3);

		await vi.advanceTimersByTimeAsync(3_999);
		expect(factories.sockets).toHaveLength(3);
		await vi.advanceTimersByTimeAsync(1);
		await vi.waitFor(() => expect(factories.sockets).toHaveLength(4));
	});

	it('returns to connected and resets reconnectAttempt when a retry succeeds mid-ladder', async () => {
		const factories = createTrackingFactories();
		const session = createSignalSession({
			brokerUrl: 'ws://test-broker',
			socketFactory: factories.socketFactory,
			rtcFactory: factories.rtcFactory
		});

		await session.connectStored(STORED_DESKTOP);
		await connectAndOpen(factories, 0);

		factories.channelsList[0].ctl.onclose?.();
		expect(session.state).toBe('reconnecting');

		await vi.advanceTimersByTimeAsync(1_000);
		await vi.waitFor(() => expect(factories.sockets).toHaveLength(2));
		await connectAndOpen(factories, 1);

		expect(session.state).toBe('connected');
		expect(session.reconnectAttempt).toBe(0);
	});

	it('cancelReconnect stops the ladder, clears timers and returns to idle', async () => {
		const factories = createTrackingFactories();
		const session = createSignalSession({
			brokerUrl: 'ws://test-broker',
			socketFactory: factories.socketFactory,
			rtcFactory: factories.rtcFactory
		});

		await session.connectStored(STORED_DESKTOP);
		await connectAndOpen(factories, 0);
		factories.channelsList[0].ctl.onclose?.();
		expect(session.state).toBe('reconnecting');

		session.cancelReconnect();
		expect(session.state).toBe('idle');
		expect(session.reconnectAttempt).toBe(0);

		await vi.advanceTimersByTimeAsync(120_000);
		expect(factories.sockets).toHaveLength(1);
		expect(session.state).toBe('idle');
	});

	it('a 4010 close mid-ladder schedules the next attempt instead of going terminal', async () => {
		const factories = createTrackingFactories();
		const session = createSignalSession({
			brokerUrl: 'ws://test-broker',
			socketFactory: factories.socketFactory,
			rtcFactory: factories.rtcFactory
		});

		await session.connectStored(STORED_DESKTOP);
		await connectAndOpen(factories, 0);
		factories.channelsList[0].ctl.onclose?.();

		await vi.advanceTimersByTimeAsync(1_000);
		await vi.waitFor(() => expect(factories.sockets).toHaveLength(2));
		factories.sockets[1].onopen?.();
		factories.sockets[1].onclose?.({ code: 4010 });

		expect(session.state).toBe('reconnecting');
		expect(session.reconnectAttempt).toBe(2);

		await vi.advanceTimersByTimeAsync(2_000);
		await vi.waitFor(() => expect(factories.sockets).toHaveLength(3));
	});

	it('schedules nothing while the page is hidden and picks up when it returns', async () => {
		const doc = stubDocument();
		try {
			const factories = createTrackingFactories();
			const session = createSignalSession({
				brokerUrl: 'ws://test-broker',
				socketFactory: factories.socketFactory,
				rtcFactory: factories.rtcFactory
			});

			await session.connectStored(STORED_DESKTOP);
			await connectAndOpen(factories, 0);

			doc.hidden = true;
			factories.channelsList[0].ctl.onclose?.();
			expect(session.state).toBe('reconnecting');
			expect(session.reconnectAttempt).toBe(1);

			await vi.advanceTimersByTimeAsync(600_000);
			await expectNoFurtherDial(factories.sockets, 1);

			doc.hidden = false;
			doc.visibilityChanged();
			await expectNoFurtherDial(factories.sockets, 1);
			await vi.advanceTimersByTimeAsync(1_000);
			await vi.waitFor(() => expect(factories.sockets).toHaveLength(2));
			expect(session.reconnectAttempt).toBe(1);
		} finally {
			restoreDocument();
		}
	});

	it('widens the cap from 60s to 300s past twenty attempts', async () => {
		const factories = createTrackingFactories();
		const session = createSignalSession({
			brokerUrl: 'ws://test-broker',
			socketFactory: factories.socketFactory,
			rtcFactory: factories.rtcFactory
		});

		await session.connectStored(STORED_DESKTOP);
		await connectAndOpen(factories, 0);
		factories.channelsList[0].ctl.onclose?.();

		for (let attempt = 1; attempt <= 20; attempt++) {
			await vi.advanceTimersByTimeAsync(Math.min(1_000 * 2 ** (attempt - 1), 60_000));
			await vi.waitFor(() => expect(factories.sockets).toHaveLength(attempt + 1));
			await vi.advanceTimersByTimeAsync(10_000);
		}
		expect(session.reconnectAttempt).toBe(21);

		await vi.advanceTimersByTimeAsync(60_000);
		await expectNoFurtherDial(factories.sockets, 21);
		await vi.advanceTimersByTimeAsync(240_000);
		await vi.waitFor(() => expect(factories.sockets).toHaveLength(22));
	});

	it('a plain-code session stays terminal on drop and never ladders', async () => {
		const { session, ctl, socket } = buildConnectedSession();

		await session.connect('TEST-CODE-LADDER1');
		socket.onopen?.();
		await vi.waitFor(() => expect(socket.sent.length).toBeGreaterThan(0));
		ctl.readyState = 'open';
		ctl.onopen?.();
		expect(session.state).toBe('connected');

		ctl.onclose?.();

		expect(session.state).toBe('failed');
		expect(session.failure).toBe('direct-connect-failed');
		expect(session.reconnectAttempt).toBe(0);
	});
});

describe('resumeStored', () => {
	beforeEach(() => {
		vi.useFakeTimers();
	});

	afterEach(() => {
		vi.useRealTimers();
	});

	it('dials at once as attempt one, then climbs the 1s/2s rungs on failure', async () => {
		const factories = createTrackingFactories();
		const session = trackedSession(factories);

		session.resumeStored(STORED_DESKTOP);
		expect(session.state).toBe('reconnecting');
		expect(session.reconnectAttempt).toBe(1);

		await vi.waitFor(() => expect(factories.sockets).toHaveLength(1));
		expect(factories.urls[0]).toContain(`room=${STORED_DESKTOP.meetRoom}`);
		expect(factories.urls[0]).toContain('kind=meet');

		await vi.advanceTimersByTimeAsync(10_000);
		expect(session.state).toBe('reconnecting');
		expect(session.failure).toBeNull();
		expect(session.reconnectAttempt).toBe(1);

		await vi.advanceTimersByTimeAsync(999);
		expect(factories.sockets).toHaveLength(1);
		await vi.advanceTimersByTimeAsync(1);
		await vi.waitFor(() => expect(factories.sockets).toHaveLength(2));

		await vi.advanceTimersByTimeAsync(10_000);
		expect(session.reconnectAttempt).toBe(2);
		await vi.advanceTimersByTimeAsync(1_999);
		expect(factories.sockets).toHaveLength(2);
		await vi.advanceTimersByTimeAsync(1);
		await vi.waitFor(() => expect(factories.sockets).toHaveLength(3));
	});

	it('reaches connected when the immediate dial lands', async () => {
		const factories = createTrackingFactories();
		const session = trackedSession(factories);

		session.resumeStored(STORED_DESKTOP);
		await vi.waitFor(() => expect(factories.sockets).toHaveLength(1));
		await connectAndOpen(factories, 0);

		expect(session.state).toBe('connected');
		expect(session.reconnectAttempt).toBe(0);
	});

	it('cancelReconnect stops a resumed ladder and returns to idle', async () => {
		const factories = createTrackingFactories();
		const session = trackedSession(factories);

		session.resumeStored(STORED_DESKTOP);
		await vi.waitFor(() => expect(factories.sockets).toHaveLength(1));

		session.cancelReconnect();
		expect(session.state).toBe('idle');
		expect(session.reconnectAttempt).toBe(0);

		await vi.advanceTimersByTimeAsync(120_000);
		expect(factories.sockets).toHaveLength(1);
		expect(session.state).toBe('idle');
	});
});

describe('session_end on ctl', () => {
	beforeEach(() => {
		vi.useFakeTimers();
		stubLocalStorage();
	});

	afterEach(() => {
		restoreLocalStorage();
		vi.useRealTimers();
	});

	it('ends a connected stored session without laddering, and keeps the credential', async () => {
		saveStoredDesktop(STORED_DESKTOP);
		const factories = createTrackingFactories();
		const session = trackedSession(factories);

		await session.connectStored(STORED_DESKTOP);
		await connectAndOpen(factories, 0);
		expect(session.state).toBe('connected');

		factories.channelsList[0].ctl.onmessage?.({
			data: JSON.stringify({ type: 'session_end', data: {} })
		});

		expect(session.state).toBe('idle');
		expect(session.failure).toBeNull();
		expect(session.reconnectAttempt).toBe(0);

		await vi.advanceTimersByTimeAsync(120_000);
		await expectNoFurtherDial(factories.sockets, 1);
		expect(session.state).toBe('idle');

		expect(loadStoredDesktop()).toEqual(STORED_DESKTOP);
	});

	it('leaves the ladder alone for an ordinary drop', async () => {
		saveStoredDesktop(STORED_DESKTOP);
		const factories = createTrackingFactories();
		const session = trackedSession(factories);

		await session.connectStored(STORED_DESKTOP);
		await connectAndOpen(factories, 0);
		factories.channelsList[0].ctl.onclose?.();

		expect(session.state).toBe('reconnecting');
		await vi.advanceTimersByTimeAsync(1_000);
		await vi.waitFor(() => expect(factories.sockets).toHaveLength(2));
	});

	it('resuming after a goodbye ladders again rather than staying quiet', async () => {
		const factories = createTrackingFactories();
		const session = trackedSession(factories);

		await session.connectStored(STORED_DESKTOP);
		await connectAndOpen(factories, 0);
		factories.channelsList[0].ctl.onmessage?.({
			data: JSON.stringify({ type: 'session_end', data: {} })
		});
		expect(session.state).toBe('idle');

		session.resumeStored(STORED_DESKTOP);
		expect(session.state).toBe('reconnecting');
		await vi.waitFor(() => expect(factories.sockets).toHaveLength(2));
		await vi.advanceTimersByTimeAsync(10_000);
		expect(session.state).toBe('reconnecting');
	});
});

describe('auto-resume eligibility', () => {
	beforeEach(() => {
		vi.useFakeTimers();
	});

	afterEach(() => {
		vi.useRealTimers();
	});

	it('arms a fresh session so the first map that mounts resumes', () => {
		const session = trackedSession(createTrackingFactories());
		expect(session.canAutoResume).toBe(true);
	});

	it('stays disarmed after a goodbye, so a remount does not redial', async () => {
		const factories = createTrackingFactories();
		const session = trackedSession(factories);

		expect(autoResumeOnMount(session, STORED_DESKTOP)).toBe(true);
		await vi.waitFor(() => expect(factories.sockets).toHaveLength(1));
		await connectAndOpen(factories, 0);
		factories.channelsList[0].ctl.onmessage?.({
			data: JSON.stringify({ type: 'session_end', data: {} })
		});
		expect(session.state).toBe('idle');

		expect(session.canAutoResume).toBe(false);
		expect(autoResumeOnMount(session, STORED_DESKTOP)).toBe(false);
		await vi.advanceTimersByTimeAsync(120_000);
		await expectNoFurtherDial(factories.sockets, 1);
	});

	it('stays disarmed after the user cancels the ladder', async () => {
		const factories = createTrackingFactories();
		const session = trackedSession(factories);

		expect(autoResumeOnMount(session, STORED_DESKTOP)).toBe(true);
		await vi.waitFor(() => expect(factories.sockets).toHaveLength(1));
		session.cancelReconnect();
		expect(session.state).toBe('idle');

		expect(session.canAutoResume).toBe(false);
		expect(autoResumeOnMount(session, STORED_DESKTOP)).toBe(false);
		await vi.advanceTimersByTimeAsync(120_000);
		await expectNoFurtherDial(factories.sockets, 1);
	});

	it('leaves the manual card working after a goodbye, and disarms again after the next', async () => {
		const factories = createTrackingFactories();
		const session = trackedSession(factories);

		await session.connectStored(STORED_DESKTOP);
		await connectAndOpen(factories, 0);
		const goodbye = JSON.stringify({ type: 'session_end', data: {} });
		factories.channelsList[0].ctl.onmessage?.({ data: goodbye });
		expect(session.canAutoResume).toBe(false);

		await session.connectStored(STORED_DESKTOP);
		await vi.waitFor(() => expect(factories.sockets).toHaveLength(2));
		await connectAndOpen(factories, 1);
		expect(session.state).toBe('connected');

		factories.channelsList[1].ctl.onmessage?.({ data: goodbye });
		expect(session.state).toBe('idle');
		expect(session.canAutoResume).toBe(false);
	});
});

const DEFAULT_ICE_SERVERS: RTCIceServer[] = [
	{ urls: 'stun:stun.cloudflare.com:3478' },
	{ urls: 'stun:stun.l.google.com:19302' }
];

describe('per-dial TURN merge', () => {
	it("hands rtcFactory a config whose iceServers are the defaults plus the provider's entries", async () => {
		const factories = createTrackingFactories();
		const turnServer: RTCIceServer = {
			urls: 'turn:relay.example:3478',
			username: 'u',
			credential: 'c'
		};
		const turnProvider = vi.fn().mockResolvedValue([turnServer]);
		const session = createSignalSession({
			brokerUrl: 'ws://test-broker',
			socketFactory: factories.socketFactory,
			rtcFactory: factories.rtcFactory,
			turnProvider
		});

		await session.connect('SOME-CODE-TURN1');
		factories.sockets[0].onopen?.();
		await vi.waitFor(() => expect(factories.configs).toHaveLength(1));

		expect(turnProvider).toHaveBeenCalledTimes(1);
		expect(factories.configs[0].iceServers).toEqual([...DEFAULT_ICE_SERVERS, turnServer]);
	});

	it('still dials with just the defaults when the provider rejects', async () => {
		const factories = createTrackingFactories();
		const turnProvider = vi.fn().mockRejectedValue(new Error('turn fetch failed'));
		const session = createSignalSession({
			brokerUrl: 'ws://test-broker',
			socketFactory: factories.socketFactory,
			rtcFactory: factories.rtcFactory,
			turnProvider
		});

		await session.connect('SOME-CODE-TURN2');
		factories.sockets[0].onopen?.();
		await vi.waitFor(() => expect(factories.configs).toHaveLength(1));

		expect(factories.configs[0].iceServers).toEqual(DEFAULT_ICE_SERVERS);
	});

	it('passes no turnProvider at all the same as one with nothing to add', async () => {
		const factories = createTrackingFactories();
		const session = createSignalSession({
			brokerUrl: 'ws://test-broker',
			socketFactory: factories.socketFactory,
			rtcFactory: factories.rtcFactory
		});

		await session.connect('SOME-CODE-TURN3');
		factories.sockets[0].onopen?.();
		await vi.waitFor(() => expect(factories.configs).toHaveLength(1));

		expect(factories.configs[0].iceServers).toEqual(DEFAULT_ICE_SERVERS);
	});

	it('passes iceTransportPolicy: relay when forceRelay is set', async () => {
		const factories = createTrackingFactories();
		const session = createSignalSession({
			brokerUrl: 'ws://test-broker',
			socketFactory: factories.socketFactory,
			rtcFactory: factories.rtcFactory,
			forceRelay: true
		});

		await session.connect('SOME-CODE-TURN4');
		factories.sockets[0].onopen?.();
		await vi.waitFor(() => expect(factories.configs).toHaveLength(1));

		expect(factories.configs[0].iceTransportPolicy).toBe('relay');
	});

	it('leaves iceTransportPolicy unset when forceRelay is not passed', async () => {
		const factories = createTrackingFactories();
		const session = createSignalSession({
			brokerUrl: 'ws://test-broker',
			socketFactory: factories.socketFactory,
			rtcFactory: factories.rtcFactory
		});

		await session.connect('SOME-CODE-TURN5');
		factories.sockets[0].onopen?.();
		await vi.waitFor(() => expect(factories.configs).toHaveLength(1));

		expect(factories.configs[0].iceTransportPolicy).toBeUndefined();
	});
});

type FakeStatReport = {
	type: string;
	state?: string;
	nominated?: boolean;
	localCandidateId?: string;
	candidateType?: string;
	id: string;
};

function connectedSessionWithStats(stats: FakeStatReport[] | null) {
	const ctl = createFakeDataChannel('ctl');
	const live = createFakeDataChannel('live');
	const socket = createFakeSocket();
	const pc = createFakePeerConnection({ ctl, live }) as PeerConnectionLike & {
		getStats?: () => Promise<FakeStatReport[]>;
	};
	if (stats) pc.getStats = async () => stats;
	const session = createSignalSession({
		brokerUrl: 'ws://test-broker',
		socketFactory: () => socket,
		rtcFactory: () => pc
	});
	return { session, ctl, live, socket, pc };
}

describe('viaRelay', () => {
	it('is null before any connection attempt', () => {
		const { session } = connectedSessionWithStats(null);
		expect(session.viaRelay).toBeNull();
	});

	it('is null once connected when the peer connection has no getStats', async () => {
		const { session, ctl, socket } = connectedSessionWithStats(null);

		await session.connect('SOME-CODE-RELAY1');
		socket.onopen?.();
		await vi.waitFor(() => expect(socket.sent.length).toBeGreaterThan(0));
		ctl.readyState = 'open';
		ctl.onopen?.();

		expect(session.state).toBe('connected');
		expect(session.viaRelay).toBeNull();
	});

	it('is true when the nominated candidate pair resolves to a relay local candidate', async () => {
		const { session, ctl, socket } = connectedSessionWithStats([
			{
				type: 'candidate-pair',
				state: 'succeeded',
				nominated: true,
				localCandidateId: 'local1',
				id: 'pair1'
			},
			{ type: 'local-candidate', candidateType: 'relay', id: 'local1' }
		]);

		await session.connect('SOME-CODE-RELAY2');
		socket.onopen?.();
		await vi.waitFor(() => expect(socket.sent.length).toBeGreaterThan(0));
		ctl.readyState = 'open';
		ctl.onopen?.();

		await vi.waitFor(() => expect(session.viaRelay).toBe(true));
	});

	it('is false when the nominated candidate pair resolves to a host local candidate', async () => {
		const { session, ctl, socket } = connectedSessionWithStats([
			{
				type: 'candidate-pair',
				state: 'succeeded',
				nominated: true,
				localCandidateId: 'local1',
				id: 'pair1'
			},
			{ type: 'local-candidate', candidateType: 'host', id: 'local1' }
		]);

		await session.connect('SOME-CODE-RELAY3');
		socket.onopen?.();
		await vi.waitFor(() => expect(socket.sent.length).toBeGreaterThan(0));
		ctl.readyState = 'open';
		ctl.onopen?.();

		await vi.waitFor(() => expect(session.viaRelay).toBe(false));
	});

	it('prefers the nominated pair over an earlier non-nominated succeeded pair', async () => {
		const { session, ctl, socket } = connectedSessionWithStats([
			{
				type: 'candidate-pair',
				state: 'succeeded',
				nominated: false,
				localCandidateId: 'local-relay',
				id: 'pair0'
			},
			{
				type: 'candidate-pair',
				state: 'succeeded',
				nominated: true,
				localCandidateId: 'local-host',
				id: 'pair1'
			},
			{ type: 'local-candidate', candidateType: 'relay', id: 'local-relay' },
			{ type: 'local-candidate', candidateType: 'host', id: 'local-host' }
		]);

		await session.connect('SOME-CODE-RELAY6');
		socket.onopen?.();
		await vi.waitFor(() => expect(socket.sent.length).toBeGreaterThan(0));
		ctl.readyState = 'open';
		ctl.onopen?.();

		await vi.waitFor(() => expect(session.viaRelay).toBe(false));
	});

	it('falls back to a succeeded pair when no pair reports nominated at all', async () => {
		const { session, ctl, socket } = connectedSessionWithStats([
			{ type: 'candidate-pair', state: 'succeeded', localCandidateId: 'local1', id: 'pair1' },
			{ type: 'local-candidate', candidateType: 'relay', id: 'local1' }
		]);

		await session.connect('SOME-CODE-RELAY7');
		socket.onopen?.();
		await vi.waitFor(() => expect(socket.sent.length).toBeGreaterThan(0));
		ctl.readyState = 'open';
		ctl.onopen?.();

		await vi.waitFor(() => expect(session.viaRelay).toBe(true));
	});

	it('resets to null on the next dial rather than keeping a stale relay result', async () => {
		const { session, ctl, socket } = connectedSessionWithStats([
			{
				type: 'candidate-pair',
				state: 'succeeded',
				nominated: true,
				localCandidateId: 'local1',
				id: 'pair1'
			},
			{ type: 'local-candidate', candidateType: 'relay', id: 'local1' }
		]);

		await session.connect('SOME-CODE-RELAY4');
		socket.onopen?.();
		await vi.waitFor(() => expect(socket.sent.length).toBeGreaterThan(0));
		ctl.readyState = 'open';
		ctl.onopen?.();
		await vi.waitFor(() => expect(session.viaRelay).toBe(true));

		const { session: other, ctl: otherCtl, socket: otherSocket } = connectedSessionWithStats(null);
		await other.connect('SOME-CODE-RELAY5');
		otherSocket.onopen?.();
		await vi.waitFor(() => expect(otherSocket.sent.length).toBeGreaterThan(0));
		otherCtl.readyState = 'open';
		otherCtl.onopen?.();

		expect(other.viaRelay).toBeNull();
	});
});
