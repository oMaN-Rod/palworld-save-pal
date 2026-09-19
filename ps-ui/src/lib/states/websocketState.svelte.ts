import { PUBLIC_WS_URL } from '$env/static/public';
import { getDispatcher } from '$lib/ws/dispatcher';
import type { Transport, WSHandlerContext } from '$lib/ws/types';
import { type Message } from '$types';

const RECONNECT_DELAY = 5000;

class SocketState implements Transport {
	readonly kind = 'ws' as const;
	#clientId = Date.now();
	#websocket!: WebSocket;
	// $state.raw: handler-routed frames are dispatched and forgotten — nothing
	// reads `ws.message` deeply, so a deep proxy only adds per-payload cost.
	#message = $state.raw<Message | null>(null);
	#connected = $state(false);
	#dispatcher = getDispatcher();
	#messageQueue = new Map<string, { resolve: (value: any) => void; reject: (err: unknown) => void }>();

	connect(context: WSHandlerContext) {
		const protocol = window.location.protocol === 'https:' ? 'wss://' : 'ws://';
		// Server/webapp builds bake an empty PUBLIC_WS_URL: the page's own
		// origin IS the server, so the websocket follows it (localhost, a LAN
		// IP, or a tailscale Funnel domain) instead of a build-time address.
		const wsBase = PUBLIC_WS_URL || `${window.location.host}/ws`;
		const wsUrl = `${protocol}${wsBase}/${this.#clientId}`;
		this.#websocket = new WebSocket(wsUrl);

		this.#websocket.onopen = () => {
			this.#connected = true;
			if (import.meta.env.DEV) console.log('Connected to backend!');
		};

		this.#websocket.onmessage = async (event) => {
			const data = JSON.parse(event.data);
			if (!data) return;

			// Resolve queued sendAndWait calls with the raw parsed data: routing it
			// through the #message $state proxy makes every consumer read through a
			// deeply reactive proxy (thousands of tracked reads for large payloads).
			if (data.type && this.#messageQueue.has(data.type)) {
				const waiter = this.#messageQueue.get(data.type);
				if (waiter) {
					waiter.resolve(data);
					this.#messageQueue.delete(data.type);
					return;
				}
			}

			this.#message = data;

			// Dev-only and type-only: logging full payloads retains them in DevTools
			// (a leak sized to the save) and serializing MB-scale frames during a
			// load costs tens of milliseconds.
			if (import.meta.env.DEV) console.log('Received message:', data.type);

			await this.#dispatcher.dispatch(data, context);
		};

		this.#websocket.onclose = () => {
			this.#connected = false;
			this.#rejectPending(new Error('WebSocket connection closed'));
			// A dropped socket may be the network policy demanding a PIN
			// (the upgrade is refused with 401 while locked). Probe once per
			// reconnect and route to the server-rendered unlock page — it
			// sets the session cookie and bounces back here. The timeout
			// keeps a hung backend from dangling the probe forever.
			fetch('/api/network/config', { signal: AbortSignal.timeout(5_000) })
				.then((resp) => {
					if (resp.status === 401) window.location.replace('/network-unlock');
				})
				.catch(() => {});
			setTimeout(() => this.connect(context), RECONNECT_DELAY);
		};
	}

	#rejectPending(reason: unknown) {
		for (const waiter of this.#messageQueue.values()) waiter.reject(reason);
		this.#messageQueue.clear();
	}

	isConnected(): boolean {
		return this.#websocket.readyState === this.#websocket.OPEN;
	}

	async send(messageData: string) {
		while (this.#websocket.readyState !== this.#websocket.OPEN) {
			await new Promise((resolve) => setTimeout(resolve, 250));
		}
		// Dev-only and type-only — see the note in onmessage above. The type is
		// pulled out with a regex instead of JSON.parse so logging never pays a
		// second serialization pass on MB-scale frames.
		if (import.meta.env.DEV) {
			const type = messageData.match(/"type"\s*:\s*"([^"]+)"/)?.[1];
			console.log('Sending message:', type ?? messageData);
		}
		this.#websocket.send(messageData);
	}

	// A WebSocket frame the backend parses as text, so bytes still go over as a
	// JSON number array here. The worker transport overrides this with a real
	// binary hand-off.
	async sendBytes(type: string, bytes: Uint8Array) {
		await this.send(JSON.stringify({ type, data: Array.from(bytes) }));
	}

	async sendAndWait(messageData: any): Promise<any> {
		return new Promise((resolve, reject) => {
			const messageType = messageData.type;
			this.#messageQueue.set(messageType, { resolve, reject });
			this.send(JSON.stringify(messageData));
		});
	}

	clear(messageType: string) {
		if (this.#message?.type === messageType) {
			this.#message = null;
		}
	}

	get message() {
		return this.#message;
	}

	set message(newMessage: Message | null) {
		this.#message = newMessage;
	}

	get connected() {
		return this.#connected;
	}
}

import { WorkerTransport } from './workerTransport.svelte';

// Vite statically replaces `import.meta.env.VITE_TRANSPORT`; unset (desktop/Docker
// builds) → undefined → the WebSocket transport. `build:web` sets it to 'worker'.
const bootTransport: Transport =
	import.meta.env.VITE_TRANSPORT === 'worker' ? new WorkerTransport() : new SocketState();

let delegate = $state<Transport>(bootTransport);
let bootContext: WSHandlerContext | null = null;

const transportFacade: Transport = {
	get kind() {
		return delegate.kind;
	},
	connect(context: WSHandlerContext) {
		bootContext = context;
		delegate.connect(context);
	},
	isConnected() {
		return delegate.isConnected();
	},
	send(messageData: string) {
		return delegate.send(messageData);
	},
	sendBytes(type: string, bytes: Uint8Array) {
		return delegate.sendBytes(type, bytes);
	},
	sendAndWait(messageData: any) {
		return delegate.sendAndWait(messageData);
	},
	clear(messageType: string) {
		delegate.clear(messageType);
	},
	get message() {
		return delegate.message;
	},
	set message(value: Message | null) {
		delegate.message = value;
	},
	get connected() {
		return delegate.connected;
	}
};

export function setTransportDelegate(transport: Transport): void {
	delegate = transport;
	if (bootContext) transport.connect(bootContext);
}

export function resetTransportDelegate(): void {
	delegate = bootTransport;
}

export const getSocketState = (): Transport => transportFacade;
