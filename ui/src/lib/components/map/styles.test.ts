import { describe, it, expect } from 'vitest';
import { structureFillColor, STRUCTURE_COLORS } from './styles';

describe('structureFillColor', () => {
	it('returns the category color when material is None/undefined', () => {
		expect(structureFillColor('Foundation', 'None')).toBe(STRUCTURE_COLORS.Foundation);
		expect(structureFillColor('Foundation', undefined)).toBe(STRUCTURE_COLORS.Foundation);
	});
	it('returns a valid hex tint for a known material', () => {
		const c = structureFillColor('Foundation', 'Stone');
		expect(c).toMatch(/^#[0-9a-f]{6}$/i);
		expect(c).not.toBe(STRUCTURE_COLORS.Foundation);
	});
	it('falls back to Other for an unknown category', () => {
		expect(structureFillColor('Nope', 'None')).toBe(STRUCTURE_COLORS.Other);
	});
});
