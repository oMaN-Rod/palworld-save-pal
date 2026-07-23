import { readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { describe, expect, it } from 'vitest';
import { DEFAULT_FOOTPRINT, blueprintStemFromAssetPath, parseBoxComponent } from './footprints.mjs';

const fixture = (name) =>
	JSON.parse(
		readFileSync(fileURLToPath(new URL(`./__fixtures__/${name}.json`, import.meta.url)), 'utf8')
	);

describe('parseBoxComponent', () => {
	it('doubles the half-extent and keeps the relative offset', () => {
		expect(parseBoxComponent(fixture('BP_BuildObject_PalBoxV2'))).toEqual({
			box: { sx: 500, sy: 400, sz: 390, ox: -150, oy: 0, oz: 200 },
			defaulted: false
		});
	});

	it('ignores the BuildWorkableBounds decoy component', () => {
		expect(parseBoxComponent(fixture('BP_BuildObject_PalBoxV2')).box.sx).not.toBe(1800);
	});

	it('is independent of property key order', () => {
		expect(parseBoxComponent(fixture('BP_BuildObject_ItemChest'))).toEqual({
			box: { sx: 66, sy: 88, sz: 72, ox: 0, oy: -1, oz: 38 },
			defaulted: false
		});
	});

	it('falls back to the default box when the keys are inherited', () => {
		expect(parseBoxComponent(fixture('BP_BuildObject_NoKeys'))).toEqual({
			box: DEFAULT_FOOTPRINT,
			defaulted: true
		});
	});

	it('returns null when there is no CheckOverlapCollision component', () => {
		expect(parseBoxComponent([{ Type: 'StaticMeshComponent', Name: 'Mesh' }])).toBeNull();
	});
});

describe('blueprintStemFromAssetPath', () => {
	it('strips the package path and the class suffix', () => {
		expect(
			blueprintStemFromAssetPath(
				'/Game/Pal/Blueprint/MapObject/BuildObject/BP_BuildObject_PalBoxV2.BP_BuildObject_PalBoxV2_C'
			)
		).toBe('BP_BuildObject_PalBoxV2');
	});

	it('returns null for the None sentinel', () => {
		expect(blueprintStemFromAssetPath('None')).toBeNull();
	});

	it('returns null for a null path', () => {
		expect(blueprintStemFromAssetPath(null)).toBeNull();
	});
});
