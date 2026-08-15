// Loads real scenery GLBs through the unmocked GLTFLoader. The mocked suite is
// exactly why both the meshopt-decoder gap and the in-place-transform bug shipped
// unnoticed: a mock cannot reject an unsupported extension or truncate an integer
// typed array the way a real parser does. Only the network is stubbed here.
import { afterEach, describe, expect, it, vi } from 'vitest';
import { existsSync, readFileSync } from 'node:fs';
import { resolve } from 'node:path';
import * as THREE from 'three';

const SCENERY_DIR = resolve(__dirname, '../../../../static/models/scenery');
const STRUCTURES_DIR = resolve(__dirname, '../../../../static/models/structures');

function samplePath(name: string): string {
	return resolve(SCENERY_DIR, `${name}.glb`);
}

function structureSamplePath(name: string): string {
	return resolve(STRUCTURES_DIR, `${name}.glb`);
}

async function loadRealMesh(name: string) {
	const fileBytes = readFileSync(samplePath(name));
	const arrayBuffer = fileBytes.buffer.slice(
		fileBytes.byteOffset,
		fileBytes.byteOffset + fileBytes.byteLength
	);

	// FileLoader reports progress via a browser-only ProgressEvent. This test only
	// reads the final outcome, so a minimal stand-in suffices.
	(global as { ProgressEvent?: unknown }).ProgressEvent = class {
		constructor(
			public type: string,
			public init?: unknown
		) {}
	};

	global.fetch = vi.fn(async (input: RequestInfo | URL) => {
		const url = input instanceof Request ? input.url : String(input);
		if (!url.includes(`${name}.glb`)) {
			throw new Error(`unexpected fetch in test: ${url}`);
		}
		return new Response(arrayBuffer.slice(0));
	}) as typeof fetch;

	const { requestMesh, meshFailed, onMeshLoaded } = await import('./meshLibrary');

	// FileLoader's `new Request(url)` throws on a bare "/..." path outside a
	// browser; an absolute dir sidesteps that without changing behaviour.
	const dir = 'http://scenery.test/models/scenery';
	expect(requestMesh(name, dir)).toBeNull();

	await new Promise<void>((resolvePromise, rejectPromise) => {
		const timeout = setTimeout(() => rejectPromise(new Error('load did not settle')), 10000);
		const unsubscribe = onMeshLoaded(() => {
			clearTimeout(timeout);
			unsubscribe();
			resolvePromise();
		});
	});

	return { geometry: requestMesh(name, dir), failed: meshFailed(name) };
}

describe('meshLibrary (real gltfpack/meshopt scenery GLBs)', () => {
	const originalFetch = global.fetch;
	const originalProgressEvent = (global as { ProgressEvent?: unknown }).ProgressEvent;

	afterEach(() => {
		global.fetch = originalFetch;
		(global as { ProgressEvent?: unknown }).ProgressEvent = originalProgressEvent;
		vi.resetModules();
	});

	const CUBE = 'Cube_8c062e';

	it.skipIf(!existsSync(samplePath(CUBE)))(
		'requestMesh decodes a real meshopt-compressed scenery mesh into non-empty geometry',
		async () => {
			const { geometry, failed } = await loadRealMesh(CUBE);
			expect(failed).toBe(false);
			expect(geometry).toBeInstanceOf(THREE.BufferGeometry);
			expect(geometry!.attributes.position.count).toBeGreaterThan(0);
		}
	);

	// This GLB's positions are Uint16 with dequantization on the mesh node.
	// requestMesh used to applyMatrix4 straight onto the still-integer attribute,
	// truncating every component into the Uint16 range and blowing the bounding
	// box out to ~65535 on every axis.
	//
	// The expected values come from the GLB's own glTF JSON: POSITION min/max are
	// [0,0,0]/[15711, 11941, 16383] and the node scale is 0.0124018202 uniform
	// with no rotation, so a correct box is exactly (max - min) * scale per axis,
	// then x100 for the centimetre contract. Asserted per axis rather than against
	// one figure: a bug that reshuffles components between axes can land near a
	// single combined number by luck, but not reproduce three differing ones.
	const MOUNTAIN = 'S_FloatingMountain_01_50a1a1';
	const CM_PER_UNIT = 100;
	const EXPECTED_SIZE_CM = {
		x: 15711 * 0.0124018202 * CM_PER_UNIT,
		y: 11941 * 0.0124018202 * CM_PER_UNIT,
		z: 16383 * 0.0124018202 * CM_PER_UNIT
	};
	const TOLERANCE = 0.05;

	it.skipIf(!existsSync(samplePath(MOUNTAIN)))(
		'requestMesh dequantizes KHR_mesh_quantization attributes to their real, per-axis extent',
		async () => {
			const { geometry, failed } = await loadRealMesh(MOUNTAIN);
			expect(failed).toBe(false);
			expect(geometry).toBeInstanceOf(THREE.BufferGeometry);

			geometry!.computeBoundingBox();
			const size = new THREE.Vector3();
			geometry!.boundingBox!.getSize(size);

			for (const axis of ['x', 'y', 'z'] as const) {
				const expected = EXPECTED_SIZE_CM[axis];
				expect(size[axis]).toBeGreaterThan(expected * (1 - TOLERANCE));
				expect(size[axis]).toBeLessThan(expected * (1 + TOLERANCE));
			}
		}
	);
});

