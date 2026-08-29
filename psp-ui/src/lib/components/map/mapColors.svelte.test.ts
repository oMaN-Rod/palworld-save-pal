import { describe, it, expect, beforeEach } from 'vitest';
import {
	DEFAULT_MATERIAL_BLEND,
	DEFAULT_MATERIAL_OPACITY,
	DEFAULT_MATERIAL_TINTS,
	DEFAULT_STRUCTURE_COLORS,
	MATERIAL_ORDER,
	STRUCTURE_TYPE_ORDER,
	mapColors,
	materialBlend,
	materialOpacity,
	materialOpacities,
	materialTints,
	resetMapColors,
	setMaterialBlend,
	setMaterialOpacity,
	setMaterialTint,
	setStructureColor,
	structureColors
} from './mapColors.svelte';

describe('mapColors', () => {
	beforeEach(() => {
		resetMapColors();
	});

	it('starts from the stock palette', () => {
		expect(structureColors()).toEqual(DEFAULT_STRUCTURE_COLORS);
		expect(materialTints()).toEqual(DEFAULT_MATERIAL_TINTS);
		expect(materialBlend()).toBe(DEFAULT_MATERIAL_BLEND);
	});

	it('exposes a stable order for the UI', () => {
		expect(STRUCTURE_TYPE_ORDER).toEqual(Object.keys(DEFAULT_STRUCTURE_COLORS));
		expect(MATERIAL_ORDER).toEqual(Object.keys(DEFAULT_MATERIAL_TINTS));
	});

	it('applies a structure override', () => {
		setStructureColor('Defense', '#123456');
		expect(structureColors().Defense).toBe('#123456');
		expect(structureColors().Food).toBe(DEFAULT_STRUCTURE_COLORS.Food);
	});

	it('applies a material override', () => {
		setMaterialTint('Wood', '#abcdef');
		expect(materialTints().Wood).toBe('#abcdef');
		expect(materialTints().Stone).toBe(DEFAULT_MATERIAL_TINTS.Stone);
	});

	it('fills in defaults for keys missing from stored data', () => {
		mapColors.current = { structures: { Defense: '#123456' }, materials: {}, opacities: {}, blend: 0.25 };
		expect(structureColors().Defense).toBe('#123456');
		expect(structureColors().Foundation).toBe(DEFAULT_STRUCTURE_COLORS.Foundation);
		expect(materialTints().Glass).toBe(DEFAULT_MATERIAL_TINTS.Glass);
	});

	it('clamps blend into 0..1', () => {
		setMaterialBlend(5);
		expect(materialBlend()).toBe(1);
		setMaterialBlend(-2);
		expect(materialBlend()).toBe(0);
	});

	it('falls back to the default blend when stored data is not a finite number', () => {
		mapColors.current = { structures: {}, materials: {}, opacities: {}, blend: NaN };
		expect(materialBlend()).toBe(DEFAULT_MATERIAL_BLEND);
	});

	it('does not mutate the previous profile object', () => {
		const before = mapColors.current;
		setStructureColor('Pal', '#000000');
		expect(mapColors.current).not.toBe(before);
		expect(before.structures.Pal).not.toBe('#000000');
	});

	it('resets every field', () => {
		setStructureColor('Pal', '#000000');
		setMaterialTint('Metal', '#000000');
		setMaterialBlend(0.1);
		resetMapColors();
		expect(structureColors()).toEqual(DEFAULT_STRUCTURE_COLORS);
		expect(materialTints()).toEqual(DEFAULT_MATERIAL_TINTS);
		expect(materialBlend()).toBe(DEFAULT_MATERIAL_BLEND);
	});

	it('falls back to the default when a stored color is not valid hex', () => {
		mapColors.current = {
			structures: { Defense: 'ot' },
			materials: { Wood: '#zzzzzz' },
			opacities: {},
			blend: 0.25
		};
		expect(structureColors().Defense).toBe(DEFAULT_STRUCTURE_COLORS.Defense);
		expect(materialTints().Wood).toBe(DEFAULT_MATERIAL_TINTS.Wood);
	});

	it('does not throw when the stored profile is null', () => {
		// @ts-expect-error simulating a corrupted `null` payload from storage
		mapColors.current = null;
		expect(structureColors()).toEqual(DEFAULT_STRUCTURE_COLORS);
		expect(materialTints()).toEqual(DEFAULT_MATERIAL_TINTS);
		expect(materialBlend()).toBe(DEFAULT_MATERIAL_BLEND);
	});
});

