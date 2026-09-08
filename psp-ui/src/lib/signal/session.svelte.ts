import { deriveDeviceSealKey, deriveKeys, normalizeCode, open, seal } from './crypto';
import type { StoredDesktop } from './credentialStore';

export type SignalConnState = 'idle' | 'connecting' | 'connected' | 'reconnecting' | 'failed';

export type SignalFailure = 'broker-unreachable' | 'pairing-failed' | 'direct-connect-failed' | null;

export interface LiveActorJson {
	id: string;
	kind: string;
	x: number;
	y: number;
	z: number;
	yaw?: number;
	name?: string;
	level?: number;
	hp?: number;
	maxHp?: number;
	guild?: string;
	owner?: string;
	species?: string;
	active?: boolean;
}

export interface LiveFrameJson {
	seq: number;
	source: string;
	capturedAtMs: number;
	observedAtMs?: number;
	fps?: number;
	ingameTime?: string;
	ingameDays?: number;
	actors: LiveActorJson[];
}

export interface SignalSession {
	readonly state: SignalConnState;
	readonly connected: boolean;
	readonly failure: SignalFailure;
	readonly failureHint: string | null;
	readonly lastFrame: LiveFrameJson | null;
	readonly lastFrameAt: number | null;
	readonly viaRelay: boolean | null;
	/**
	 * Plain callback per applied frame. The store bridge uses this instead of
	 * an effect: a `$effect.root` created during a component's initialization
	 * is owned by that component and dies with it, which silently froze the
	 * map after navigating away from the pairing page. Listeners survive
	 * disconnect/reconnect; the returned function unsubscribes.
	 */
	onFrame(fn: (frame: LiveFrameJson) => void): () => void;
	connect(code: string): Promise<void>;
	connectStored(desktop: StoredDesktop): Promise<void>;
	resumeStored(desktop: StoredDesktop): void;
	readonly canAutoResume: boolean;
	disconnect(): void;
	cancelReconnect(): void;
	readonly reconnectAttempt: number;
	send(type: string, data?: unknown): void;
	sendRaw(text: string): void;
	sendBytes(type: string, bytes: Uint8Array): void;
	sendAndWait(type: string, data?: unknown, timeoutMs?: number): Promise<unknown>;
	readonly message: {
		subscribe(fn: (frame: { type: string; data: unknown }) => void): () => void;
	};
}

export interface DataChannelLike {
	readonly label: string;
	readyState: 'connecting' | 'open' | 'closing' | 'closed';
	send(data: string): void;
	close(): void;
	onopen: (() => void) | null;
	onclose: (() => void) | null;
	onmessage: ((event: { data: string }) => void) | null;
}

export interface IceStatReport {
	type: string;
	state?: string;
	nominated?: boolean;
	localCandidateId?: string;
	candidateType?: string;
	id: string;
}

export interface PeerConnectionLike {
	iceConnectionState: string;
	localDescription: { type: string; sdp: string } | null;
	onicecandidate: ((event: { candidate: { toJSON(): unknown } | null }) => void) | null;
	oniceconnectionstatechange: (() => void) | null;
	createDataChannel(label: string, options?: RTCDataChannelInit): DataChannelLike;
	createOffer(): Promise<{ type: string; sdp: string }>;
	setLocalDescription(desc: { type: string; sdp: string }): Promise<void>;
	setRemoteDescription(desc: { type: string; sdp: string }): Promise<void>;
	addIceCandidate(candidate: unknown): Promise<void>;
	close(): void;
	getStats?(): Promise<Iterable<IceStatReport>>;
}

export interface BrokerSocketLike {
	send(data: string): void;
	close(): void;
	onopen: (() => void) | null;
	onclose: ((event?: { code?: number }) => void) | null;
	onerror: (() => void) | null;
	onmessage: ((event: { data: string }) => void) | null;
}

export interface SignalSessionOptions {
	iceServers?: RTCIceServer[];
	brokerUrl?: string;
	socketFactory?: (url: string) => BrokerSocketLike;
	rtcFactory?: (config: RTCConfiguration) => PeerConnectionLike;
	now?: () => number;
	turnProvider?: () => Promise<RTCIceServer[]>;
	forceRelay?: boolean;
}

const DEFAULT_ICE_SERVERS: RTCIceServer[] = [
	{ urls: 'stun:stun.cloudflare.com:3478' },
	{ urls: 'stun:stun.l.google.com:19302' }
];

