import { describe, expect, it } from 'vitest';
import { bytesToBase64, readChunk, sha256Hex } from './modUpload';

describe('sha256Hex', () => {
	it('hashes a blob to lower-case hex', async () => {
		expect(await sha256Hex(new Blob(['abc']))).toBe(
			'ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad'
		);
	});
});

describe('bytesToBase64', () => {
	it('encodes more bytes than one call may take as arguments', () => {
		const bytes = Uint8Array.from(
			{ length: 3 * 1024 * 1024 + 7 },
			(_, index) => (index * 31) % 256
		);
		expect(bytesToBase64(bytes)).toBe(Buffer.from(bytes).toString('base64'));
	});

	it('encodes an empty array', () => {
		expect(bytesToBase64(new Uint8Array())).toBe('');
	});
});

describe('readChunk', () => {
	it('reads the chunk for a sequence number, short at the end and empty past it', async () => {
		const blob = new Blob([new Uint8Array([0, 1, 2, 3, 4, 5, 6, 7, 8, 9])]);
		expect([...(await readChunk(blob, 0, 4))]).toEqual([0, 1, 2, 3]);
		expect([...(await readChunk(blob, 2, 4))]).toEqual([8, 9]);
		expect([...(await readChunk(blob, 3, 4))]).toEqual([]);
	});
});
