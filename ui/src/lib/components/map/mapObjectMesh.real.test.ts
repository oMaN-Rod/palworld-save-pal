// Guards the shipped bake, not the placement arithmetic: a failure here means the
// manifest changed. Imported from the tracked data/json source rather than
// ui/static, which .gitignore treats as a generated copy.
import { existsSync } from 'node:fs';
import { resolve } from 'node:path';
import { describe, expect, it } from 'vitest';
import manifestJson from '../../../../../data/json/map_object_meshes.json';
import { manifestParts, meshNames, type MapObjectManifest } from './mapObjectMesh';

const MANIFEST = manifestJson as unknown as MapObjectManifest;

const MODELS_DIR = resolve(__dirname, '../../../../static/models/mapobjects');

describe('the baked manifest', () => {
	it('covers all 14 actor classes', () => {
		expect(Object.keys(MANIFEST)).toHaveLength(14);
	});

	it('bakes 17 distinct meshes', () => {
		expect(meshNames(MANIFEST)).toHaveLength(17);
	});

	it('gives every relic a pedestal and a jewel', () => {
		for (const actorClass of Object.keys(MANIFEST)) {
			if (!actorClass.includes('Relic')) continue;
			expect(manifestParts(MANIFEST, actorClass)).toHaveLength(2);
		}
	});

	it('gives fast travel points a single statue', () => {
		expect(manifestParts(MANIFEST, 'BP_LevelObject_TowerFastTravelPoint_C')).toHaveLength(1);
	});

	// The tower and crystal come from a ChildActorComponent the actor does not own
	// directly; without them the statue renders alone in mid-air. Named
	// individually so a bake that drops one fails here rather than at a glance.
	it('gives the unlock map point its statue, tower and crystal', () => {
		expect(
			manifestParts(MANIFEST, 'BP_LevelObject_UnlockMapPoint_C').map((part) => part.mesh)
		).toEqual([
			'SM_FastTravelStatueVariant_185d80',
			'SM_pal_b07_PaldiumCrystal_02_3e214b',
			'SM_pal_map_small_tower_01_2e73df'
		]);
	});

	it("carries the game's own cull distance", () => {
		expect(MANIFEST['BP_LevelObject_TowerFastTravelPoint_C'].cullDistanceCm).toBe(30000);
		expect(MANIFEST['BP_LevelObject_Relic_C'].cullDistanceCm).toBe(100000);
	});

	// The game supplies no LDMaxDrawDistance for this class, so the bake substitutes
	// the 100000 every relic carries. A 47 m tower drawn at every zoom is why the
	// substitute exists, and dropping it would restore that silently.
	it('substitutes a cull distance for the unlock map point', () => {
		expect(MANIFEST['BP_LevelObject_UnlockMapPoint_C'].cullDistanceCm).toBe(100000);
	});

	// partLocalMatrix does convert pitch and roll, but that path is unvalidated:
	// carrying a UE rotation across the frame change is not the plain y/z swap it
	// looks like (the w sign flips too), and the discrepancy cancels only for yaw.
	// Pitch and roll are pinned one axis at a time but never against a UE ground
	// truth, so a bake that starts emitting them must fail here and be re-derived.
	it('rotates about yaw only', () => {
		for (const [actorClass, entry] of Object.entries(MANIFEST)) {
			for (const part of entry.parts) {
				expect({ actorClass, mesh: part.mesh, pitch: part.rot[0], roll: part.rot[2] }).toEqual({
					actorClass,
					mesh: part.mesh,
					pitch: 0,
					roll: 0
				});
			}
		}
	});

	// Placement multiplies these straight into a Matrix4, so a null or short tuple
	// from a future bake would produce NaN elements rather than throw.
	it('gives every part a finite 3-tuple loc, rot and scale', () => {
		for (const [actorClass, entry] of Object.entries(MANIFEST)) {
			for (const part of entry.parts) {
				for (const field of ['loc', 'rot', 'scale'] as const) {
					expect({ actorClass, field, value: part[field].length }).toEqual({
						actorClass,
						field,
						value: 3
					});
					expect(part[field].every(Number.isFinite)).toBe(true);
				}
			}
		}
	});

	// A statue baked at scale 0 renders as nothing at all, which on a map of
	// hundreds of markers reads as "that one just isn't placed yet".
	it('never bakes a degenerate scale', () => {
		for (const entry of Object.values(MANIFEST)) {
			for (const part of entry.parts) {
				for (const axis of part.scale) expect(Math.abs(axis)).toBeGreaterThan(0);
			}
		}
	});

	it('ships a glb for every mesh it names', () => {
		for (const name of meshNames(MANIFEST)) {
			expect({ name, present: existsSync(resolve(MODELS_DIR, `${name}.glb`)) }).toEqual({
				name,
				present: true
			});
		}
	});
});
