// Loads real Pal GLBs through the unmocked GLTFLoader; only the network is stubbed,
// so meshopt registration, dequantization and webp texture support run for real.
import { afterEach, describe, expect, it, vi } from 'vitest';
import { existsSync, readFileSync } from 'node:fs';
import { resolve } from 'node:path';
import * as THREE from 'three';
import manifestJson from '../../../../../../../data/json/pal_meshes.json';

type Entry = { file: string; bytes: number; triangles: number };
const MANIFEST = manifestJson as unknown as Record<string, Entry>;

const PALS_DIR = resolve(__dirname, '../../../../../../static/models/pals');

const KEY = 'anubis';
const FILE = MANIFEST[KEY]?.file;

const UNTEXTURED_KEY = 'blackpuppy';
const UNTEXTURED_FILE = MANIFEST[UNTEXTURED_KEY]?.file;

function samplePath(file: string): string {
	return resolve(PALS_DIR, file);
}

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

async function loadRealPalMesh(key: string, file: string) {
	const fileBytes = readFileSync(samplePath(file));
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
		if (!url.includes(file)) {
			throw new Error(`unexpected fetch in test: ${url}`);
		}
		return new Response(arrayBuffer.slice(0));
	}) as typeof fetch;

	const { requestPalMesh, palMeshFailed, onPalMeshLoaded } = await import('./palMeshLibrary');

	const baseUrl = 'http://pals.test/models/pals';
	expect(requestPalMesh(key, baseUrl)).toBeNull();

	await new Promise<void>((resolvePromise, rejectPromise) => {
		const timeout = setTimeout(() => rejectPromise(new Error('load did not settle')), 10000);
		const unsubscribe = onPalMeshLoaded(() => {
			clearTimeout(timeout);
			unsubscribe();
			resolvePromise();
		});
	});

	return {
		object: requestPalMesh(key, baseUrl),
		secondCall: requestPalMesh(key, baseUrl),
		failed: palMeshFailed(key)
	};
}

