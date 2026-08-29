import { describe, expect, it } from 'vitest';
import { featureTypeLabel } from './labels';

describe('featureTypeLabel', () => {
	it('capitalizes a single word', () => {
		expect(featureTypeLabel('dungeon')).toBe('Dungeon');
	});

	it('splits underscores into words', () => {
		expect(featureTypeLabel('fast_travel')).toBe('Fast Travel');
		expect(featureTypeLabel('predator_pal')).toBe('Predator Pal');
	});

	it('is empty for an empty type', () => {
		expect(featureTypeLabel('')).toBe('');
	});
});
