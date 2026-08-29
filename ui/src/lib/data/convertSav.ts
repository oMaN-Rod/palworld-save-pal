import { WorkerTransport } from '$lib/states/workerTransport.svelte';
import { getSocketState } from '$states/websocketState.svelte';

// Worker-internal frame types, deliberately absent from `MessageType`: nothing
// sends them over the WebSocket. The browser build converts in its own wasm
// engine, the desktop and Docker builds POST to the backend's /api/convert.
const SAV_TO_JSON = 'convert_sav_to_json';
const JSON_TO_SAV = 'convert_json_to_sav';

type ConvertReply = { data: { json?: string; bytes?: Uint8Array; error?: string } };

function replyError(reply: ConvertReply): Error | null {
	if (reply.data.error) return new Error(reply.data.error);
	return null;
}

/**
 * Reads a `.sav` into the uesave JSON the raw editor holds. May transfer
 * `bytes` — do not reuse the buffer after.
 */
export async function savToJson(bytes: Uint8Array, fileName: string): Promise<string> {
	const transport = getSocketState();
	if (transport instanceof WorkerTransport) {
		const reply = await transport.sendRawAndWait<ConvertReply>({ type: SAV_TO_JSON, bytes }, [
			bytes.buffer as ArrayBuffer
		]);
		const error = replyError(reply);
		if (error) throw error;
		return reply.data.json ?? '';
	}

	const body = new FormData();
	body.append('file', new Blob([bytes as BlobPart]), fileName);
	const response = await fetch('/api/convert/sav-to-json', { method: 'POST', body });
	if (!response.ok) throw new Error(`Server error: ${response.status}`);
	return response.text();
}

/** Writes edited uesave JSON back to `.sav` bytes. */
export async function jsonToSav(json: string): Promise<Uint8Array> {
	const transport = getSocketState();
	if (transport instanceof WorkerTransport) {
		const reply = await transport.sendRawAndWait<ConvertReply>({ type: JSON_TO_SAV, json });
		const error = replyError(reply);
		if (error) throw error;
		return reply.data.bytes ?? new Uint8Array();
	}

	const response = await fetch('/api/convert/json-to-sav', {
		method: 'POST',
		headers: { 'Content-Type': 'application/json' },
		body: json
	});
	if (!response.ok) throw new Error(`Server error: ${response.status}`);
	return new Uint8Array(await response.arrayBuffer());
}
