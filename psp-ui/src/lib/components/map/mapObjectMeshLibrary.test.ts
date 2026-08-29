import { describe, expect, it } from 'vitest';
import * as THREE from 'three';
import { bundleMapObjectMesh } from './mapObjectMeshLibrary';

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
