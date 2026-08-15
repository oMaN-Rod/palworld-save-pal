import { describe, it, expect, vi } from 'vitest';
import * as THREE from 'three';

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
	structureParts,
	requestMesh,
	meshFailed,
	onMeshLoaded,
	requestTexturedMesh,
	texturedMeshFailed,
	onTexturedMeshLoaded,
	bundleMapObjectMesh,
	configureTexturedMaterial
} from './meshLibrary';
import manifest from '../../../../../data/json/structure_meshes.json';

function sceneWithMeshes(count: number): THREE.Object3D {
	const group = new THREE.Group();
	for (let i = 0; i < count; i++) {
		group.add(new THREE.Mesh(new THREE.BoxGeometry(1, 1, 1)));
	}
	return group;
}

function sceneWithMismatchedMeshes(): THREE.Object3D {
	const group = new THREE.Group();
	group.add(new THREE.Mesh(new THREE.BoxGeometry(1, 1, 1)));

	const bare = new THREE.BufferGeometry();
	bare.setAttribute('position', new THREE.Float32BufferAttribute([0, 0, 0, 1, 0, 0, 0, 1, 0], 3));
	group.add(new THREE.Mesh(bare));

	return group;
}

function lastCallFor(name: string) {
	const call = [...loadCalls].reverse().find((c) => c.url.includes(name));
	if (!call) throw new Error(`no load() call recorded for ${name}`);
	return call;
}

describe('structureParts', () => {
	it('returns null for an unknown id', () => {
		expect(structureParts('__definitely_not_a_real_id__')).toBeNull();
	});

	it('returns manifest-backed parts for a known id', () => {
		const entries = manifest as unknown as Record<string, { parts: { mesh: string }[] }>;
		const id = Object.keys(entries)[0];
		const parts = structureParts(id);
		expect(Array.isArray(parts)).toBe(true);
		expect(parts!.length).toBeGreaterThan(0);
		expect(parts![0].mesh).toBe(entries[id].parts[0].mesh);
	});

	// Saves spell some ids with different casing than the data table row key,
	// e.g. "Stone_Foundation" for the row "Stone_foundation".
	it('falls back to a case-insensitive match', () => {
		const entries = manifest as unknown as Record<string, { parts: { mesh: string }[] }>;
		const id = Object.keys(entries).find((k) => k.toLowerCase() !== k)!;
		expect(structureParts(id.toLowerCase())).toEqual(entries[id].parts);
		expect(structureParts(id.toUpperCase())).toEqual(entries[id].parts);
	});

	it('resolves the real Stone_Foundation casing seen in saves', () => {
		expect(structureParts('Stone_Foundation')).not.toBeNull();
	});
});

describe('requestMesh success path', () => {
	it('returns null while loading, then cached geometry once the load lands', () => {
		const name = 'RequestMesh_Success';
		expect(requestMesh(name)).toBeNull();

		lastCallFor(name).onLoad({ scene: sceneWithMeshes(1) });

		expect(meshFailed(name)).toBe(false);
		expect(requestMesh(name)).toBeInstanceOf(THREE.BufferGeometry);
	});
});

describe('requestMesh cm contract', () => {
	it('scales source geometry (metres, per the glTF exporter) up 100x to UE centimetres', () => {
		const name = 'RequestMesh_CmContract';
		expect(requestMesh(name)).toBeNull();

		lastCallFor(name).onLoad({ scene: sceneWithMeshes(1) });

		const geo = requestMesh(name)!;
		geo.computeBoundingBox();
		const size = new THREE.Vector3();
		geo.boundingBox!.getSize(size);
		const source = new THREE.BoxGeometry(1, 1, 1);
		source.computeBoundingBox();
		const sourceSize = new THREE.Vector3();
		source.boundingBox!.getSize(sourceSize);
		expect(size.x).toBeCloseTo(sourceSize.x * 100, 6);
		expect(size.y).toBeCloseTo(sourceSize.y * 100, 6);
		expect(size.z).toBeCloseTo(sourceSize.z * 100, 6);
	});
});

