import { PUBLIC_WS_URL } from '$env/static/public';
import { getDispatcher } from '$lib/ws/dispatcher';
import type { WSHandlerContext } from '$lib/ws/types';
import { type Message } from '$types';

const RECONNECT_DELAY = 5000;

export function createSocketState() {
	const clientId = Date.now();
	let websocket: WebSocket;
	let message: Message | null = $state(null);
	let connected: boolean = $state(false);
	const dispatcher = getDispatcher();
	let messageQueue = new Map<string, (value: any) => void>();

	function connect(context: WSHandlerContext) {
		const protocol = window.location.protocol === 'https:' ? 'wss://' : 'ws://';
		const wsUrl = `${protocol}${PUBLIC_WS_URL}/${clientId}`;
		websocket = new WebSocket(wsUrl);

		websocket.onopen = () => {
			connected = true;
			console.log('Connected to backend!');
		};

		websocket.onmessage = async (event) => {
			const data = JSON.parse(event.data);
			message = data;
			if (!message) return;
			const messageSnapshot = $state.snapshot(message);
			console.log(`Received message: ${message.type}`, messageSnapshot);
			if (message.type && messageQueue.has(message.type)) {
				const resolve = messageQueue.get(message.type);
				if (resolve) {
					resolve(message);
					messageQueue.delete(message.type);
					return;
				}
			}

			await dispatcher.dispatch(message, context);
		};

		websocket.onclose = () => {
			connected = false;
			setTimeout(() => connect(context), RECONNECT_DELAY);
		};
	}

	async function send(messageData: string) {
		while (websocket.readyState !== websocket.OPEN) {
			await new Promise((resolve) => setTimeout(resolve, 250));
		}
		console.log(`Sending message: ${messageData}`);
		websocket.send(messageData);
	}

	async function sendAndWait(messageData: any): Promise<any> {
		return new Promise((resolve) => {
			const messageType = messageData.type;
			messageQueue.set(messageType, resolve);
			send(JSON.stringify(messageData));
		});
	}

	function clear(messageType: string) {
		if (message?.type === messageType) {
			message = null;
		}
	}

	return {
		get message() {
			return message;
		},
		set message(newMessage: Message | null) {
			message = newMessage;
		},
		get connected() {
			return connected;
		},
		send,
		sendAndWait,
		clear,
		connect
	};
}

const socketState: ReturnType<typeof createSocketState> = createSocketState();
export const getSocketState = () => socketState;
