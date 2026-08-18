import { describe, expect, it } from 'vitest';
import { decodePickBytes, MAX_PICK_INDEX } from './pickEncoding';

// Mirrors the pick vertex shader exactly: id = index + 1, split into 3 bytes.
function shaderEncode(index: number): [number, number, number] {
	const id = index + 1;
	return [Math.floor(id / 65536), Math.floor((id % 65536) / 256), id % 256];
}

describe('decodePickBytes', () => {
	it('treats an all-zero pixel as a miss', () => {
		expect(decodePickBytes(0, 0, 0)).toBe(-1);
	});

	it('round-trips the shader encoding across byte boundaries', () => {
		for (const i of [0, 1, 2, 254, 255, 256, 257, 65534, 65535, 65536, 65537, MAX_PICK_INDEX]) {
			const [r, g, b] = shaderEncode(i);
			expect(decodePickBytes(r, g, b)).toBe(i);
		}
	});

	it('keeps every byte within range for the maximum index', () => {
		for (const c of shaderEncode(MAX_PICK_INDEX)) {
			expect(c).toBeGreaterThanOrEqual(0);
			expect(c).toBeLessThanOrEqual(255);
		}
	});
});
