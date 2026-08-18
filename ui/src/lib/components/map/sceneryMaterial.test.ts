import * as THREE from 'three';
import { describe, expect, it } from 'vitest';
import {
	createSceneryMaterial,
	SCENERY_AMBIENT,
	SCENERY_CLIFF_COLOR,
	SCENERY_CLIFF_DARKEN,
	SCENERY_CLIFF_DESAT,
	setSceneryMaterialMap,
	setSceneryMaterialOpacity
} from './sceneryMaterial';
import { mosaicTexture, type TintMosaic } from './sceneryTint';

describe('createSceneryMaterial', () => {
	it('returns a ShaderMaterial with every uniform the fragment shader reads', () => {
		const material = createSceneryMaterial();
		expect(material).toBeInstanceOf(THREE.ShaderMaterial);
		for (const name of [
			'uMap',
			'uHasMap',
			'uBase',
			'uSun',
			'uAmbient',
			'uCliff',
			'uCliffStart',
			'uCliffEnd',
			'uCliffDesat',
			'uCliffDarken'
		]) {
			expect(material.uniforms).toHaveProperty(name);
		}
	});

	it('defaults uHasMap to 0 and uAmbient to SCENERY_AMBIENT', () => {
		const material = createSceneryMaterial();
		expect(material.uniforms.uHasMap.value).toBe(0);
		expect(material.uniforms.uAmbient.value).toBe(SCENERY_AMBIENT);
	});

	it('derives rock from the biome colour multiplicatively, so a face never outshines its ground', () => {
		const material = createSceneryMaterial();
		expect(SCENERY_CLIFF_DARKEN).toBeLessThanOrEqual(1);
		expect(SCENERY_CLIFF_DESAT).toBeGreaterThan(0);
		expect(SCENERY_CLIFF_DESAT).toBeLessThanOrEqual(1);
		// The old form added a constant floor (uCliff * (0.55 + 0.45 * base)), which let
		// a cliff face render brighter than a dark biome and flattened the terrain.
		expect(material.fragmentShader).not.toMatch(/0\.55\s*\+\s*0\.45/);
		expect(material.fragmentShader).toMatch(/uCliffDarken/);
	});

	it('uses a cool cliff tone, since it multiplies the sampled biome colour', () => {
		const { r, g, b } = new THREE.Color(SCENERY_CLIFF_COLOR);
		expect(b).toBeGreaterThan(r);
		expect(g).toBeGreaterThan(r);
	});

	it('does not declare instanceMatrix itself (three injects it for InstancedMesh)', () => {
		const material = createSceneryMaterial();
		expect(material.vertexShader).not.toMatch(/attribute\s+mat4\s+instanceMatrix/);
	});

	it('shades slope from n.z, not n.y -- our up axis is +Z', () => {
		const material = createSceneryMaterial();
		expect(material.fragmentShader).toContain('abs(n.z)');
	});
});

describe('setSceneryMaterialMap', () => {
	it('sets uMap.value and uHasMap to 1 when given a texture', () => {
		const material = createSceneryMaterial();
		const mosaic: TintMosaic = { data: new Uint8ClampedArray(4 * 4 * 4), size: 4 };
		const texture = mosaicTexture(mosaic);

		setSceneryMaterialMap(material, texture);

		expect(material.uniforms.uMap.value).toBe(texture);
		expect(material.uniforms.uHasMap.value).toBe(1);
	});

	it('clears uMap.value and resets uHasMap to 0 when given null', () => {
		const material = createSceneryMaterial();
		const mosaic: TintMosaic = { data: new Uint8ClampedArray(4 * 4 * 4), size: 4 };
		setSceneryMaterialMap(material, mosaicTexture(mosaic));

		setSceneryMaterialMap(material, null);

		expect(material.uniforms.uMap.value).toBeNull();
		expect(material.uniforms.uHasMap.value).toBe(0);
	});
});

describe('mosaicTexture', () => {
	it('wraps the mosaic as a DataTexture sized to the mosaic, with flipY false and NoColorSpace', () => {
		const size = 8;
		const mosaic: TintMosaic = { data: new Uint8ClampedArray(size * size * 4), size };

		const texture = mosaicTexture(mosaic);

		expect(texture.image.width).toBe(size);
		expect(texture.image.height).toBe(size);
		expect(texture.flipY).toBe(false);
		expect(texture.colorSpace).toBe(THREE.NoColorSpace);
	});

	it('filters with linear/mipmap-linear and generates mipmaps, so magnified and minified samples are smooth', () => {
		const size = 8;
		const mosaic: TintMosaic = { data: new Uint8ClampedArray(size * size * 4), size };

		const texture = mosaicTexture(mosaic);

		expect(texture.magFilter).toBe(THREE.LinearFilter);
		expect(texture.minFilter).toBe(THREE.LinearMipmapLinearFilter);
		expect(texture.generateMipmaps).toBe(true);
		expect(texture.wrapS).toBe(THREE.ClampToEdgeWrapping);
		expect(texture.wrapT).toBe(THREE.ClampToEdgeWrapping);
		expect(texture.anisotropy).toBe(8);
	});
});

describe('scenery opacity', () => {
	it('starts fully opaque, unblended, and depth-writing', () => {
		const m = createSceneryMaterial();
		expect(m.uniforms.uOpacity.value).toBe(1);
		expect(m.transparent).toBe(false);
		expect(m.depthWrite).toBe(true);
	});

	it('writes the uniform and turns on blending below 1', () => {
		const m = createSceneryMaterial();
		setSceneryMaterialOpacity(m, 0.4);
		expect(m.uniforms.uOpacity.value).toBeCloseTo(0.4, 5);
		expect(m.transparent).toBe(true);
	});

	// Rock instances share one InstancedMesh and cannot be sorted per instance, so
	// depth writing is all that keeps a far boulder from drawing over a near one.
	// Pals are made visible by clearing the depth buffer, not by holes in scenery.
	it('never stops writing depth at any opacity', () => {
		const m = createSceneryMaterial();
		for (const o of [1, 0.9, 0.5, 0.1]) {
			setSceneryMaterialOpacity(m, o);
			expect(m.depthWrite).toBe(true);
		}
	});

	it('returns to unblended at exactly 1', () => {
		const m = createSceneryMaterial();
		setSceneryMaterialOpacity(m, 0.3);
		setSceneryMaterialOpacity(m, 1);
		expect(m.transparent).toBe(false);
		expect(m.uniforms.uOpacity.value).toBe(1);
	});
});
