import { describe, it, expect } from 'vitest';
import { tintOffsetAt, sampleTint, TINT_ZOOM, TINT_TILE_PX, TINT_MOSAIC_PX, type TintMosaic } from './sceneryTint';

// 4x4 RGBA mosaic with every texel distinct, so a row/column mixup shows up as a
// wrong colour rather than an accidental match.
function makeMosaic(): TintMosaic {
	const size = 4;
	const data = new Uint8ClampedArray(size * size * 4);
	for (let row = 0; row < size; row++) {
		for (let col = 0; col < size; col++) {
			const offset = (row * size + col) * 4;
			data[offset] = col * 10; // r
			data[offset + 1] = row * 10; // g
			data[offset + 2] = 200; // b
			data[offset + 3] = 255; // a
		}
	}
	return { data, size };
}

function setTexel(mosaic: TintMosaic, col: number, row: number, r: number, g: number, b: number, a: number) {
	const offset = (row * mosaic.size + col) * 4;
	mosaic.data[offset] = r;
	mosaic.data[offset + 1] = g;
	mosaic.data[offset + 2] = b;
	mosaic.data[offset + 3] = a;
}

describe('mosaic size constants', () => {
	it('keeps TINT_MOSAIC_PX derived from TINT_TILE_PX and TINT_ZOOM, not a drifting literal', () => {
		expect(TINT_MOSAIC_PX).toBe(TINT_TILE_PX * 2 ** TINT_ZOOM);
	});

	it('is pinned to zoom 3 / 4096px, one step up from the previous zoom-2 mosaic', () => {
		expect(TINT_ZOOM).toBe(3);
		expect(TINT_MOSAIC_PX).toBe(4096);
	});
});

describe('tintOffsetAt', () => {
	it('maps the centre of the mosaic to the centre texel', () => {
		expect(tintOffsetAt(0.5, 0.5, 4)).toBe((2 * 4 + 2) * 4);
	});

	it('maps the origin to texel 0', () => {
		expect(tintOffsetAt(0, 0, 4)).toBe(0);
	});

	it('clamps an out-of-range coordinate to the mosaic edge instead of wrapping', () => {
		const size = 4;
		const maxOffset = (size * size - 1) * 4;

		const atTopRight = tintOffsetAt(1, 1, size);
		expect(atTopRight).toBeGreaterThanOrEqual(0);
		expect(atTopRight).toBeLessThanOrEqual(maxOffset);
		expect(atTopRight).toBe((3 * 4 + 3) * 4);

		const beyondBothEdges = tintOffsetAt(1.5, -0.2, size);
		expect(beyondBothEdges).toBeGreaterThanOrEqual(0);
		expect(beyondBothEdges).toBeLessThanOrEqual(maxOffset);
		expect(beyondBothEdges).toBe((0 * 4 + 3) * 4);
	});
});

describe('sampleTint', () => {
	it('returns the channels at the sampled texel, normalised to 0..1', () => {
		const mosaic = makeMosaic();
		setTexel(mosaic, 1, 2, 30, 60, 90, 255);
		// col 1 of 4 -> mercX in [0.25, 0.5); row 2 of 4 -> mercY in [0.5, 0.75).
		const sample = sampleTint(mosaic, 0.3, 0.6);
		expect(sample).not.toBeNull();
		expect(sample!.r).toBeCloseTo(30 / 255, 10);
		expect(sample!.g).toBeCloseTo(60 / 255, 10);
		expect(sample!.b).toBeCloseTo(90 / 255, 10);
	});

	it('returns null for a fully transparent texel', () => {
		const mosaic = makeMosaic();
		setTexel(mosaic, 0, 0, 10, 20, 30, 0);
		expect(sampleTint(mosaic, 0, 0)).toBeNull();
	});

	it('selects different texels for a distinct mercX vs mercY, catching an x/y swap', () => {
		const mosaic = makeMosaic();
		setTexel(mosaic, 1, 0, 111, 0, 0, 255);
		setTexel(mosaic, 0, 1, 0, 222, 0, 255);

		// col 1, row 0
		const a = sampleTint(mosaic, 0.3, 0.1);
		// col 0, row 1
		const b = sampleTint(mosaic, 0.1, 0.3);

		expect(a).not.toBeNull();
		expect(b).not.toBeNull();
		expect(a!.r).toBeCloseTo(111 / 255, 10);
		expect(a!.g).toBeCloseTo(0, 10);
		expect(b!.r).toBeCloseTo(0, 10);
		expect(b!.g).toBeCloseTo(222 / 255, 10);
	});

	it('does not swap x/y at the real mosaic size (TINT_MOSAIC_PX)', () => {
		const size = TINT_MOSAIC_PX;
		const alongX = tintOffsetAt(0.75, 0.1, size);
		const alongY = tintOffsetAt(0.1, 0.75, size);

		expect(alongX).not.toBe(alongY);
		expect(alongX).toBeGreaterThanOrEqual(0);
		expect(alongX).toBeLessThan(size * size * 4);
		expect(alongY).toBeGreaterThanOrEqual(0);
		expect(alongY).toBeLessThan(size * size * 4);
	});
});
