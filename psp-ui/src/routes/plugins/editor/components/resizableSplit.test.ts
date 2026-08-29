import { describe, expect, it } from 'vitest';
import {
	DEFAULT_SPLIT_RATIO,
	SPLIT_MAX_RATIO,
	SPLIT_MIN_RATIO,
	clampSplitRatio,
	ratioFromPointer,
	toggleOrientation
} from './resizableSplit';

describe('clampSplitRatio', () => {
	it('leaves a mid-range ratio untouched', () => {
		expect(clampSplitRatio(0.42)).toBe(0.42);
	});

	it('floors a ratio below the minimum', () => {
		expect(clampSplitRatio(0.01)).toBe(SPLIT_MIN_RATIO);
	});

	it('caps a ratio above the maximum', () => {
		expect(clampSplitRatio(0.99)).toBe(SPLIT_MAX_RATIO);
	});

	it('falls back to the default when the ratio is not finite', () => {
		expect(clampSplitRatio(Number.NaN)).toBe(DEFAULT_SPLIT_RATIO);
	});
});

describe('ratioFromPointer', () => {
	it('maps a pointer to the fraction of the container before it', () => {
		expect(ratioFromPointer(300, 100, 400)).toBe(0.5);
	});

	it('clamps a pointer dragged past the far edge', () => {
		expect(ratioFromPointer(500, 100, 400)).toBe(SPLIT_MAX_RATIO);
	});

	it('falls back to the default for a zero-sized container', () => {
		expect(ratioFromPointer(300, 100, 0)).toBe(DEFAULT_SPLIT_RATIO);
	});
});

describe('toggleOrientation', () => {
	it('swaps horizontal to vertical', () => {
		expect(toggleOrientation('horizontal')).toBe('vertical');
	});

	it('swaps vertical to horizontal', () => {
		expect(toggleOrientation('vertical')).toBe('horizontal');
	});
});
