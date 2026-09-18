/**
 * `crypto.subtle.digest` cannot stream, so the whole file is read into memory to hash it; this cap
 * keeps that allocation within what a WebView reliably grants. The server's own limit is 2 GiB.
 */
export const MAX_UPLOAD_BYTES = 512 * 1024 * 1024;

const BINARY_SLICE = 0x8000;

export async function sha256Hex(blob: Blob): Promise<string> {
	const digest = await crypto.subtle.digest('SHA-256', await blob.arrayBuffer());
	return Array.from(new Uint8Array(digest), (byte) => byte.toString(16).padStart(2, '0')).join('');
}

export async function readChunk(blob: Blob, seq: number, chunkSize: number): Promise<Uint8Array> {
	const start = seq * chunkSize;
	return new Uint8Array(await blob.slice(start, start + chunkSize).arrayBuffer());
}

export function bytesToBase64(bytes: Uint8Array): string {
	let binary = '';
	for (let offset = 0; offset < bytes.length; offset += BINARY_SLICE) {
		binary += String.fromCharCode(...bytes.subarray(offset, offset + BINARY_SLICE));
	}
	return btoa(binary);
}
