import { describe, expect, it } from 'vitest';

import { skillBorderClass, skillFilter, skillOpacity } from './colors';

/*
 * Rank 5 is the WorldTree passive tier. It is a top-tier positive passive and
 * must read the same as rank 4; red is reserved for the detrimental ranks
 * (-1, -2, -3).
 */
describe('rank colouring', () => {
	it('renders rank 5 with the same border as rank 4', () => {
		expect(skillBorderClass(5)).toBe(skillBorderClass(4));
	});

	it('renders rank 5 with the same icon filter as rank 4', () => {
		expect(skillFilter(5)).toBe(skillFilter(4));
	});

	it('renders rank 5 with the same opacity as rank 4', () => {
		expect(skillOpacity(5)).toBe(skillOpacity(4));
	});

	it('does not paint rank 5 red', () => {
		expect(skillBorderClass(5)).not.toContain('FF0000');
		expect(skillFilter(5)).not.toBe(skillFilter(-1));
	});

	it.each([-1, -2, -3])('keeps detrimental rank %i red', (rank) => {
		expect(skillBorderClass(rank)).toContain('FF0000');
		expect(skillOpacity(rank)).toBe('opacity-15');
	});

	it('leaves ranks 1-4 unchanged', () => {
		expect(skillBorderClass(1)).toBe('border-l-surface-600');
		expect(skillBorderClass(2)).toBe('border-l-[#fcdf19]');
		expect(skillBorderClass(3)).toBe('border-l-[#fcdf19]');
		expect(skillBorderClass(4)).toBe('border-l-[#68ffd8]');
		expect(skillFilter(1)).toBe('');
		expect(skillOpacity(1)).toBe('opacity-25');
	});
});
