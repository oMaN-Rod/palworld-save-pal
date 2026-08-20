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

import { requestMesh, sweepMeshLibrary } from './meshLibrary';
import { activeMeshUnion, clearActiveMeshes, setActiveMeshes } from './meshUsage';

function settleMesh(name: string): void {
	expect(requestMesh(name)).toBeNull(); // kicks off the load
	const call = [...loadCalls].reverse().find((c) => c.url.includes(name));
	if (!call) throw new Error(`no load() call recorded for ${name}`);
	const scene = new THREE.Group();
	scene.add(new THREE.Mesh(new THREE.BoxGeometry(1, 1, 1)));
	call.onLoad({ scene });
}

describe('sweepMeshLibrary', () => {
	it('disposes an inactive, aged-out entry and reloads it on next request', () => {
		settleMesh('sweep-a');
		const first = requestMesh('sweep-a');
		expect(first).not.toBeNull();
		const dispose = vi.spyOn(first!, 'dispose');

		// Not active, and "now" far past any max age.
		const result = sweepMeshLibrary(new Set(), 60_000, Date.now() + 120_000);
		expect(result.swept).toBe(1);
		expect(dispose).toHaveBeenCalled();
		expect(requestMesh('sweep-a')).toBeNull(); // cache miss -> reloads
	});

	it('an entry pinned by an active set is never swept', () => {
		settleMesh('sweep-pinned');
		expect(requestMesh('sweep-pinned')).not.toBeNull();
		setActiveMeshes('scenery', ['sweep-pinned']);

		const result = sweepMeshLibrary(activeMeshUnion(['scenery']), 60_000, Date.now() + 120_000);
		expect(result.swept).toBe(0);
		expect(requestMesh('sweep-pinned')).not.toBeNull();
		clearActiveMeshes('scenery');
	});

	it('the age guard keeps recently used entries even when inactive', () => {
		settleMesh('sweep-fresh');
		expect(requestMesh('sweep-fresh')).not.toBeNull();

		// maxAge 60s, entry touched "now": age 0 -> kept.
		const result = sweepMeshLibrary(new Set(), 60_000, Date.now());
		expect(result.swept).toBe(0);
		expect(requestMesh('sweep-fresh')).not.toBeNull();
	});
});

describe('activeMeshUnion', () => {
	it('unions scopes and ignores unknown ones', () => {
		setActiveMeshes('structures', ['a', 'b']);
		setActiveMeshes('scenery', ['b', 'c']);
		const union = activeMeshUnion(['structures', 'scenery', 'ghost']);
		expect([...union].sort()).toEqual(['a', 'b', 'c']);
		clearActiveMeshes('structures');
		clearActiveMeshes('scenery');
		expect(activeMeshUnion(['structures', 'scenery']).size).toBe(0);
	});

	it('replacing a scope drops its previous names', () => {
		setActiveMeshes('scenery', ['old']);
		setActiveMeshes('scenery', ['new']);
		expect([...activeMeshUnion(['scenery'])]).toEqual(['new']);
		clearActiveMeshes('scenery');
	});
});