const BROKER_OPEN_TIMEOUT_MS = 10_000;
const ANSWER_TIMEOUT_MS = 20_000;
const ICE_CONNECT_TIMEOUT_MS = 30_000;
const DATACHANNEL_OPEN_TIMEOUT_MS = 10_000;
const ICE_DISCONNECT_GRACE_MS = 5_000;
const SEND_AND_WAIT_TIMEOUT_MS = 10_000;
const RECONNECT_BASE_DELAY_MS = 1_000;
const RECONNECT_MAX_DELAY_MS = 60_000;
const RECONNECT_LONG_LADDER_AFTER = 20;
const RECONNECT_LONG_MAX_DELAY_MS = 300_000;

const ROLE_TAKEN_CLOSE = 4010;

const SESSION_END = 'session_end';

const GUEST_AAD = new TextEncoder().encode('guest');
const HOST_AAD = new TextEncoder().encode('host');

const FAILURE_HINTS: Record<Exclude<SignalFailure, null>, string> = {
	'broker-unreachable':
		"Couldn't reach the pairing server. Check your connection and try again.",
	'pairing-failed':
		"That code didn't reach a waiting app. Check the code, or start pairing again on your PC — codes expire after 5 minutes.",
	'direct-connect-failed':
		"Found your PC, but a direct connection couldn't be established. Restrictive networks (mobile hotspots, CGNAT) need a relay — you can add your own TURN server in settings."
};

const STORED_PAIRING_FAILED_HINT =
	"Your PC isn't reachable — make sure Palworld Save Pal is running with remote access armed.";

const BUSY_ROOM_HINT =
	'Another device is currently connected to this PC. Disconnect it first, or wait for it to drop.';

const PAIRING_BUSY_HINT =
	'Someone else is already pairing with this PC. Finish or stop that pairing first.';

const PAIRING_FRAGMENT_RE = /^#v1\.(.+)$/;

export function parsePairingFragment(hash: string): string | null {
	const match = PAIRING_FRAGMENT_RE.exec(hash);
	return match ? match[1] : null;
}

function bytesToBase64(bytes: Uint8Array): string {
	let binary = '';
	for (const byte of bytes) binary += String.fromCharCode(byte);
	return btoa(binary);
}

function base64ToBytes(b64: string): Uint8Array {
	const binary = atob(b64);
	const bytes = new Uint8Array(binary.length);
	for (let i = 0; i < binary.length; i++) bytes[i] = binary.charCodeAt(i);
	return bytes;
}

function documentHidden(): boolean {
	return typeof document !== 'undefined' && document.hidden;
}

function resolveDefaultBrokerBase(): string {
	const envUrl = import.meta.env.VITE_SIGNAL_BROKER_URL as string | undefined;
	if (envUrl) return envUrl;
	const protocol = window.location.protocol === 'https:' ? 'wss:' : 'ws:';
	return `${protocol}//${window.location.host}`;
}

type SignalingEnvelope = { t: 'offer' | 'answer'; sdp: string } | { t: 'ice'; c: string };

interface LiveBuffer {
	seq: number;
	parts: number;
	chunks: Map<number, string>;
}

class SignalSessionImpl implements SignalSession {
	#state = $state<SignalConnState>('idle');
	#failure = $state<SignalFailure>(null);
	#lastFrame = $state.raw<LiveFrameJson | null>(null);
	#lastFrameAt = $state<number | null>(null);
	#reconnectAttempt = $state<number>(0);
	#viaRelay = $state<boolean | null>(null);

	#iceServers: RTCIceServer[];
	#brokerUrlOverride?: string;
	#socketFactory: (url: string) => BrokerSocketLike;
	#rtcFactory: (config: RTCConfiguration) => PeerConnectionLike;
	#turnProvider?: () => Promise<RTCIceServer[]>;
	#forceRelay: boolean;
	#now: () => number;

	#sealKey: CryptoKey | null = null;
	#deviceId: string | null = null;
	#storedDesktop: StoredDesktop | null = null;
	#socket: BrokerSocketLike | null = null;
	#pc: PeerConnectionLike | null = null;
	#ctl: DataChannelLike | null = null;
	#live: DataChannelLike | null = null;

