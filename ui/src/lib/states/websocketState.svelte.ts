import { PUBLIC_WS_URL } from '$env/static/public';
import { getDispatcher } from '$lib/ws/dispatcher';
import type { WSHandlerContext } from '$lib/ws/types';
import { type Message } from '$types';

const RECONNECT_DELAY = 5000;

class SocketState {
	#clientId = Date.now();
	#websocket!: WebSocket;
	// $state.raw: handler-routed frames are dispatched and forgotten — nothing
	// reads `ws.message` deeply, so a deep proxy only adds per-payload cost.
	#message = $state.raw<Message | null>(null);
	#connected = $state(false);
	#dispatcher = getDispatcher();
	#messageQueue = new Map<string, ((value: any) => void)[]>();

	connect(context: WSHandlerContext) {
		const protocol = window.location.protocol === 'https:' ? 'wss://' : 'ws://';
		const wsUrl = `${protocol}${PUBLIC_WS_URL}/${this.#clientId}`;
		this.#websocket = new WebSocket(wsUrl);

		this.#websocket.onopen = () => {
			this.#connected = true;
			console.log('Connected to backend!');
		};

		this.#websocket.onmessage = async (event) => {
			const data = JSON.parse(event.data);
			if (!data) return;

			// Resolve queued sendAndWait calls with the raw parsed data: routing it
			// through the #message $state proxy makes every consumer read through a
			// deeply reactive proxy (thousands of tracked reads for large payloads).
			// Resolvers are queued per type so concurrent same-type requests each
			// settle instead of overwriting each other.
			if (data.type && this.#messageQueue.has(data.type)) {
				const resolvers = this.#messageQueue.get(data.type);
				if (resolvers) {
					this.#messageQueue.delete(data.type);
					for (const resolve of resolvers) {
						resolve(data);
					}
					return;
				}
			}

			this.#message = data;

			// Dev-only and type-only: logging full payloads retains them in DevTools
			// (a memory leak sized to the save) and serializing MB-scale frames
			// during loads costs tens of milliseconds.
			if (import.meta.env.DEV) console.log('Received message:', data.type);

			await this.#dispatcher.dispatch(data, context);
		};

		this.#websocket.onclose = () => {
			this.#connected = false;
			setTimeout(() => this.connect(context), RECONNECT_DELAY);
		};
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
		return new Promise((resolve) => {
			const messageType = messageData.type;
			// Queue, don't overwrite: a second concurrent request of the same
			// type must not orphan the first promise.
			const resolvers = this.#messageQueue.get(messageType) ?? [];
			resolvers.push(resolve);
			this.#messageQueue.set(messageType, resolvers);
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
const socketStateInstance =
	import.meta.env.VITE_TRANSPORT === 'worker' ? new WorkerTransport() : new SocketState();

export const getSocketState = () => socketStateInstance;