describe('palMeshLibrary (real gltfpack/meshopt/webp Pal GLBs)', () => {
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

	// A mocked loader cannot catch format-level defects in the baked assets. This
	// caught one: a bake left a `skin` index on a mesh node with no JOINTS_0/
	// WEIGHTS_0 attributes, and three marks any such node a SkinnedMesh and then
	// crashes in normalizeSkinWeights(). Attribute inspection missed it because
	// the dangling reference was on the node, not the primitive.
	it.skipIf(!FILE || !existsSync(samplePath(FILE)))(
		'requestPalMesh decodes a real meshopt-compressed, quantized, webp-textured Pal GLB',
		async () => {
			const { object, secondCall, failed } = await loadRealPalMesh(KEY, FILE);

			expect(failed).toBe(false);
			expect(object).toBeInstanceOf(THREE.Object3D);

			let meshCount = 0;
			let materialCount = 0;
			const box = new THREE.Box3();
			object!.traverse((child) => {
				const mesh = child as THREE.Mesh;
				if (!mesh.isMesh || !mesh.geometry) return;
				meshCount++;
				if (mesh.material) materialCount++;
				for (const name of Object.keys(mesh.geometry.attributes)) {
					expect(mesh.geometry.attributes[name].array).toBeInstanceOf(Float32Array);
				}
				mesh.geometry.computeBoundingBox();
				if (mesh.geometry.boundingBox) box.union(mesh.geometry.boundingBox);
			});
			expect(meshCount).toBeGreaterThan(0);

			expect(materialCount).toBeGreaterThan(0);

			// Finite non-zero extents catch geometry destroyed by an in-place
			// applyMatrix4 over quantized attributes, not just "something parsed".
			const size = new THREE.Vector3();
			box.getSize(size);
			for (const axis of ['x', 'y', 'z'] as const) {
				expect(Number.isFinite(size[axis])).toBe(true);
				expect(size[axis]).toBeGreaterThan(0);
			}

			// Unit-conversion guard. Both the metre->centimetre conversion and the
			// dequantization transform must be baked into geometry, leaving the root
			// at identity: palLayer copies a matrix straight into `.matrix` with
			// matrixAutoUpdate off and so never composes a node-level scale.
			//
			// The extent bounds are wide but exclude both known-broken variants by
			// orders of magnitude: a node-level `root.scale` leaves geometry in raw
			// quantized units (tens of thousands), and scaling raw attributes before
			// baking the node translation displaces the mesh rather than mis-scaling
			// it. Real anubis measures ~241 x 287 x 138 cm.
			expect(object!.scale.x).toBeCloseTo(1, 10);
			expect(object!.scale.y).toBeCloseTo(1, 10);
			expect(object!.scale.z).toBeCloseTo(1, 10);
			for (const axis of ['x', 'y', 'z'] as const) {
				expect(size[axis]).toBeGreaterThan(20);
				expect(size[axis]).toBeLessThan(1000);
			}

			expect(secondCall).toBe(object);
		}
	);

	// The bakes leave metallicFactor/roughnessFactor unset (245 of 245 materials
	// across all 88 shipped GLBs), so GLTFLoader applies glTF's 1.0 default. A
	// fully metallic material computes diffuse as `rgb * (1 - metalness)` = 0, so
	// AmbientLight leaves every Pal near-black. Only a real load sees this.
	it.skipIf(!FILE || !existsSync(samplePath(FILE)))(
		'normalizes every material off the glTF fully-metallic default so ambient light reaches it',
		async () => {
			const { object } = await loadRealPalMesh(KEY, FILE);

			let materialCount = 0;
			object!.traverse((child) => {
				const mesh = child as THREE.Mesh;
				if (!mesh.isMesh || !mesh.material) return;
				for (const mat of Array.isArray(mesh.material) ? mesh.material : [mesh.material]) {
					materialCount++;
					const std = mat as THREE.MeshStandardMaterial;
					expect(std.metalness).toBe(0);
					expect(std.roughness).toBe(0.9);
				}
			});
			expect(materialCount).toBeGreaterThan(0);
		}
	);

	it.skipIf(!FILE || !existsSync(samplePath(FILE)))(
		'gives textured materials an emissive floor and leaves untextured ones unlit',
		async () => {
			const { object } = await loadRealPalMesh(KEY, FILE);
			let textured = 0;
			let untextured = 0;
			object!.traverse((child) => {
				const mesh = child as THREE.Mesh;
				if (!mesh.isMesh) return;
				for (const mat of Array.isArray(mesh.material) ? mesh.material : [mesh.material]) {
					const std = mat as THREE.MeshStandardMaterial;
					if (std.map) {
						textured++;
						expect(std.emissiveMap).toBe(std.map);
						expect(std.emissiveIntensity).toBeCloseTo(0.25, 6);
						expect(std.emissive.getHex()).toBe(0xffffff);
					} else {
						untextured++;
						expect(std.emissiveMap).toBeNull();
						expect(std.emissive.getHex()).toBe(0x000000);
					}
				}
			});
			expect(textured).toBeGreaterThan(0);
		}
	);

	it.skipIf(!UNTEXTURED_FILE || !existsSync(samplePath(UNTEXTURED_FILE)))(
		'leaves untextured materials unlit even when the model also has textured ones',
		async () => {
			const { object } = await loadRealPalMesh(UNTEXTURED_KEY, UNTEXTURED_FILE);
			let textured = 0;
			let untextured = 0;
			object!.traverse((child) => {
				const mesh = child as THREE.Mesh;
				if (!mesh.isMesh) return;
				for (const mat of Array.isArray(mesh.material) ? mesh.material : [mesh.material]) {
					const std = mat as THREE.MeshStandardMaterial;
					if (std.map) {
						textured++;
						expect(std.emissiveMap).toBe(std.map);
						expect(std.emissiveIntensity).toBeCloseTo(0.25, 6);
						expect(std.emissive.getHex()).toBe(0xffffff);
					} else {
						untextured++;
						expect(std.emissiveMap).toBeNull();
						expect(std.emissive.getHex()).toBe(0x000000);
					}
				}
			});
			expect(textured).toBeGreaterThan(0);
			expect(untextured).toBeGreaterThan(0);
		}
	);

	// Mercator mirrors handedness (see MESH_FLIP), flipping winding so FrontSide
	// culls the visible faces and a Pal renders its own interior. GLTFLoader
	// defaults to FrontSide; anubis shipped single-sided and rendered inside-out.
	it.skipIf(!FILE || !existsSync(samplePath(FILE)))(
		'renders every Pal material double-sided, whatever the glTF declared',
		async () => {
			const { object } = await loadRealPalMesh(KEY, FILE!);

			let materials = 0;
			object!.traverse((child) => {
				const mesh = child as THREE.Mesh;
				if (!mesh.isMesh) return;
				for (const mat of Array.isArray(mesh.material) ? mesh.material : [mesh.material]) {
					materials++;
					expect(mat.side).toBe(THREE.DoubleSide);
				}
			});
			expect(materials).toBeGreaterThan(0);
		}
	);
	// PalModelViewer spins the model about +y, an assumption inherited from
	// MESH_FLIP rather than measured anywhere. Anubis is a tall biped, so a model
	// exported z-up fails this and would spin lying on its side.
	it.skipIf(!FILE || !existsSync(samplePath(FILE)))(
		'ships geometry that stands up the +y axis',
		async () => {
			const { object } = await loadRealPalMesh(KEY, FILE!);

			const box = new THREE.Box3().setFromObject(object!);
			const size = box.getSize(new THREE.Vector3());

			expect(size.y, `extents ${size.x} x ${size.y} x ${size.z}`).toBeGreaterThan(size.x);
			expect(size.y, `extents ${size.x} x ${size.y} x ${size.z}`).toBeGreaterThan(size.z);
		}
	);
});
