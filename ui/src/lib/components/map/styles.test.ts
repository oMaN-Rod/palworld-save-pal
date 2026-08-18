import { beforeEach, describe, expect, it } from 'vitest';
import {
	DEFAULT_MATERIAL_TINTS,
	DEFAULT_STRUCTURE_COLORS,
	resetMapColors,
	setMaterialBlend,
	setMaterialOpacity,
	setStructureColor
} from './mapColors.svelte';
import { structureFillColor, structureOpacity } from './styles';

describe('structureFillColor', () => {
	beforeEach(() => {
		resetMapColors();
	});

	it('returns the category color when material is None/undefined', () => {
		expect(structureFillColor('Foundation', 'None')).toBe(DEFAULT_STRUCTURE_COLORS.Foundation);
		expect(structureFillColor('Foundation', undefined)).toBe(DEFAULT_STRUCTURE_COLORS.Foundation);
	});

	it('returns a valid hex tint for a known material', () => {
		const c = structureFillColor('Foundation', 'Stone');
		expect(c).toMatch(/^#[0-9a-f]{6}$/i);
		expect(c).not.toBe(DEFAULT_STRUCTURE_COLORS.Foundation);
	});

	it('falls back to Other for an unknown category', () => {
		expect(structureFillColor('Nope', 'None')).toBe(DEFAULT_STRUCTURE_COLORS.Other);
	});

	it('ignores an unknown material', () => {
		expect(structureFillColor('Foundation', 'Unobtainium')).toBe(
			DEFAULT_STRUCTURE_COLORS.Foundation
		);
	});

	it('honours a structure color override', () => {
		setStructureColor('Foundation', '#010203');
		expect(structureFillColor('Foundation', 'None')).toBe('#010203');
	});

	it('yields the pure base color at blend 0', () => {
		setMaterialBlend(0);
		expect(structureFillColor('Foundation', 'Stone')).toBe(DEFAULT_STRUCTURE_COLORS.Foundation);
	});

	it('yields the pure tint at blend 1', () => {
		setMaterialBlend(1);
		expect(structureFillColor('Foundation', 'Stone')).toBe(DEFAULT_MATERIAL_TINTS.Stone);
	});
});

describe('structureOpacity', () => {
	beforeEach(() => {
		resetMapColors();
	});

	it('is fully opaque for absent, None, and unknown materials', () => {
		expect(structureOpacity(undefined)).toBe(1);
		expect(structureOpacity('None')).toBe(1);
		expect(structureOpacity('Unobtainium')).toBe(1);
	});

	it('returns the stock glass default', () => {
		expect(structureOpacity('Glass')).toBe(0.4);
	});

	it('honours an override', () => {
		setMaterialOpacity('Glass', 0.8);
		expect(structureOpacity('Glass')).toBe(0.8);
	});
});
