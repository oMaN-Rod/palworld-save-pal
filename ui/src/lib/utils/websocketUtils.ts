import { getSocketState } from '$states/websocketState.svelte';
import { MessageType } from '$types';

export async function sendAndWait<T>(type: MessageType, data?: any): Promise<T> {
	const ws = getSocketState();
	const response = await ws.sendAndWait({
		type,
		data
	});

	if (response.type === 'error') {
		throw new Error(response.data);
	}

	return response.data;
}

export function send(type: MessageType, data?: any): void {
	const ws = getSocketState();
	ws.send(
		JSON.stringify({
			type,
			data
		})
	);
}

export function isReady(): boolean {
	const ws = getSocketState();
	return ws.isConnected();
}

export function pushProgressMessage(data: any): void {
	const ws = getSocketState();
	ws.message = { type: MessageType.PROGRESS_MESSAGE, data };
}
