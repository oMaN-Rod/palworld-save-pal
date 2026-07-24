import { describe, it, expect } from 'vitest';
import { oozCompress, oozDecompress } from '../src/lib/worker/ooz';

describe('ooz.wasm', () => {
	it('compress → decompress round-trips exactly', async () => {
		const original = new Uint8Array(4096);
		for (let i = 0; i < original.length; i++) original[i] = (i * 7 + 13) & 0xff;
		const compressed = await oozCompress(original);
		expect(compressed.length).toBeGreaterThan(0);
		const restored = await oozDecompress(compressed, original.length);
		expect(restored.length).toBe(original.length);
		expect(Array.from(restored)).toEqual(Array.from(original));
	});
});