	#timers = new Map<string, ReturnType<typeof setTimeout>>();
	#pending = new Map<
		string,
		{ token: number; resolve: (value: unknown) => void; reject: (err: unknown) => void }
	>();
	#pendingToken = 0;
	#listeners = new Set<(frame: { type: string; data: unknown }) => void>();
	#frameListeners = new Set<(frame: LiveFrameJson) => void>();
	#liveBuffer: LiveBuffer | null = null;
	#lastAppliedSeq: number | null = null;
	#connectSeq = 0;
	#remoteDescriptionApplied = false;
	#pendingIceCandidates: unknown[] = [];
	#busyRoom = false;
	#visibilityListener: (() => void) | null = null;
	#resumeDialPending = false;
	#remoteEnded = false;
	#autoResumeArmed = true;

	constructor(options: SignalSessionOptions = {}) {
		this.#iceServers = [...DEFAULT_ICE_SERVERS, ...(options.iceServers ?? [])];
		this.#brokerUrlOverride = options.brokerUrl;
		this.#socketFactory =
			options.socketFactory ?? ((url) => new WebSocket(url) as unknown as BrokerSocketLike);
		this.#rtcFactory =
			options.rtcFactory ??
			((config) => new RTCPeerConnection(config) as unknown as PeerConnectionLike);
		this.#turnProvider = options.turnProvider;
		this.#forceRelay = options.forceRelay ?? false;
		this.#now = options.now ?? Date.now;
	}

	get state() {
		return this.#state;
	}

	get connected() {
		return this.#state === 'connected';
	}

	get canAutoResume() {
		return this.#state === 'idle' && this.#autoResumeArmed;
	}

	get failure() {
		return this.#failure;
	}

