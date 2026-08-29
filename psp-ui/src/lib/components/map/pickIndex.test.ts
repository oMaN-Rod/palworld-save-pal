import { describe, it, expect } from 'vitest';
import { PickIndex } from './pickIndex';

describe('PickIndex', () => {
	it('assigns contiguous bases across multiple buckets', () => {
		const idx = new PickIndex();
		expect(idx.add(['a', 'b'])).toBe(0);
		expect(idx.add(['c'])).toBe(2);
		expect(idx.add(['d', 'e', 'f'])).toBe(3);
		expect(idx.size).toBe(6);
	});

	it('maps every global index back to its own key', () => {
		const idx = new PickIndex();
		idx.add(['a', 'b']);
		idx.add(['c']);
		idx.add(['d', 'e', 'f']);
		expect(['a', 'b', 'c', 'd', 'e', 'f'].map((_, i) => idx.keyAt(i))).toEqual([
			'a', 'b', 'c', 'd', 'e', 'f'
		]);
	});

	it('returns null outside the assigned range', () => {
		const idx = new PickIndex();
		idx.add(['a']);
		expect(idx.keyAt(-1)).toBeNull();
		expect(idx.keyAt(1)).toBeNull();
	});

	it('starts over after reset so a rebuild cannot return a stale key', () => {
		const idx = new PickIndex();
		idx.add(['a', 'b']);
		idx.reset();
		expect(idx.size).toBe(0);
		expect(idx.keyAt(0)).toBeNull();
		expect(idx.add(['z'])).toBe(0);
		expect(idx.keyAt(0)).toBe('z');
	});
});
