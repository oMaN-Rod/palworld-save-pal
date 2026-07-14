import { PUBLIC_WS_URL } from '$env/static/public';
import { getDispatcher } from '$lib/ws/dispatcher';
import type { WSHandlerContext } from '$lib/ws/types';
import { type Message } from '$types';

const RECONNECT_DELAY = 5000;

class SocketState {
	#clientId = Date.now();
	#websocket!: WebSocket;
	#message = $state<Message | null>(null);
	#connected = $state(false);
	#dispatcher = getDispatcher();
	#messageQueue = new Map<string, (value: any) => void>();

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
			if (data.type && this.#messageQueue.has(data.type)) {
				const resolve = this.#messageQueue.get(data.type);
				if (resolve) {
					resolve(data);
					this.#messageQueue.delete(data.type);
					return;
				}
			}

			this.#message = data;

			console.log(`Received message: ${data.type}`, data);

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
		console.log(`Sending message: ${messageData}`);
		this.#websocket.send(messageData);
	}

	async sendAndWait(messageData: any): Promise<any> {
		return new Promise((resolve) => {
			const messageType = messageData.type;
			this.#messageQueue.set(messageType, resolve);
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

const socketStateInstance = new SocketState();

export const getSocketState = () => socketStateInstance;