describe('material opacity', () => {
	beforeEach(() => {
		resetMapColors();
	});

	it('defaults every material to opaque except glass', () => {
		expect(materialOpacities()).toEqual(DEFAULT_MATERIAL_OPACITY);
		expect(materialOpacity('Glass')).toBe(0.4);
		expect(materialOpacity('Stone')).toBe(1);
	});

	it('treats absent, None, and unknown materials as fully opaque', () => {
		expect(materialOpacity(undefined)).toBe(1);
		expect(materialOpacity('None')).toBe(1);
		expect(materialOpacity('Unobtainium')).toBe(1);
	});

	it('applies an override', () => {
		setMaterialOpacity('Glass', 0.75);
		expect(materialOpacity('Glass')).toBe(0.75);
		expect(materialOpacity('Stone')).toBe(1);
	});

	it('clamps out-of-range input', () => {
		setMaterialOpacity('Glass', 5);
		expect(materialOpacity('Glass')).toBe(1);
		setMaterialOpacity('Glass', -2);
		expect(materialOpacity('Glass')).toBe(0);
	});

	it('fills in defaults for keys missing from stored data', () => {
		mapColors.current = {
			structures: {},
			materials: {},
			opacities: { Glass: 0.2 },
			blend: 0.5
		};
		expect(materialOpacity('Glass')).toBe(0.2);
		expect(materialOpacity('Wood')).toBe(1);
	});

	it('rejects stored values that are not finite numbers in range', () => {
		mapColors.current = {
			structures: {},
			materials: {},
			opacities: { Glass: NaN, Wood: 4, Stone: '0.5' as unknown as number, Metal: -1 },
			blend: 0.5
		};
		expect(materialOpacity('Glass')).toBe(DEFAULT_MATERIAL_OPACITY.Glass);
		expect(materialOpacity('Wood')).toBe(1);
		expect(materialOpacity('Stone')).toBe(1);
		expect(materialOpacity('Metal')).toBe(1);
	});

	it('survives a null profile', () => {
		mapColors.current = null as unknown as typeof mapColors.current;
		expect(() => materialOpacities()).not.toThrow();
		expect(materialOpacity('Glass')).toBe(DEFAULT_MATERIAL_OPACITY.Glass);
	});

	it('is restored by reset', () => {
		setMaterialOpacity('Glass', 0.9);
		resetMapColors();
		expect(materialOpacities()).toEqual(DEFAULT_MATERIAL_OPACITY);
	});
});

// structureFillColor calls these once per structure per rebuild -- half a million
// times during a base load, and each used to rebuild the whole validated palette
// at ~7us. Deriving is pure in the stored profile, so an unchanged profile must
// hand back the very same object; identity is the contract the cost depends on.
describe('palette derivation is memoised', () => {
	beforeEach(() => {
		resetMapColors();
	});

	it('returns the identical structure palette while the profile is unchanged', () => {
		expect(structureColors()).toBe(structureColors());
	});

	it('returns the identical material tints while the profile is unchanged', () => {
		expect(materialTints()).toBe(materialTints());
	});

	it('returns the identical opacities while the profile is unchanged', () => {
		expect(materialOpacities()).toBe(materialOpacities());
	});

	it('hands back a new structure palette once a colour changes', () => {
		const before = structureColors();
		setStructureColor('Defense', '#123456');
		const after = structureColors();
		expect(after).not.toBe(before);
		expect(after.Defense).toBe('#123456');
		expect(before.Defense).toBe(DEFAULT_STRUCTURE_COLORS.Defense);
	});

	it('hands back new tints once a tint changes', () => {
		const before = materialTints();
		setMaterialTint('Wood', '#abcdef');
		const after = materialTints();
		expect(after).not.toBe(before);
		expect(after.Wood).toBe('#abcdef');
	});

	it('hands back new opacities once an opacity changes', () => {
		const before = materialOpacities();
		setMaterialOpacity('Glass', 0.9);
		const after = materialOpacities();
		expect(after).not.toBe(before);
		expect(after.Glass).toBe(0.9);
	});

	it('re-derives after a reset', () => {
		setStructureColor('Defense', '#123456');
		const overridden = structureColors();
		resetMapColors();
		const restored = structureColors();
		expect(restored).not.toBe(overridden);
		expect(restored.Defense).toBe(DEFAULT_STRUCTURE_COLORS.Defense);
	});

	// The palette is now shared rather than copied per call, so a caller that
	// mutated it would poison every later reader. Freezing turns that into an
	// immediate error instead of an action-at-a-distance bug.
	it('is frozen, so a caller cannot corrupt the shared palette', () => {
		expect(Object.isFrozen(structureColors())).toBe(true);
		expect(() => {
			(structureColors() as Record<string, string>).Defense = '#000000';
		}).toThrow();
	});

	it('still derives a usable palette when the stored profile is missing', () => {
		mapColors.current = null as unknown as typeof mapColors.current;
		expect(structureColors()).toEqual(DEFAULT_STRUCTURE_COLORS);
		expect(structureColors()).toBe(structureColors());
	});
});