// In Node, GLTFLoader falls back to TextureLoader, whose ImageLoader builds its
// image via document.createElementNS. Without a stand-in that throws, GLTFLoader
// swallows it, and `material.map` is never set for anything to observe. Firing
// the 'load' event satisfies it; three builds the Texture from this element
// whether or not any pixel data was decoded.
class FakeImageElement {
	private listeners = new Map<string, Set<() => void>>();
	private _src = '';
	crossOrigin?: string;
	addEventListener(type: string, fn: () => void): void {
		if (!this.listeners.has(type)) this.listeners.set(type, new Set());
		this.listeners.get(type)!.add(fn);
	}
	removeEventListener(type: string, fn: () => void): void {
		this.listeners.get(type)?.delete(fn);
	}
	get src(): string {
		return this._src;
	}
	set src(value: string) {
		this._src = value;
		queueMicrotask(() => {
			for (const fn of this.listeners.get('load') ?? []) fn.call(this);
		});
	}
}

// These newly-baked structure glbs carry webp + quantization + meshopt, not the
// Draco the shared loader also sets up, so DRACOLoader goes unexercised here.
async function loadRealTexturedMesh(name: string) {
	const fileBytes = readFileSync(structureSamplePath(name));
	const arrayBuffer = fileBytes.buffer.slice(
		fileBytes.byteOffset,
		fileBytes.byteOffset + fileBytes.byteLength
	);

	(global as { ProgressEvent?: unknown }).ProgressEvent = class {
		constructor(
			public type: string,
			public init?: unknown
		) {}
	};
	(global as { self?: unknown }).self = globalThis;
	(global as { document?: unknown }).document = {
		createElementNS: (_ns: string, tag: string) => (tag === 'img' ? new FakeImageElement() : null)
	};

	global.fetch = vi.fn(async (input: RequestInfo | URL) => {
		const url = input instanceof Request ? input.url : String(input);
		if (!url.includes(`${name}.glb`)) {
			throw new Error(`unexpected fetch in test: ${url}`);
		}
		return new Response(arrayBuffer.slice(0));
	}) as typeof fetch;

	const { requestTexturedMesh, texturedMeshFailed, onTexturedMeshLoaded } = await import('./meshLibrary');

	const dir = 'http://structures.test/models/structures';
	expect(requestTexturedMesh(name, dir)).toBeNull();

	await new Promise<void>((resolvePromise, rejectPromise) => {
		const timeout = setTimeout(() => rejectPromise(new Error('load did not settle')), 10000);
		const unsubscribe = onTexturedMeshLoaded(() => {
			clearTimeout(timeout);
			unsubscribe();
			resolvePromise();
		});
	});

	return { bundle: requestTexturedMesh(name, dir), failed: texturedMeshFailed(name) };
}

describe('meshLibrary (real textured structure GLBs)', () => {
	const originalFetch = global.fetch;
	const originalProgressEvent = (global as { ProgressEvent?: unknown }).ProgressEvent;
	const originalSelf = (global as { self?: unknown }).self;
	const originalDocument = (global as { document?: unknown }).document;

	afterEach(() => {
		global.fetch = originalFetch;
		(global as { ProgressEvent?: unknown }).ProgressEvent = originalProgressEvent;
		(global as { self?: unknown }).self = originalSelf;
		(global as { document?: unknown }).document = originalDocument;
		vi.resetModules();
	});

	const SINGLE_MATERIAL = 'SM_BlastFurnacePrimitive_cc87d1';

	it.skipIf(!existsSync(structureSamplePath(SINGLE_MATERIAL)))(
		'requestTexturedMesh decodes a real single-material structure glb with its embedded texture',
		async () => {
			const { bundle, failed } = await loadRealTexturedMesh(SINGLE_MATERIAL);
			expect(failed).toBe(false);
			expect(bundle?.geometry).toBeInstanceOf(THREE.BufferGeometry);
			expect(bundle!.geometry.attributes.position.count).toBeGreaterThan(0);
			expect(Array.isArray(bundle?.material)).toBe(false);
			const material = bundle!.material as THREE.MeshStandardMaterial;
			expect(material.map).toBeInstanceOf(THREE.Texture);
		},
		30000
	);

	// Ships 2 primitives, each with its own material and embedded texture -- the
	// case a single shared material would render wrong for half the mesh.
	const MULTI_MATERIAL = 'SM_Capacitor_Large_dbb237';

	it.skipIf(!existsSync(structureSamplePath(MULTI_MATERIAL)))(
		'requestTexturedMesh bundles a real multi-material structure glb with one geometry group per material',
		async () => {
			const { bundle, failed } = await loadRealTexturedMesh(MULTI_MATERIAL);
			expect(failed).toBe(false);
			expect(Array.isArray(bundle?.material)).toBe(true);
			const materials = bundle!.material as THREE.MeshStandardMaterial[];
			expect(materials).toHaveLength(2);
			expect(bundle!.geometry.groups).toHaveLength(2);
			for (const mat of materials) {
				expect(mat.map).toBeInstanceOf(THREE.Texture);
			}
		},
		30000
	);

	// SK_-prefixed structures used to carry a `skins` binding whose JOINTS_0/
	// WEIGHTS_0 attributes the bake had pruned. GLTFLoader decides isSkinnedMesh
	// from the node's `skin` alone, so it crashed in normalizeSkinWeights and 25
	// structures fell back to proxy boxes. Pinned because the symptom was silent:
	// nothing errored, the buildings simply became boxes.
	const SKINNED_NO_WEIGHTS = 'SK_Generator_6f2e11';

	it.skipIf(!existsSync(structureSamplePath(SKINNED_NO_WEIGHTS)))(
		'requestTexturedMesh parses a formerly skinned glb once its orphaned binding is stripped',
		async () => {
			const { bundle, failed } = await loadRealTexturedMesh(SKINNED_NO_WEIGHTS);
			expect(failed).toBe(false);
			expect(bundle?.geometry).toBeInstanceOf(THREE.BufferGeometry);
			expect(bundle!.geometry.attributes.position.count).toBeGreaterThan(0);
		},
		30000
	);
});
