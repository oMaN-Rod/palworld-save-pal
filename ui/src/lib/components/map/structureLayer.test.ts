import { describe, it, expect, beforeEach, afterEach, vi } from 'vitest';
import * as THREE from 'three';

// Mesh-path tests need real GLTFLoader.load() calls to intercept: structure mesh
// resolution goes through requestMesh/requestTexturedMesh, which share this
// loader singleton.
const { loadCalls } = vi.hoisted(() => ({
	loadCalls: [] as Array<{
		url: string;
		onLoad: (gltf: { scene: THREE.Object3D }) => void;
		onError: (err: unknown) => void;
	}>
}));

vi.mock('three/examples/jsm/loaders/GLTFLoader.js', () => ({
	GLTFLoader: vi.fn().mockImplementation(() => ({
		setDRACOLoader: vi.fn(),
		setMeshoptDecoder: vi.fn(),
		load: (
			url: string,
			onLoad: (gltf: { scene: THREE.Object3D }) => void,
			_onProgress: unknown,
			onError: (err: unknown) => void
		) => {
			loadCalls.push({ url, onLoad, onError });
		}
	}))
}));

vi.mock('three/examples/jsm/loaders/DRACOLoader.js', () => ({
	DRACOLoader: vi.fn().mockImplementation(() => ({
		setDecoderPath: vi.fn()
	}))
}));

import {
	meshInstanceMatrix,
	proxyInstanceMatrix,
	pickPixelCoords,
	texturedGroupMaterial,
	createStructureLayer
} from './structureLayer';
import { MercatorCoordinate } from 'maplibre-gl';
import { resetMapColors, setMaterialOpacity, setMaterialTint } from './mapColors.svelte';
import type { MeshPart } from './meshPlacement';
import type { BaseStructure, Footprint } from '$types';
import { requestMesh, type TexturedMeshBundle } from './meshLibrary';
import manifest from '../../../../../data/json/structure_meshes.json';

// Non-unit on purpose, so a stray latitude factor in the code under test cannot
// hide behind cmToMerc = 1.
const CM_TO_MERC = 2.5e-9;

const identityPart: MeshPart = { loc: [0, 0, 0], rot: [0, 0, 0], scale: [1, 1, 1] };

const base = (over: Partial<BaseStructure>): BaseStructure => ({
	instance_id: 'i',
	map_object_id: 'Wooden_DoorWall',
	x: 0,
	y: 0,
	z: 0,
	yaw: 0,
	scale_x: 1,
	scale_y: 1,
	scale_z: 1,
	hp_current: 1,
	hp_max: 1,
	build_player_uid: 'u',
	...over
});

// Wooden_DoorWall's real footprint offset from the manifest -- carried by 791 of
// 815 mesh-backed structure ids. A regression that lets it leak into the mesh
// path again would shift every one of them off the ground.
const fpWithBoxOffset: Footprint = {
	sx: 400,
	sy: 20,
	sz: 325,
	ox: 0,
	oy: 0,
	oz: 158.8,
	typeA: 'Foundation',
	archetype: 'wallDoor'
};

// Divide by cmToMerc, full stop -- no MercatorCoordinate lookup, no per-point
// latitude. A helper that re-derived latitude would encode the very bug these
// tests exist to catch.
function altitudeCmFromMatrix(matrix: THREE.Matrix4, cmToMerc: number): number {
	const position = new THREE.Vector3().setFromMatrixPosition(matrix);
	return position.z / cmToMerc;
}