describe('requestMesh multi-mesh glb', () => {
	it('merges every mesh in the glb into one geometry, not just the first', () => {
		const name = 'RequestMesh_MultiMesh';
		const singleMeshVertexCount = new THREE.BoxGeometry(1, 1, 1).attributes.position.count;
		expect(requestMesh(name)).toBeNull();

		lastCallFor(name).onLoad({ scene: sceneWithMeshes(2) });

		const geo = requestMesh(name);
		expect(geo).toBeInstanceOf(THREE.BufferGeometry);
		expect(geo!.attributes.position.count).toBe(singleMeshVertexCount * 2);
	});

	it('falls back to shared attributes when meshes have mismatched attribute sets', () => {
		const name = 'RequestMesh_MismatchedAttrs';
		requestMesh(name);

		lastCallFor(name).onLoad({ scene: sceneWithMismatchedMeshes() });

		expect(meshFailed(name)).toBe(false);
		const geo = requestMesh(name);
		expect(geo).toBeInstanceOf(THREE.BufferGeometry);
		expect(geo!.attributes.uv).toBeUndefined();
		expect(geo!.attributes.normal).toBeUndefined();
		expect(geo!.attributes.position.count).toBe(36 + 3);
	});
});

describe('requestMesh dir override', () => {
	it('loads from the given directory instead of the default structures path', () => {
		const name = 'RequestMesh_SceneryDir';
		expect(requestMesh(name, '/models/scenery')).toBeNull();

		expect(lastCallFor(name).url).toBe(`/models/scenery/${name}.glb`);
	});
});

describe('onMeshLoaded unsubscribe', () => {
	it('stops invoking the callback once unsubscribed', () => {
		const name = 'RequestMesh_Unsubscribe';
		const cb = vi.fn();
		const unsubscribe = onMeshLoaded(cb);

		requestMesh(name);
		lastCallFor(name).onError(new Error('boom'));
		expect(cb).toHaveBeenCalledTimes(1);

		unsubscribe();

		const name2 = 'RequestMesh_Unsubscribe_After';
		requestMesh(name2);
		lastCallFor(name2).onError(new Error('boom again'));
		expect(cb).toHaveBeenCalledTimes(1);
	});
});

describe('requestMesh failure path', () => {
	it('reports a permanently-failed mesh via meshFailed, not as still loading', () => {
		const name = 'RequestMesh_404';
		expect(requestMesh(name)).toBeNull();
		expect(meshFailed(name)).toBe(false);

		lastCallFor(name).onError(new Error('404'));

		expect(meshFailed(name)).toBe(true);
		expect(requestMesh(name)).toBeNull();
	});

	it('never re-requests a mesh once it has permanently failed', () => {
		const name = 'RequestMesh_NoRetry';
		requestMesh(name);
		lastCallFor(name).onError(new Error('404'));

		const callsBefore = loadCalls.length;
		requestMesh(name);
		requestMesh(name);
		requestMesh(name);

		expect(loadCalls.length).toBe(callsBefore);
	});

	it('notifies listeners when a load permanently fails, not just on success', () => {
		const name = 'RequestMesh_NotifyOnFail';
		const cb = vi.fn();
		onMeshLoaded(cb);

		requestMesh(name);
		lastCallFor(name).onError(new Error('boom'));

		expect(cb).toHaveBeenCalled();
	});

	it('treats a load with zero extractable meshes as a permanent failure rather than retrying forever', () => {
		const name = 'RequestMesh_EmptyScene';
		requestMesh(name);
		lastCallFor(name).onLoad({ scene: sceneWithMeshes(0) });

		expect(meshFailed(name)).toBe(true);
		expect(requestMesh(name)).toBeNull();

		const callsBefore = loadCalls.length;
		requestMesh(name);
		expect(loadCalls.length).toBe(callsBefore);
	});
});