	get failureHint() {
		if (!this.#failure) return null;
		if (this.#busyRoom) return this.#storedDesktop ? BUSY_ROOM_HINT : PAIRING_BUSY_HINT;
		if (this.#failure === 'pairing-failed' && this.#storedDesktop)
			return STORED_PAIRING_FAILED_HINT;
		return FAILURE_HINTS[this.#failure];
	}

	get lastFrame() {
		return this.#lastFrame;
	}

	get lastFrameAt() {
		return this.#lastFrameAt;
	}

	get reconnectAttempt() {
		return this.#reconnectAttempt;
	}

	get viaRelay() {
		return this.#viaRelay;
	}

	get message() {
		return {
			subscribe: (fn: (frame: { type: string; data: unknown }) => void) => {
				this.#listeners.add(fn);
				return () => this.#listeners.delete(fn);
			}
		};
	}

	async connect(code: string): Promise<void> {
		this.disconnect();
		this.#remoteEnded = false;
		this.#autoResumeArmed = true;
		const mySeq = ++this.#connectSeq;
		this.#state = 'connecting';
		this.#failure = null;

		let roomId: string;
		let sealKey: CryptoKey;
		try {
			const derived = await deriveKeys(normalizeCode(code));
			roomId = derived.roomId;
			sealKey = derived.sealKey;
		} catch {
			if (mySeq !== this.#connectSeq) return;
			this.#fail('broker-unreachable');
			return;
		}
		if (mySeq !== this.#connectSeq) return;
		this.#sealKey = sealKey;

		const base = this.#brokerUrlOverride ?? resolveDefaultBrokerBase();
		const url = `${base}/signal/ws?room=${roomId}&role=guest`;
		this.#dial(mySeq, url);
	}

	async connectStored(desktop: StoredDesktop): Promise<void> {
		this.disconnect();
		this.#remoteEnded = false;
		this.#autoResumeArmed = true;
		this.#storedDesktop = desktop;
		await this.#dialStored(desktop, 'connecting');
	}

	resumeStored(desktop: StoredDesktop): void {
		this.disconnect();
		this.#remoteEnded = false;
		this.#autoResumeArmed = true;
		this.#storedDesktop = desktop;
		this.#state = 'reconnecting';
		this.#reconnectAttempt = 1;
		this.#resumeDialPending = true;
		void this.#dialStored(desktop, 'reconnecting');
	}

	async #dialStored(desktop: StoredDesktop, mode: 'connecting' | 'reconnecting'): Promise<void> {
		const mySeq = ++this.#connectSeq;
		this.#state = mode;
		this.#failure = null;

		let sealKey: CryptoKey;
		try {
			sealKey = await deriveDeviceSealKey(desktop.deviceSecret);
		} catch {
			if (mySeq !== this.#connectSeq) return;
			this.#fail('broker-unreachable');
			return;
		}
		if (mySeq !== this.#connectSeq) return;
		this.#sealKey = sealKey;
		this.#deviceId = desktop.deviceId;

		const base = this.#brokerUrlOverride ?? resolveDefaultBrokerBase();
		const url = `${base}/signal/ws?room=${desktop.meetRoom}&role=guest&kind=meet`;
		this.#dial(mySeq, url);
	}

	#dial(mySeq: number, url: string): void {
		this.#busyRoom = false;
		this.#viaRelay = null;
		let socket: BrokerSocketLike;
		try {
			socket = this.#socketFactory(url);
		} catch {
			this.#fail('broker-unreachable');
			return;
		}
		this.#socket = socket;

		let opened = false;
		this.#setTimer('brokerOpen', BROKER_OPEN_TIMEOUT_MS, () => {
			if (mySeq !== this.#connectSeq) return;
			this.#fail('broker-unreachable');
		});

		socket.onopen = () => {
			if (mySeq !== this.#connectSeq) return;
			opened = true;
			this.#clearTimer('brokerOpen');
			this.#setTimer('answer', ANSWER_TIMEOUT_MS, () => {
				if (mySeq !== this.#connectSeq) return;
				this.#fail('pairing-failed');
			});
			void this.#startHandshake(mySeq);
		};

		socket.onerror = () => {
		};

		socket.onclose = (event) => {
			if (mySeq !== this.#connectSeq) return;
			if (event?.code === ROLE_TAKEN_CLOSE) {
				this.#busyRoom = true;
				this.#fail('pairing-failed');
				return;
			}
			if (!opened) this.#fail('broker-unreachable');
		};

		socket.onmessage = (event) => {
			if (mySeq !== this.#connectSeq) return;
			void this.#handleSignalingMessage(event.data, mySeq);
		};
	}

	disconnect(): void {
		this.#teardown();
		this.#state = 'idle';
		this.#failure = null;
		this.#lastFrame = null;
		this.#lastFrameAt = null;
		this.#reconnectAttempt = 0;
		this.#resumeDialPending = false;
		this.#autoResumeArmed = false;
		this.#storedDesktop = null;
		this.#busyRoom = false;
	}

	cancelReconnect(): void {
		this.disconnect();
	}

	send(type: string, data?: unknown): void {
		if (!this.#ctl || this.#ctl.readyState !== 'open') {
			throw new Error('SignalSession: cannot send before the ctl channel is open');
		}
		this.#ctl.send(JSON.stringify({ type, data }));
	}

	sendRaw(text: string): void {
		if (!this.#ctl || this.#ctl.readyState !== 'open') {
			throw new Error('SignalSession: cannot send before the ctl channel is open');
		}
		this.#ctl.send(text);
	}

	sendBytes(_type: string, _bytes: Uint8Array): void {
		throw new Error('SignalSession.sendBytes is not supported yet');
	}

	sendAndWait(type: string, data?: unknown, timeoutMs = SEND_AND_WAIT_TIMEOUT_MS): Promise<unknown> {
		const mySeq = this.#connectSeq;
		const token = ++this.#pendingToken;
		const timerName = `wait:${type}:${token}`;
		return new Promise((resolve, reject) => {
			this.#setTimer(timerName, timeoutMs, () => {
				this.#clearTimer(timerName);
				if (mySeq !== this.#connectSeq) return;
				if (this.#pending.get(type)?.token === token) this.#pending.delete(type);
				reject(new Error('timeout: ' + type));
			});
			this.#pending.set(type, {
				token,
				resolve: (value) => {
					this.#clearTimer(timerName);
					resolve(value);
				},
				reject: (err) => {
					this.#clearTimer(timerName);
					reject(err);
				}
			});
			try {
				this.send(type, data);
			} catch (err) {
				this.#clearTimer(timerName);
				if (this.#pending.get(type)?.token === token) this.#pending.delete(type);
				reject(err);
			}
		});
	}

	async #startHandshake(mySeq: number): Promise<void> {
		const turn = (await this.#turnProvider?.().catch(() => [])) ?? [];
		if (mySeq !== this.#connectSeq) return;

		let pc: PeerConnectionLike;
		try {
			pc = this.#rtcFactory({
				iceServers: [...this.#iceServers, ...turn],
				...(this.#forceRelay ? { iceTransportPolicy: 'relay' as const } : {})
			});
		} catch {
			if (mySeq !== this.#connectSeq) return;
			this.#fail('broker-unreachable');
			return;
		}
		if (mySeq !== this.#connectSeq) {
			pc.close();
			return;
		}
		this.#pc = pc;

		const ctl = pc.createDataChannel('ctl', { ordered: true });
		const live = pc.createDataChannel('live', { ordered: false, maxRetransmits: 0 });
		this.#ctl = ctl;
		this.#live = live;
		this.#wireCtlChannel(ctl, mySeq);
		this.#wireLiveChannel(live, mySeq);

		pc.onicecandidate = (event) => {
			if (mySeq !== this.#connectSeq || !event.candidate) return;
			void this.#sendSealed({ t: 'ice', c: JSON.stringify(event.candidate.toJSON()) });
		};

		pc.oniceconnectionstatechange = () => {
			if (mySeq !== this.#connectSeq) return;
			const iceState = pc.iceConnectionState;
			if (iceState === 'connected' || iceState === 'completed') {
				this.#clearTimer('ice');
				this.#clearTimer('iceGrace');
				if (ctl.readyState !== 'open') {
					this.#setTimer('dc', DATACHANNEL_OPEN_TIMEOUT_MS, () => {
						if (mySeq !== this.#connectSeq) return;
						this.#fail('direct-connect-failed');
					});
				}
			} else if (iceState === 'disconnected') {
				this.#setTimer('iceGrace', ICE_DISCONNECT_GRACE_MS, () => {
					if (mySeq !== this.#connectSeq) return;
					this.#connectionLost();
				});
			} else if (iceState === 'failed' || iceState === 'closed') {
				this.#connectionLost();
			}
		};

		let offer: { type: string; sdp: string };
		try {
			offer = await pc.createOffer();
			await pc.setLocalDescription(offer);
		} catch {
			if (mySeq !== this.#connectSeq) return;
			this.#fail('pairing-failed');
			return;
		}
		if (mySeq !== this.#connectSeq) return;

		await this.#sendSealed({ t: 'offer', sdp: offer.sdp });
	}

	#wireCtlChannel(ctl: DataChannelLike, mySeq: number) {
		ctl.onopen = () => {
			if (mySeq !== this.#connectSeq) return;
			this.#clearTimer('dc');
			this.#state = 'connected';
			this.#failure = null;
			this.#reconnectAttempt = 0;
			this.#resumeDialPending = false;
			void this.#resolveViaRelay(mySeq);
		};
		ctl.onclose = () => {
			if (mySeq !== this.#connectSeq) return;
			this.#connectionLost();
		};
		ctl.onmessage = (event) => {
			if (mySeq !== this.#connectSeq) return;
			this.#handleCtlMessage(event.data);
		};
	}

	async #resolveViaRelay(mySeq: number): Promise<void> {
		const pc = this.#pc;
		if (!pc?.getStats) {
			this.#viaRelay = null;
			return;
		}
		try {
			const stats = [...(await pc.getStats())];
			if (mySeq !== this.#connectSeq) return;
			const pair =
				stats.find((stat) => stat.type === 'candidate-pair' && stat.nominated === true) ??
				stats.find((stat) => stat.type === 'candidate-pair' && stat.state === 'succeeded');
			const local = pair?.localCandidateId
				? stats.find((stat) => stat.type === 'local-candidate' && stat.id === pair.localCandidateId)
				: undefined;
			this.#viaRelay = local ? local.candidateType === 'relay' : null;
		} catch {
			this.#viaRelay = null;
		}
	}

	#wireLiveChannel(live: DataChannelLike, mySeq: number) {
		live.onclose = () => {
			if (mySeq !== this.#connectSeq) return;
			this.#connectionLost();
		};
		live.onmessage = (event) => {
			if (mySeq !== this.#connectSeq) return;
			this.#handleLiveMessage(event.data);
		};
	}

	#handleCtlMessage(raw: string) {
		let msg: { type?: unknown; data?: unknown };
		try {
			msg = JSON.parse(raw);
		} catch {
			return;
		}
		const type = typeof msg.type === 'string' ? msg.type : null;
		if (!type) return;

		if (type === SESSION_END) {
			this.#remoteEnded = true;
			this.#connectionLost();
			return;
		}

		const pending = this.#pending.get(type);
		if (pending) {
			this.#pending.delete(type);
			pending.resolve(msg.data);
			return;
		}
		for (const listener of this.#listeners) listener({ type, data: msg.data });
	}

	#handleLiveMessage(raw: string) {
		let msg: Record<string, unknown>;
		try {
			msg = JSON.parse(raw);
		} catch {
			return;
		}

		const seq = typeof msg.seq === 'number' ? msg.seq : null;
		if (seq === null) return;
		if (this.#lastAppliedSeq !== null && seq <= this.#lastAppliedSeq) return;

		const part = typeof msg.part === 'number' ? msg.part : null;
		const parts = typeof msg.parts === 'number' ? msg.parts : null;

		if (part === null || parts === null) {
			this.#applyLiveFrame(seq, msg as unknown as LiveFrameJson);
			return;
		}

		if (this.#liveBuffer) {
			if (seq < this.#liveBuffer.seq) return;
			if (seq > this.#liveBuffer.seq) this.#liveBuffer = null;
		}
		if (!this.#liveBuffer) this.#liveBuffer = { seq, parts, chunks: new Map() };

		const chunk = typeof msg.data === 'string' ? msg.data : '';
		this.#liveBuffer.chunks.set(part, chunk);
		if (this.#liveBuffer.chunks.size < this.#liveBuffer.parts) return;

		const ordered = [...this.#liveBuffer.chunks.keys()].sort((a, b) => a - b);
		const text = ordered.map((key) => this.#liveBuffer!.chunks.get(key)).join('');
		this.#liveBuffer = null;

		try {
			this.#applyLiveFrame(seq, JSON.parse(text) as LiveFrameJson);
		} catch {
		}
	}

	#applyLiveFrame(seq: number, frame: LiveFrameJson) {
		if (this.#lastAppliedSeq !== null && seq <= this.#lastAppliedSeq) return;
		this.#lastAppliedSeq = seq;
		this.#lastFrame = frame;
		this.#lastFrameAt = this.#now();
		for (const listener of this.#frameListeners) {
			try {
				listener(frame);
			} catch {
			}
		}
	}

	onFrame(fn: (frame: LiveFrameJson) => void): () => void {
		this.#frameListeners.add(fn);
		return () => this.#frameListeners.delete(fn);
	}

	async #sendSealed(msg: SignalingEnvelope): Promise<void> {
		const sealKey = this.#sealKey;
		const socket = this.#socket;
		if (!sealKey || !socket) return;
		const plaintext = new TextEncoder().encode(JSON.stringify(msg));
		const sealed = await seal(sealKey, plaintext, GUEST_AAD);
		const payload = this.#deviceId
			? JSON.stringify({ v: 1, device: this.#deviceId, blob: bytesToBase64(sealed) })
			: bytesToBase64(sealed);
		try {
			socket.send(payload);
		} catch {
		}
	}

	async #handleSignalingMessage(raw: string, mySeq: number): Promise<void> {
		if (!this.#sealKey) return;

		let msg: { t?: string; sdp?: string; c?: string };
		try {
			const sealed = base64ToBytes(raw);
			const plaintext = await open(this.#sealKey, sealed, HOST_AAD);
			msg = JSON.parse(new TextDecoder().decode(plaintext));
		} catch {
			return;
		}
		if (mySeq !== this.#connectSeq) return;

		if (msg.t === 'answer' && typeof msg.sdp === 'string' && this.#pc) {
			this.#clearTimer('answer');
			try {
				await this.#pc.setRemoteDescription({ type: 'answer', sdp: msg.sdp });
			} catch {
				if (mySeq !== this.#connectSeq) return;
				this.#fail('pairing-failed');
				return;
			}
			if (mySeq !== this.#connectSeq) return;
			this.#remoteDescriptionApplied = true;
			await this.#flushPendingIceCandidates();
			if (mySeq !== this.#connectSeq) return;
			this.#setTimer('ice', ICE_CONNECT_TIMEOUT_MS, () => {
				if (mySeq !== this.#connectSeq) return;
				this.#fail('direct-connect-failed');
			});
		} else if (msg.t === 'ice' && typeof msg.c === 'string' && this.#pc) {
			let candidate: unknown;
			try {
				candidate = JSON.parse(msg.c);
			} catch {
				return;
			}
			if (!this.#remoteDescriptionApplied) {
				this.#pendingIceCandidates.push(candidate);
				return;
			}
			try {
				await this.#pc.addIceCandidate(candidate);
			} catch {
			}
		}
	}

	async #flushPendingIceCandidates(): Promise<void> {
		const pc = this.#pc;
		if (!pc) return;
		const candidates = this.#pendingIceCandidates;
		this.#pendingIceCandidates = [];
		for (const candidate of candidates) {
			try {
				await pc.addIceCandidate(candidate);
			} catch {
			}
		}
	}

	#fail(failure: Exclude<SignalFailure, null>) {
		if (this.#state === 'reconnecting' && this.#storedDesktop) {
			this.#teardown();
			this.#scheduleReconnect();
			return;
		}
		this.#teardown();
		this.#state = 'failed';
		this.#failure = failure;
	}

	#connectionLost() {
		if (this.#remoteEnded) {
			this.disconnect();
			return;
		}
		if (this.#state === 'connected' && this.#storedDesktop) {
			this.#teardown();
			this.#state = 'reconnecting';
			this.#failure = null;
			this.#reconnectAttempt = 0;
			this.#scheduleReconnect();
			return;
		}
		this.#fail('direct-connect-failed');
	}

	#scheduleReconnect() {
		if (this.#resumeDialPending) this.#resumeDialPending = false;
		else this.#reconnectAttempt++;
		this.#armReconnect();
	}

	#armReconnect() {
		if (documentHidden()) {
			this.#waitForVisible();
			return;
		}
		const mySeq = this.#connectSeq;
		const maxDelayMs =
			this.#reconnectAttempt > RECONNECT_LONG_LADDER_AFTER
				? RECONNECT_LONG_MAX_DELAY_MS
				: RECONNECT_MAX_DELAY_MS;
		const delayMs = Math.min(
			RECONNECT_BASE_DELAY_MS * 2 ** (this.#reconnectAttempt - 1),
			maxDelayMs
		);
		this.#setTimer('reconnect', delayMs, () => {
			if (mySeq !== this.#connectSeq) return;
			const desktop = this.#storedDesktop;
			if (!desktop) return;
			void this.#dialStored(desktop, 'reconnecting');
		});
	}

	#waitForVisible() {
		if (typeof document === 'undefined' || this.#visibilityListener) return;
		const listener = () => {
			if (documentHidden()) return;
			this.#stopWaitingForVisible();
			if (this.#state !== 'reconnecting' || !this.#storedDesktop) return;
			this.#armReconnect();
		};
		this.#visibilityListener = listener;
		document.addEventListener('visibilitychange', listener);
	}

	#stopWaitingForVisible() {
		const listener = this.#visibilityListener;
		if (!listener) return;
		this.#visibilityListener = null;
		if (typeof document !== 'undefined') {
			document.removeEventListener('visibilitychange', listener);
		}
	}

	#teardown() {
		this.#connectSeq++;
		this.#clearAllTimers();
		this.#stopWaitingForVisible();
		for (const pending of this.#pending.values()) {
			pending.reject(new Error('SignalSession disconnected'));
		}
		this.#pending.clear();
		this.#liveBuffer = null;
		this.#lastAppliedSeq = null;
		this.#remoteDescriptionApplied = false;
		this.#pendingIceCandidates = [];

		try {
			this.#ctl?.close();
		} catch {
		}
		try {
			this.#live?.close();
		} catch {
		}
		try {
			this.#pc?.close();
		} catch {
		}
		try {
			this.#socket?.close();
		} catch {
		}

		this.#ctl = null;
		this.#live = null;
		this.#pc = null;
		this.#socket = null;
		this.#sealKey = null;
		this.#deviceId = null;
	}

	#setTimer(name: string, ms: number, fn: () => void) {
		this.#clearTimer(name);
		this.#timers.set(name, setTimeout(fn, ms));
	}

	#clearTimer(name: string) {
		const timer = this.#timers.get(name);
		if (timer !== undefined) {
			clearTimeout(timer);
			this.#timers.delete(name);
		}
	}

	#clearAllTimers() {
		for (const timer of this.#timers.values()) clearTimeout(timer);
		this.#timers.clear();
	}
}

export function createSignalSession(options?: SignalSessionOptions): SignalSession {
	return new SignalSessionImpl(options);
}