describe('meshInstanceMatrix', () => {
	it('anchors mesh altitude at the raw actor z, not the collision-box footprint offset', () => {
		const s = base({ z: 5000 });
		const matrix = meshInstanceMatrix(s, identityPart, 'MainMap', 1, CM_TO_MERC);
		const altitude = altitudeCmFromMatrix(matrix, CM_TO_MERC);

		expect(altitude).toBeCloseTo(s.z, 5);
		expect(altitude).not.toBeCloseTo(s.z + fpWithBoxOffset.oz, 5);
	});

	// Discriminator for the double-latitude-correction defect. cmToMerc already
	// carries the camera centre's latitude, so no per-instance term remains: two
	// structures with the same z but very different (deliberately non-mirror-image,
	// since mirrored points share cos(lat)) x must give the same matrix Z. The
	// buggy version differs by cos(centerLat)/cos(instanceLat), ~4.14% here, while
	// the fixed one is bit-identical.
	it('produces the same altitude regardless of latitude', () => {
		const sNorth = base({ x: -1_099_400, y: 0, z: 5000 });
		const sSouth = base({ x: 340_000, y: 0, z: 5000 });

		const matrixNorth = meshInstanceMatrix(sNorth, identityPart, 'MainMap', 1, CM_TO_MERC);
		const matrixSouth = meshInstanceMatrix(sSouth, identityPart, 'MainMap', 1, CM_TO_MERC);

		const zNorth = new THREE.Vector3().setFromMatrixPosition(matrixNorth).z;
		const zSouth = new THREE.Vector3().setFromMatrixPosition(matrixSouth).z;

		const relativeDiff = Math.abs(zNorth - zSouth) / Math.abs(zSouth);
		expect(relativeDiff).toBeLessThan(1e-9);
	});
});

describe('proxyInstanceMatrix (unchanged proxy behavior)', () => {
	it('still bakes the footprint origin/half-height offset in for the proxy box path', () => {
		const s = base({ z: 5000 });
		const matrix = proxyInstanceMatrix(s, fpWithBoxOffset, 'wallDoor', 'MainMap', 1, CM_TO_MERC);
		const altitude = altitudeCmFromMatrix(matrix, CM_TO_MERC);

		const halfH = fpWithBoxOffset.sz / 2;
		expect(altitude).toBeCloseTo(s.z + fpWithBoxOffset.oz - halfH, 5);
	});

	// The same discriminator as above, applied to the proxy path's own anchor
	// computation.
	it('produces the same altitude regardless of latitude', () => {
		const sNorth = base({ x: -1_099_400, y: 0, z: 5000 });
		const sSouth = base({ x: 340_000, y: 0, z: 5000 });

		const matrixNorth = proxyInstanceMatrix(sNorth, fpWithBoxOffset, 'wallDoor', 'MainMap', 1, CM_TO_MERC);
		const matrixSouth = proxyInstanceMatrix(sSouth, fpWithBoxOffset, 'wallDoor', 'MainMap', 1, CM_TO_MERC);

		const zNorth = new THREE.Vector3().setFromMatrixPosition(matrixNorth).z;
		const zSouth = new THREE.Vector3().setFromMatrixPosition(matrixSouth).z;

		const relativeDiff = Math.abs(zNorth - zSouth) / Math.abs(zSouth);
		expect(relativeDiff).toBeLessThan(1e-9);
	});
});

describe('pickPixelCoords', () => {
	it('converts CSS pixels to device pixels at ratio 1', () => {
		expect(pickPixelCoords(10, 20, 1, 100, 50)).toEqual({ x: 10, y: 29 });
	});

	it('converts CSS pixels to device pixels at ratio 2 (HiDPI)', () => {
		expect(pickPixelCoords(10, 20, 2, 200, 100)).toEqual({ x: 20, y: 59 });
	});

	it('flips Y so CSS y=0 (top) lands on the last device row (bottom-left origin)', () => {
		expect(pickPixelCoords(0, 0, 1, 100, 50)).toEqual({ x: 0, y: 49 });
	});

	it('rejects a point beyond the right edge', () => {
		expect(pickPixelCoords(100, 0, 1, 100, 50)).toBeNull();
	});

	it('rejects a point beyond the bottom edge', () => {
		expect(pickPixelCoords(0, 50, 1, 100, 50)).toBeNull();
	});

	it('rejects negative CSS x', () => {
		expect(pickPixelCoords(-1, 0, 1, 100, 50)).toBeNull();
	});

	it('rejects negative CSS y', () => {
		expect(pickPixelCoords(0, -1, 1, 100, 50)).toBeNull();
	});

	it('accepts the last valid pixel (width-1, height-1) without an off-by-one', () => {
		expect(pickPixelCoords(99, 0, 1, 100, 50)).toEqual({ x: 99, y: 49 });
		expect(pickPixelCoords(0, 49, 1, 100, 50)).toEqual({ x: 0, y: 0 });
	});
});