// The cache is shared by structures (a few dozen meshes) and scenery (hundreds).
// Notifying every listener regardless of directory meant each scenery mesh woke
// the structure layer into a full rebuild of every base: 79 of 82 rebuilds during
// one real base load came from meshes it never draws.
describe('onMeshLoaded directory scoping', () => {
	const STRUCTURES = '/models/structures';
	const SCENERY = '/models/scenery';

	it('notifies a listener when a mesh from its own directory settles', () => {
		let hits = 0;
		const off = onMeshLoaded(() => hits++, STRUCTURES);
		const name = 'Scoped_OwnDir';

		requestMesh(name, STRUCTURES);
		lastCallFor(name).onLoad({ scene: sceneWithMeshes(1) });

		off();
		expect(hits).toBe(1);
	});

	it('does not notify a listener when a mesh from another directory settles', () => {
		let structureHits = 0;
		let sceneryHits = 0;
		const offStructures = onMeshLoaded(() => structureHits++, STRUCTURES);
		const offScenery = onMeshLoaded(() => sceneryHits++, SCENERY);
		const name = 'Scoped_OtherDir';

		requestMesh(name, SCENERY);
		lastCallFor(name).onLoad({ scene: sceneWithMeshes(1) });

		offStructures();
		offScenery();
		expect(sceneryHits).toBe(1);
		expect(structureHits).toBe(0);
	});

	it('scopes permanent failures the same way as successes', () => {
		let structureHits = 0;
		let sceneryHits = 0;
		const offStructures = onMeshLoaded(() => structureHits++, STRUCTURES);
		const offScenery = onMeshLoaded(() => sceneryHits++, SCENERY);
		const name = 'Scoped_FailureDir';

		requestMesh(name, SCENERY);
		lastCallFor(name).onError(new Error('nope'));

		offStructures();
		offScenery();
		expect(meshFailed(name)).toBe(true);
		expect(sceneryHits).toBe(1);
		expect(structureHits).toBe(0);
	});

	it('notifies an unscoped listener for every directory', () => {
		let hits = 0;
		const off = onMeshLoaded(() => hits++);
		const a = 'Scoped_UnscopedA';
		const b = 'Scoped_UnscopedB';

		requestMesh(a, STRUCTURES);
		lastCallFor(a).onLoad({ scene: sceneWithMeshes(1) });
		requestMesh(b, SCENERY);
		lastCallFor(b).onLoad({ scene: sceneWithMeshes(1) });

		off();
		expect(hits).toBe(2);
	});
});

// requestTexturedMesh is a separate cache that keeps per-primitive materials
// instead of merging them away; these mirror the plain suite's coverage.
describe('requestTexturedMesh', () => {
	it('returns null while loading, then a cached bundle once the load lands', () => {
		const name = 'RequestTexturedMesh_Success';
		expect(requestTexturedMesh(name)).toBeNull();

		lastCallFor(name).onLoad({ scene: sceneWithMeshes(1) });

		expect(texturedMeshFailed(name)).toBe(false);
		const bundle = requestTexturedMesh(name);
		expect(bundle?.geometry).toBeInstanceOf(THREE.BufferGeometry);
		expect(Array.isArray(bundle?.material)).toBe(false);
	});

	it('scales geometry (metres, per the glTF exporter) up 100x to UE centimetres, same as requestMesh', () => {
		const name = 'RequestTexturedMesh_CmContract';
		requestTexturedMesh(name);
		lastCallFor(name).onLoad({ scene: sceneWithMeshes(1) });

		const geo = requestTexturedMesh(name)!.geometry;
		geo.computeBoundingBox();
		const size = new THREE.Vector3();
		geo.boundingBox!.getSize(size);
		const source = new THREE.BoxGeometry(1, 1, 1);
		source.computeBoundingBox();
		const sourceSize = new THREE.Vector3();
		source.boundingBox!.getSize(sourceSize);
		expect(size.x).toBeCloseTo(sourceSize.x * 100, 6);
		expect(size.y).toBeCloseTo(sourceSize.y * 100, 6);
		expect(size.z).toBeCloseTo(sourceSize.z * 100, 6);
	});

	// Unlike requestMesh, a multi-primitive glb keeps each primitive's material
	// via geometry.groups instead of merging into one untextured blob.
	it('bundles a multi-primitive glb with one geometry group per primitive material', () => {
		const name = 'RequestTexturedMesh_MultiMaterial';
		requestTexturedMesh(name);
		lastCallFor(name).onLoad({ scene: sceneWithMeshes(2) });

		const bundle = requestTexturedMesh(name)!;
		expect(Array.isArray(bundle.material)).toBe(true);
		expect((bundle.material as THREE.Material[]).length).toBe(2);
		expect(bundle.geometry.groups).toHaveLength(2);
		expect(bundle.geometry.groups[0].materialIndex).toBe(0);
		expect(bundle.geometry.groups[1].materialIndex).toBe(1);
	});

	it('reports a permanently-failed glb via texturedMeshFailed, not as still loading', () => {
		const name = 'RequestTexturedMesh_404';
		expect(requestTexturedMesh(name)).toBeNull();
		expect(texturedMeshFailed(name)).toBe(false);

		lastCallFor(name).onError(new Error('404'));

		expect(texturedMeshFailed(name)).toBe(true);
		expect(requestTexturedMesh(name)).toBeNull();
	});

	it('never re-requests a glb once it has permanently failed', () => {
		const name = 'RequestTexturedMesh_NoRetry';
		requestTexturedMesh(name);
		lastCallFor(name).onError(new Error('404'));

		const callsBefore = loadCalls.length;
		requestTexturedMesh(name);
		requestTexturedMesh(name);

		expect(loadCalls.length).toBe(callsBefore);
	});

	it('notifies onTexturedMeshLoaded listeners on both success and failure', () => {
		const successName = 'RequestTexturedMesh_NotifySuccess';
		const failName = 'RequestTexturedMesh_NotifyFail';
		const cb = vi.fn();
		const off = onTexturedMeshLoaded(cb);

		requestTexturedMesh(successName);
		lastCallFor(successName).onLoad({ scene: sceneWithMeshes(1) });
		requestTexturedMesh(failName);
		lastCallFor(failName).onError(new Error('boom'));

		off();
		expect(cb).toHaveBeenCalledTimes(2);
	});

	it('loads from the given directory instead of the default structures path', () => {
		const name = 'RequestTexturedMesh_SceneryDir';
		expect(requestTexturedMesh(name, '/models/scenery')).toBeNull();
		expect(lastCallFor(name).url).toBe(`/models/scenery/${name}.glb`);
	});

	it('keeps its own cache independent of requestMesh, so both can hold the same glb at once', () => {
		const name = 'RequestTexturedMesh_IndependentCache';
		expect(requestMesh(name)).toBeNull();
		lastCallFor(name).onLoad({ scene: sceneWithMeshes(1) });
		expect(requestMesh(name)).toBeInstanceOf(THREE.BufferGeometry);

		// The textured cache has not seen this name yet, so it still reports
		// loading (null) even though the plain geometry cache already resolved it.
		expect(requestTexturedMesh(name)).toBeNull();
		lastCallFor(name).onLoad({ scene: sceneWithMeshes(1) });
		expect(requestTexturedMesh(name)?.geometry).toBeInstanceOf(THREE.BufferGeometry);
	});
});

