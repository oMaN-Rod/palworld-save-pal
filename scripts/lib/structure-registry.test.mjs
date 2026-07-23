import { describe, it, expect } from 'vitest';
import { classifyArchetype, detectMaterial } from './structure-registry.mjs';

describe('classifyArchetype', () => {
	it('classifies by name first', () => {
		expect(classifyArchetype('Wooden_TriangleRoof', [], 'Foundation')).toBe('gableRoof');
		expect(classifyArchetype('Stone_PyramidRoof', [], 'Foundation')).toBe('pyramidRoof');
		expect(classifyArchetype('Metal_Stairs', [], 'Foundation')).toBe('stair');
		expect(classifyArchetype('Wooden_DoorWall', [], 'Foundation')).toBe('wallDoor');
		expect(classifyArchetype('Wooden_Wall', [], 'Foundation')).toBe('wall');
		expect(classifyArchetype('Wooden_Foundation', [], 'Foundation')).toBe('foundation');
		expect(classifyArchetype('Wooden_Fence', [], 'Foundation')).toBe('fence');
	});

	it('falls back to blueprint mesh names when the id is opaque', () => {
		expect(classifyArchetype('DefenseTurret2', ['SM_Turret_Base', 'SM_Turret_Barrel'], 'Defense')).toBe('turret');
	});

	it('falls back to a category default, then box', () => {
		expect(classifyArchetype('WeirdThing', [], 'Storage')).toBe('chest');
		expect(classifyArchetype('WeirdThing', [], 'Other')).toBe('box');
	});
});

describe('detectMaterial', () => {
	it('reads the name prefix', () => {
		expect(detectMaterial('Stone_Wall', undefined)).toBe('Stone');
		expect(detectMaterial('Wooden_Wall', undefined)).toBe('Wood');
	});
	it('reads MaterialType enum when name is silent', () => {
		expect(detectMaterial('PalBoxV2', 'EPalBuildMaterialType::Metal')).toBe('Metal');
	});
	it('returns None when unknown', () => {
		expect(detectMaterial('Altar', undefined)).toBe('None');
	});
});
