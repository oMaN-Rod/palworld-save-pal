import { describe, expect, it } from 'vitest';
import { initOoz, oozCompress, oozCompressSync, oozDecompress, oozDecompressSync } from './ooz';

function sample(): Uint8Array {
	const bytes = new Uint8Array(4096);
	for (let i = 0; i < bytes.length; i++) bytes[i] = (i * 7 + 13) & 0xff;
	return bytes;
}

describe('ooz.wasm', () => {
	it('compress → decompress round-trips exactly', async () => {
		const original = sample();
		const compressed = await oozCompress(original);
		expect(compressed.length).toBeGreaterThan(0);
		const restored = await oozDecompress(compressed, original.length);
		expect(restored.length).toBe(original.length);
		expect(Array.from(restored)).toEqual(Array.from(original));
	});

	// psp-core calls its Oodle bridge from inside a synchronous encode, so the
	// codec the worker lends it cannot be a promise — the module has to be up
	// before the first call, which is what `initOoz` is for.
	it('round-trips synchronously once initOoz has resolved', async () => {
		await initOoz();
		const original = sample();
		const compressed = oozCompressSync(original);
		expect(compressed.length).toBeGreaterThan(0);
		expect(Array.from(oozDecompressSync(compressed, original.length))).toEqual(
			Array.from(original)
		);
	});
});