describe('bundleMapObjectMesh (re-exported for structures)', () => {
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
});

// Shared by palMeshLibrary and mapObjectMeshLibrary so their textured materials
// tune together rather than drifting apart.
describe('configureTexturedMaterial', () => {
	it('zeroes metalness and raises roughness off the glTF fully-metallic default', () => {
		const material = new THREE.MeshStandardMaterial({ metalness: 1, roughness: 1 });
		configureTexturedMaterial(material);
		expect(material.metalness).toBe(0);
		expect(material.roughness).toBe(0.9);
	});

	it('forces DoubleSide so the mercator winding flip does not cull the visible face', () => {
		const material = new THREE.MeshStandardMaterial({ side: THREE.FrontSide });
		configureTexturedMaterial(material);
		expect(material.side).toBe(THREE.DoubleSide);
	});

	it('gives a textured material an emissive floor', () => {
		const map = new THREE.Texture();
		const material = new THREE.MeshStandardMaterial({ map });
		configureTexturedMaterial(material);
		expect(material.emissiveMap).toBe(map);
		expect(material.emissiveIntensity).toBeCloseTo(0.25, 6);
		expect(material.emissive.getHex()).toBe(0xffffff);
	});

	it('leaves an untextured material unlit', () => {
		const material = new THREE.MeshStandardMaterial();
		configureTexturedMaterial(material);
		expect(material.emissiveMap).toBeNull();
		expect(material.emissive.getHex()).toBe(0x000000);
	});
});

describe('configureTexturedMaterial vertex colours', () => {
	it('forces vertexColors off', () => {
		const material = new THREE.MeshStandardMaterial();
		material.vertexColors = true;
		configureTexturedMaterial(material);
		expect(material.vertexColors).toBe(false);
	});

	it('leaves opacity and transparency untouched', () => {
		const material = new THREE.MeshStandardMaterial({ transparent: true, opacity: 0.25 });
		material.vertexColors = true;
		configureTexturedMaterial(material);
		expect(material.transparent).toBe(true);
		expect(material.opacity).toBe(0.25);
	});
});