describe('GPU pick base upload across groups (C1)', () => {
	// scene.overrideMaterial shares one ShaderMaterial instance across every
	// InstancedMesh three draws; three only re-uploads a ShaderMaterial's
	// uniforms when the program or material identity changes. Without
	// uniformsNeedUpdate, only the first drawn group's uPickBase would ever
	// reach the GPU and every later group would rasterize with that base.
	it("uploads each group's own pickBase, not just the first", () => {
		const layer = createStructureLayer({ id: 'test-pick-base' });
		const stubMap = { getCenter: () => ({ lng: 0, lat: 0 }), triggerRepaint: () => {} };
		layer.attachMapForTest(stubMap as unknown as Parameters<typeof layer.attachMapForTest>[0]);

		const footprints: Record<string, Footprint> = {
			PickTestBucketA: { sx: 100, sy: 100, sz: 100, ox: 0, oy: 0, oz: 0, typeA: 'Foundation' },
			PickTestBucketB: { sx: 200, sy: 150, sz: 80, ox: 0, oy: 0, oz: 0, typeA: 'Furniture' }
		};
		const structures = [
			base({ instance_id: 'a0', map_object_id: 'PickTestBucketA' }),
			base({ instance_id: 'a1', map_object_id: 'PickTestBucketA' }),
			base({ instance_id: 'b0', map_object_id: 'PickTestBucketB' }),
			base({ instance_id: 'b1', map_object_id: 'PickTestBucketB' })
		];

		layer.update(structures, footprints, 'MainMap', 1);

		const groups = layer.groupsForTest();
		expect(groups.length).toBe(2);

		for (const group of groups) {
			const fakeMaterial = { uniforms: { uPickBase: { value: -1 } } };
			group.mesh.onBeforeRender(
				null as any,
				null as any,
				null as any,
				null as any,
				fakeMaterial as unknown as THREE.Material,
				null as any
			);

			expect(fakeMaterial.uniforms.uPickBase.value).toBe(group.pickBase);
			expect((fakeMaterial as unknown as THREE.ShaderMaterial).uniformsNeedUpdate).toBe(true);

			for (let i = 0; i < group.keys.length; i++) {
				expect(layer.keyAtForTest(group.pickBase + i)).toBe(group.keys[i]);
			}
		}
	});
});

