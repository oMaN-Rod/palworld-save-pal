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

import { structureParts, requestMesh, meshFailed, onMeshLoaded } from './meshLibrary';
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
