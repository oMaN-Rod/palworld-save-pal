import * as THREE from 'three';
import { describe, expect, it } from 'vitest';
import { bundleMapObjectMesh } from './mapObjectMeshLibrary';

// Material configuration is shared with palMeshLibrary and tested in
// meshLibrary.test.ts. This file covers only what is specific to map objects:
// bundling one glb's primitives into what an InstancedMesh needs.
describe('bundleMapObjectMesh', () => {
	it('returns null for an empty glb', () => {
		expect(bundleMapObjectMesh([], [])).toBeNull();
	});

	it('returns the geometry and material untouched for a single-primitive glb', () => {
		const geometry = new THREE.BoxGeometry(1, 1, 1);
		const material = new THREE.MeshStandardMaterial();
		const bundle = bundleMapObjectMesh([geometry], [material]);
		expect(bundle?.geometry).toBe(geometry);
		expect(bundle?.material).toBe(material);
	});

	// SM_FastTravelStatueVariant and SM_JewelBase both ship two primitives with
	// two different materials -- the case a single shared material would render
	// wrong for one half of the mesh.
	it('merges a multi-primitive glb with groups so each primitive keeps its own material', () => {
		const geoA = new THREE.BoxGeometry(1, 1, 1);
		const geoB = new THREE.BoxGeometry(1, 1, 1);
		const matA = new THREE.MeshStandardMaterial();
		const matB = new THREE.MeshStandardMaterial();

		const bundle = bundleMapObjectMesh([geoA, geoB], [matA, matB]);

		expect(Array.isArray(bundle?.material)).toBe(true);
		const materials = bundle!.material as THREE.Material[];
		expect(materials).toEqual([matA, matB]);

		expect(bundle!.geometry.groups).toHaveLength(2);
		expect(bundle!.geometry.groups[0].materialIndex).toBe(0);
		expect(bundle!.geometry.groups[1].materialIndex).toBe(1);
	});
});