describe('opacity bucketing', () => {
	beforeEach(() => {
		resetMapColors();
	});

	afterEach(() => {
		resetMapColors();
	});

	it('splits a transparent material into its own group even at an identical colour', () => {
		// Same tint + same typeA => identical colorHex, so opacity is the only
		// thing that can separate these two into distinct instanced groups.
		setMaterialTint('Glass', '#abcdef');
		setMaterialTint('Stone', '#abcdef');

		const layer = createStructureLayer({ id: 'test-opacity-bucketing' });
		const stubMap = { getCenter: () => ({ lng: 0, lat: 0 }), triggerRepaint: () => {} };
		layer.attachMapForTest(stubMap as unknown as Parameters<typeof layer.attachMapForTest>[0]);

		const dims = { sx: 100, sy: 100, sz: 100, ox: 0, oy: 0, oz: 0, typeA: 'Foundation' };
		const footprints: Record<string, Footprint> = {
			OpacityGlass: { ...dims, material: 'Glass' },
			OpacityStone: { ...dims, material: 'Stone' }
		};
		const structures = [
			base({ instance_id: 'g0', map_object_id: 'OpacityGlass' }),
			base({ instance_id: 's0', map_object_id: 'OpacityStone' })
		];

		layer.update(structures, footprints, 'MainMap', 1);

		const groups = layer.groupsForTest();
		expect(groups.length).toBe(2);

		const materials = groups.map((g) => g.mesh.material as THREE.MeshLambertMaterial);
		const transparent = materials.filter((mat) => mat.transparent);
		const opaque = materials.filter((mat) => !mat.transparent);

		expect(transparent.length).toBe(1);
		expect(transparent[0].opacity).toBeCloseTo(0.4, 5);

		expect(opaque.length).toBe(1);
		expect(opaque[0].opacity).toBe(1);
	});

	// Sharing MapLibre's depth buffer means a transparent group that skips
	// depthWrite leaves pixels at the cleared far value, and MapLibre's later
	// depth-tested passes paint straight over the glass. Opaque groups are
	// unaffected because their own depth writes reject those fragments.
	it('writes depth for transparent groups so later passes cannot erase them', () => {
		setMaterialTint('Glass', '#abcdef');
		setMaterialTint('Stone', '#abcdef');

		const layer = createStructureLayer({ id: 'test-glass-depthwrite' });
		const stubMap = { getCenter: () => ({ lng: 0, lat: 0 }), triggerRepaint: () => {} };
		layer.attachMapForTest(stubMap as unknown as Parameters<typeof layer.attachMapForTest>[0]);

		const dims = { sx: 100, sy: 100, sz: 100, ox: 0, oy: 0, oz: 0, typeA: 'Foundation' };
		const footprints: Record<string, Footprint> = {
			DepthGlass: { ...dims, material: 'Glass' },
			DepthStone: { ...dims, material: 'Stone' }
		};

		layer.update(
			[
				base({ instance_id: 'dg0', map_object_id: 'DepthGlass' }),
				base({ instance_id: 'ds0', map_object_id: 'DepthStone' })
			],
			footprints,
			'MainMap',
			1
		);

		const materials = layer
			.groupsForTest()
			.map((g) => g.mesh.material as THREE.MeshLambertMaterial);

		expect(materials.length).toBe(2);
		for (const mat of materials) {
			expect(mat.depthWrite).toBe(true);
		}
	});

	it('keeps same-opacity structures in one group', () => {
		setMaterialTint('Wood', '#abcdef');
		setMaterialTint('Stone', '#abcdef');

		const layer = createStructureLayer({ id: 'test-opacity-shared' });
		const stubMap = { getCenter: () => ({ lng: 0, lat: 0 }), triggerRepaint: () => {} };
		layer.attachMapForTest(stubMap as unknown as Parameters<typeof layer.attachMapForTest>[0]);

		const dims = { sx: 100, sy: 100, sz: 100, ox: 0, oy: 0, oz: 0, typeA: 'Foundation' };
		const footprints: Record<string, Footprint> = {
			SharedWood: { ...dims, material: 'Wood' },
			SharedStone: { ...dims, material: 'Stone' }
		};
		const structures = [
			base({ instance_id: 'w0', map_object_id: 'SharedWood' }),
			base({ instance_id: 't0', map_object_id: 'SharedStone' })
		];

		layer.update(structures, footprints, 'MainMap', 1);

		expect(layer.groupsForTest().length).toBe(1);
	});
});

