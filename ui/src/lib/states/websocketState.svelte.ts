import { PUBLIC_WS_URL } from '$env/static/public';
import { MessageType, type Message } from '$types';
import { ExponentialBackoff, handleAll, retry } from 'cockatiel';

const retryPolicy = retry(handleAll, { maxAttempts: 3, backoff: new ExponentialBackoff() });
const RECONNECT_DELAY = 5000;

export function createSocketState() {
	const clientId = Date.now();
	let websocket: WebSocket;
	let message: Message | null = $state(null);
	let connected: boolean = $state(false);
	let messageQueue: Map<string, (value: any) => void> = new Map();

	function connect() {
		const protocol = window.location.protocol === 'https:' ? 'wss://' : 'ws://';
		const wsUrl = `${protocol}${PUBLIC_WS_URL}/${clientId}`;
		websocket = new WebSocket(wsUrl);

		websocket.onopen = async () => {
			connected = true;
			websocket.send(JSON.stringify({ type: MessageType.SYNC_APP_STATE }));
			console.log('Connected to backend!');
		};

		websocket.onmessage = (event) => {
			const data = JSON.parse(event.data);
			message = data;
			message = { ...message } as Message;
			console.log('Received message:', JSON.stringify(message, null, 2));

			if (message.type && messageQueue.has(message.type)) {
				const resolve = messageQueue.get(message.type);
				if (resolve) {
					resolve(message);
					messageQueue.delete(message.type);
				}
			}
		};

		websocket.onclose = (event) => {
			console.log('Connection closed:', event);
			connected = false;
			message = {
				type: MessageType.ERROR,
				data: 'Lost connection to backend! Attempting to reconnect...'
			};
			setTimeout(connect, RECONNECT_DELAY);
		};
	}

	connect();

	async function send(messageData: string) {
		console.log('Sending message:', messageData);
		while (websocket.readyState !== websocket.OPEN) {
			await new Promise((resolve) => setTimeout(resolve, 250));
		}
		websocket.send(messageData);
	}

	async function sendAndWait(messageData: any): Promise<any> {
		return new Promise((resolve) => {
			const messageType = messageData.type;
			messageQueue.set(messageType, resolve);
			send(JSON.stringify(messageData));
		});
	}

	function clear(messageType: MessageType) {
		console.log('Clearing message:', messageType);
		if (message && message.type === messageType) {
			message = null;
			console.log('Message cleared', messageType);
		} else {
			console.log('Message not cleared - type mismatch or message is null');
		}
	}

	return {
		get message() {
			return message;
		},
		set message(newMessage: Message | null) {
			console.log('Setting new message:', newMessage);
			message = newMessage;
		},
		get connected() {
			return connected;
		},
		send,
		sendAndWait,
		clear
	};
}

let socketStore: ReturnType<typeof createSocketState>;

export function getSocketState() {
	if (!socketStore) {
		socketStore = createSocketState();
	}
	return socketStore;
}
