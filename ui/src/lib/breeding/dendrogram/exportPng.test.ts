import { describe, expect, it } from 'vitest';
import { fitBounds, slugify } from './exportPng';

describe('fitBounds', () => {
	it('maps a tree bbox into a margin-padded canvas with the tree at the origin', () => {
		// Tree laid out from (0,0) — the common dendrogram case.
		expect(fitBounds({ x: 0, y: 0, width: 400, height: 300 }, 32)).toEqual({
			dx: 32,
			dy: 32,
			width: 464,
			height: 364
		});
	});

	it('compensates a non-zero bbox origin so content lands inside the margin', () => {
		expect(fitBounds({ x: 10, y: 20, width: 100, height: 50 }, 8)).toEqual({
			dx: -2,
			dy: -12,
			width: 116,
			height: 66
		});
	});

	it('never returns a degenerate (sub-pixel) canvas', () => {
		expect(fitBounds({ x: 0, y: 0, width: 0, height: 0 }, 10)).toEqual({
			dx: 10,
			dy: 10,
			width: 20,
			height: 20
		});
	});
});

describe('slugify', () => {
	it('slugifies a tribe name for the export filename', () => {
		expect(slugify('LazyDragon_Electric')).toBe('lazydragon-electric');
		expect(slugify('Anubis')).toBe('anubis');
	});

	it('handles spaces, punctuation, and empty input', () => {
		expect(slugify(' Jet Dragon !! ')).toBe('jet-dragon');
		expect(slugify('')).toBe('dendrogram');
		expect(slugify('---')).toBe('dendrogram');
	});
});