describe('texturedGroupMaterial', () => {
	it('is opaque with opacity 1 for a single-material bundle', () => {
		const material = new THREE.MeshStandardMaterial();
		const bundle: TexturedMeshBundle = { geometry: new THREE.BoxGeometry(1, 1, 1), material };
		const resolved = texturedGroupMaterial(bundle, 1) as THREE.MeshStandardMaterial;

		expect(resolved.transparent).toBe(false);
		expect(resolved.opacity).toBe(1);
	});

	it('marks the material transparent and carries the opacity value below 1', () => {
		const material = new THREE.MeshStandardMaterial();
		const bundle: TexturedMeshBundle = { geometry: new THREE.BoxGeometry(1, 1, 1), material };
		const resolved = texturedGroupMaterial(bundle, 0.4) as THREE.MeshStandardMaterial;

		expect(resolved.transparent).toBe(true);
		expect(resolved.opacity).toBeCloseTo(0.4, 5);
	});

	it('clones rather than mutating the cached bundle material, so one opacity cannot bleed into another group', () => {
		const material = new THREE.MeshStandardMaterial({ opacity: 1, transparent: false });
		const bundle: TexturedMeshBundle = { geometry: new THREE.BoxGeometry(1, 1, 1), material };

		const resolved = texturedGroupMaterial(bundle, 0.4) as THREE.MeshStandardMaterial;

		expect(resolved).not.toBe(material);
		expect(material.opacity).toBe(1);
		expect(material.transparent).toBe(false);
	});

	it('resolves every material in a multi-material bundle, preserving array order', () => {
		const matA = new THREE.MeshStandardMaterial();
		const matB = new THREE.MeshStandardMaterial();
		const bundle: TexturedMeshBundle = {
			geometry: new THREE.BoxGeometry(1, 1, 1),
			material: [matA, matB]
		};

		const resolved = texturedGroupMaterial(bundle, 0.5) as THREE.Material[];

		expect(Array.isArray(resolved)).toBe(true);
		expect(resolved).toHaveLength(2);
		expect(resolved[0]).not.toBe(matA);
		expect(resolved[1]).not.toBe(matB);
		for (const m of resolved as THREE.MeshStandardMaterial[]) {
			expect(m.transparent).toBe(true);
			expect(m.opacity).toBeCloseTo(0.5, 5);
		}
	});
});

