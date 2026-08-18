// Pins requestPalMesh's cache-sharing contract: the same Object3D for every call
// with a given key, never a fresh clone. Mocked here so it stays deterministic
// and independent of whether the real pipeline can parse a given GLB.
import * as THREE from 'three';
import { describe, expect, it, vi } from 'vitest';

const { loadCalls } = vi.hoisted(() => ({
	loadCalls: [] as Array<{
		url: string;
		onLoad: (gltf: { scene: THREE.Object3D }) => void;
		onError: (err: unknown) => void;
	}>
}));

vi.mock('three/examples/jsm/loaders/GLTFLoader.js', () => ({
	GLTFLoader: vi.fn().mockImplementation(() => ({
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

import { requestPalMesh } from './palMeshLibrary';

function lastCallFor(key: string) {
	const call = [...loadCalls].reverse().find((c) => c.url.includes(key));
	if (!call) throw new Error(`no load() call recorded for ${key}`);
	return call;
}

describe('requestPalMesh sharing contract', () => {
	it('returns the exact same Object3D instance on every call once loaded, never a clone', () => {
		const key = 'anubis';
		expect(requestPalMesh(key)).toBeNull();

		const scene = new THREE.Group();
		scene.add(new THREE.Mesh(new THREE.BoxGeometry(1, 1, 1)));
		lastCallFor(key).onLoad({ scene });

		const first = requestPalMesh(key);
		const second = requestPalMesh(key);
		expect(first).toBeInstanceOf(THREE.Object3D);
		expect(second).toBe(first);
	});
});
