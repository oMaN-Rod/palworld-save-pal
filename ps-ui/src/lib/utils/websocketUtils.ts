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
	Promise.resolve(
		ws.send(
			JSON.stringify({
				type,
				data
			})
		)
	).catch((error: unknown) => {
		console.error(`Failed to send ${type}:`, error);
	});
}

/** Sends bulk bytes. May transfer the buffer — do not reuse `bytes` after. */
export function sendBytes(type: MessageType, bytes: Uint8Array): void {
	Promise.resolve(getSocketState().sendBytes(type, bytes)).catch((error: unknown) => {
		console.error(`Failed to send ${type}:`, error);
	});
}

export function isReady(): boolean {
	const ws = getSocketState();
	return ws.isConnected();
}

export function pushProgressMessage(data: any): void {
	const ws = getSocketState();
	ws.message = { type: MessageType.PROGRESS_MESSAGE, data };
}