// Exercises the mode switch inside update() through the real caches, not just
// the pure material-resolution helper above.
describe('textured mode wiring', () => {
	// Both caches are keyed by mesh name at module scope and shared across this
	// file, so each case below uses its own single-part manifest id to keep loads
	// from resolving out of a previous test's cache.
	const MANIFEST = manifest as unknown as Record<string, { parts: { mesh: string }[] }>;
	const meshNameFor = (structureId: string) => MANIFEST[structureId].parts[0].mesh;

	// GLTFLoader always parses a glTF material into a MeshStandardMaterial, never
	// the bare MeshBasicMaterial default, so giving each synthetic mesh one
	// explicitly is what makes the assertions below meaningful.
	function sceneWithBoxes(count: number): THREE.Object3D {
		const group = new THREE.Group();
		for (let i = 0; i < count; i++) {
			group.add(new THREE.Mesh(new THREE.BoxGeometry(1, 1, 1), new THREE.MeshStandardMaterial()));
		}
		return group;
	}

	function lastCallFor(name: string) {
		const call = [...loadCalls].reverse().find((c) => c.url.includes(name));
		if (!call) throw new Error(`no load() call recorded for ${name}`);
		return call;
	}

	function makeLayer(idSuffix: string) {
		const layer = createStructureLayer({ id: `test-textured-${idSuffix}` });
		const stubMap = { getCenter: () => ({ lng: 0, lat: 0 }), triggerRepaint: () => {} };
		layer.attachMapForTest(stubMap as unknown as Parameters<typeof layer.attachMapForTest>[0]);
		return layer;
	}

	function oneStructure(structureId: string) {
		return [base({ instance_id: `${structureId}0`, map_object_id: structureId })];
	}

	beforeEach(() => {
		resetMapColors();
	});

	afterEach(() => {
		resetMapColors();
	});

	it('renders the flat-colour MeshLambertMaterial when textured mode is off, unchanged from today', () => {
		const structureId = 'BlastFurnace';
		const meshName = meshNameFor(structureId);
		const layer = makeLayer('off');
		layer.update(oneStructure(structureId), {}, 'MainMap', 1);
		lastCallFor(meshName).onLoad({ scene: sceneWithBoxes(1) });
		layer.update(oneStructure(structureId), {}, 'MainMap', 1);

		const groups = layer.groupsForTest();
		expect(groups.length).toBe(1);
		expect(groups[0].mesh.material).toBeInstanceOf(THREE.MeshLambertMaterial);
	});

	// The other tests here cannot guard the default's *value*: with the bundle
	// still pending, requestTexturedMesh returns null and the colour path runs
	// whichever way the default reads. This one resolves the bundle first, then
	// calls update() with `textured` omitted, so only a `false` default keeps it
	// on the colour path. Flipping it to `true` was confirmed to fail here.
	it('defaults to the flat-colour material even once a textured bundle is already cached and ready', async () => {
		const structureId = 'CookingStove';
		const meshName = meshNameFor(structureId);
		const layer = makeLayer('default-guard');
		layer.update(oneStructure(structureId), {}, 'MainMap', 1);
		lastCallFor(meshName).onLoad({ scene: sceneWithBoxes(1) }); // colour-mode geometry ready
		layer.update(oneStructure(structureId), {}, 'MainMap', 1, true); // textured on; drives the load
		lastCallFor(meshName).onLoad({ scene: sceneWithBoxes(1) }); // textured bundle settles
		await Promise.resolve(); // flush the settle-triggered rebuild

		// Confirms the bundle is genuinely resolved before the real assertion below;
		// otherwise this would pass for the wrong reason.
		expect(layer.groupsForTest()[0].mesh.material).toBeInstanceOf(THREE.MeshStandardMaterial);

		layer.update(oneStructure(structureId), {}, 'MainMap', 1); // textured omitted -> default

		expect(layer.groupsForTest()[0].mesh.material).toBeInstanceOf(THREE.MeshLambertMaterial);
	});

	it('falls back to the flat-colour material while the textured glb is still loading, then switches once it settles', async () => {
		const structureId = 'BlastFurnace2';
		const meshName = meshNameFor(structureId);
		const layer = makeLayer('pending');
		layer.update(oneStructure(structureId), {}, 'MainMap', 1);
		lastCallFor(meshName).onLoad({ scene: sceneWithBoxes(1) }); // colour-mode geometry ready
		layer.update(oneStructure(structureId), {}, 'MainMap', 1, true); // textured on; its own bundle not loaded yet

		let groups = layer.groupsForTest();
		expect(groups.length).toBe(1);
		expect(groups[0].mesh.material).toBeInstanceOf(THREE.MeshLambertMaterial);

		// requestMesh's own call already settled and is served from cache, so the
		// most recent load() is requestTexturedMesh's; settling it should requeue a
		// rebuild via onTexturedMeshLoaded.
		lastCallFor(meshName).onLoad({ scene: sceneWithBoxes(1) });
		await Promise.resolve();

		groups = layer.groupsForTest();
		expect(groups.length).toBe(1);
		expect(groups[0].mesh.material).not.toBeInstanceOf(THREE.MeshLambertMaterial);
		expect(groups[0].mesh.material).toBeInstanceOf(THREE.MeshStandardMaterial);
	});

	it('keeps a multi-material glb as a material array once textured, matching InstancedMesh geometry groups', async () => {
		const structureId = 'BlastFurnace3';
		const meshName = meshNameFor(structureId);
		const layer = makeLayer('multimat');
		layer.update(oneStructure(structureId), {}, 'MainMap', 1);
		lastCallFor(meshName).onLoad({ scene: sceneWithBoxes(1) });
		layer.update(oneStructure(structureId), {}, 'MainMap', 1, true);

		lastCallFor(meshName).onLoad({ scene: sceneWithBoxes(2) });
		await Promise.resolve();

		const groups = layer.groupsForTest();
		expect(Array.isArray(groups[0].mesh.material)).toBe(true);
		expect((groups[0].mesh.material as THREE.Material[]).length).toBe(2);
	});

	it('applies opacity to the textured material the same way the colour path does', async () => {
		const structureId = 'BlastFurnace4';
		const meshName = meshNameFor(structureId);
		setMaterialOpacity('Stone', 0.4);
		const footprints: Record<string, Footprint> = {
			[structureId]: { sx: 100, sy: 100, sz: 100, ox: 0, oy: 0, oz: 0, typeA: 'Foundation', material: 'Stone' }
		};

		const layer = makeLayer('opacity');
		layer.update(oneStructure(structureId), footprints, 'MainMap', 1);
		lastCallFor(meshName).onLoad({ scene: sceneWithBoxes(1) });
		layer.update(oneStructure(structureId), footprints, 'MainMap', 1, true);
		lastCallFor(meshName).onLoad({ scene: sceneWithBoxes(1) });
		await Promise.resolve();

		const material = layer.groupsForTest()[0].mesh.material as THREE.MeshStandardMaterial;
		expect(material.transparent).toBe(true);
		expect(material.opacity).toBeCloseTo(0.4, 5);
	});
});

// The point of setVerticalScale: a camera-driven scale change, which happens on
// every move event, must not pay update()'s full group/material rebuild.
describe('setVerticalScale (camera-only compose)', () => {
	function makeVScaleLayer(idSuffix: string) {
		const layer = createStructureLayer({ id: `test-vscale-${idSuffix}` });
		const stubMap = { getCenter: () => ({ lng: 0, lat: 0 }), triggerRepaint: () => {} };
		layer.attachMapForTest(stubMap as unknown as Parameters<typeof layer.attachMapForTest>[0]);
		return layer;
	}

	it('changes instance matrices without changing group count or material identity', () => {
		const layer = makeVScaleLayer('identity');
		const footprints: Record<string, Footprint> = {
			VScaleBox: { sx: 100, sy: 100, sz: 100, ox: 0, oy: 0, oz: 0, typeA: 'Foundation' }
		};
		const structures = [
			base({ instance_id: 'v0', map_object_id: 'VScaleBox', z: 1000 }),
			base({ instance_id: 'v1', map_object_id: 'VScaleBox', z: 2000 })
		];

		layer.update(structures, footprints, 'MainMap', 1);

		const groupsBefore = layer.groupsForTest();
		expect(groupsBefore.length).toBe(1);
		const meshBefore = groupsBefore[0].mesh;
		const materialBefore = meshBefore.material;
		const matrixBefore = new THREE.Matrix4();
		meshBefore.getMatrixAt(0, matrixBefore);
		const versionBefore = meshBefore.instanceMatrix.version;

		layer.setVerticalScale(3);

		const groupsAfter = layer.groupsForTest();
		expect(groupsAfter.length).toBe(1);
		expect(groupsAfter[0].mesh).toBe(meshBefore);
		expect(groupsAfter[0].mesh.material).toBe(materialBefore);

		const matrixAfter = new THREE.Matrix4();
		meshBefore.getMatrixAt(0, matrixAfter);
		expect(matrixAfter.elements).not.toEqual(matrixBefore.elements);
		// needsUpdate is write-only on BufferAttribute, so the version increment is
		// what proves it was set.
		expect(meshBefore.instanceMatrix.version).toBeGreaterThan(versionBefore);
	});

	it('does not increase update() call count when only verticalScale changes', () => {
		const layer = makeVScaleLayer('no-rebuild');
		const footprints: Record<string, Footprint> = {
			VScaleBox2: { sx: 100, sy: 100, sz: 100, ox: 0, oy: 0, oz: 0, typeA: 'Foundation' }
		};
		layer.update([base({ instance_id: 'v0', map_object_id: 'VScaleBox2' })], footprints, 'MainMap', 1);

		const updateSpy = vi.spyOn(layer, 'update');
		layer.setVerticalScale(2);
		layer.setVerticalScale(3);

		expect(updateSpy).not.toHaveBeenCalled();
	});
});

// Everything above exercises only the proxy path or asserts material type, so
// the mesh path's partLocalMatrix post-multiply has no coverage of its own: a
// reversed multiplication order or an index drift would misplace every
// multi-part structure while the rest of this file stayed green. Wooden_DoorWall
// is a real two-part entry with a non-identity part offset, pinned here against
// meshInstanceMatrix both after update() and after setVerticalScale().
describe('mesh-path part-local transform (multi-part structure)', () => {
	type RealManifestPart = MeshPart & { mesh: string };
	const MANIFEST = manifest as unknown as Record<string, { parts: RealManifestPart[] }>;

	// Baked values round-trip through a Float32Array, so this needs a relative
	// tolerance floored for near-zero elements, not a flat epsilon.
	function expectMatrixClose(got: THREE.Matrix4, want: THREE.Matrix4) {
		const ge = got.elements;
		const we = want.elements;
		for (let i = 0; i < 16; i++) {
			expect(Math.abs(ge[i] - we[i])).toBeLessThanOrEqual(Math.max(1e-9, Math.abs(we[i]) * 1e-6));
		}
	}

	beforeEach(() => {
		resetMapColors();
	});

	afterEach(() => {
		resetMapColors();
	});

	it('matches meshInstanceMatrix right after update() and again after setVerticalScale() at a different scale', () => {
		const structureId = 'Wooden_DoorWall';
		const part = MANIFEST[structureId].parts[0];
		expect(part.loc).not.toEqual([0, 0, 0]); // guards against a manifest edit silently degrading this to identity

		const layer = createStructureLayer({ id: 'test-part-matrix' });
		const stubMap = { getCenter: () => ({ lng: 0, lat: 0 }), triggerRepaint: () => {} };
		layer.attachMapForTest(stubMap as unknown as Parameters<typeof layer.attachMapForTest>[0]);

		const s = base({
			instance_id: 'door0',
			map_object_id: structureId,
			x: 12345,
			y: -6789,
			z: 250,
			yaw: 0.7
		});

		layer.update([s], {}, 'MainMap', 1);
		for (const p of MANIFEST[structureId].parts) {
			const call = [...loadCalls].reverse().find((c) => c.url.includes(p.mesh));
			if (!call) throw new Error(`no load() call recorded for ${p.mesh}`);
			call.onLoad({ scene: new THREE.Mesh(new THREE.BoxGeometry(1, 1, 1), new THREE.MeshStandardMaterial()) });
		}
		layer.update([s], {}, 'MainMap', 1);

		const partGeom = requestMesh(part.mesh);
		expect(partGeom).not.toBeNull();
		const group = layer.groupsForTest().find((g) => g.mesh.geometry === partGeom);
		expect(group).toBeDefined();
		const instanceIndex = group!.keys.indexOf(s.instance_id);
		expect(instanceIndex).toBeGreaterThanOrEqual(0);

		const merc = MercatorCoordinate.fromLngLat([0, 0], 0);
		const mPerUnit = merc.meterInMercatorCoordinateUnits();

		const got = new THREE.Matrix4();
		group!.mesh.getMatrixAt(instanceIndex, got);
		expectMatrixClose(got, meshInstanceMatrix(s, part, 'MainMap', 1, 1 * mPerUnit));

		layer.setVerticalScale(3.7);
		group!.mesh.getMatrixAt(instanceIndex, got);
		expectMatrixClose(got, meshInstanceMatrix(s, part, 'MainMap', 3.7, 3.7 * mPerUnit));
	});
});
