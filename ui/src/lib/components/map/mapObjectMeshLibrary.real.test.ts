// Loads real map-object GLBs through the unmocked GLTFLoader. Only a real parser
// can show whether the meshopt decoder is registered, quantized attributes
// survive dequantization, and embedded webp images resolve into a material's
// .map. Only the network is stubbed; the texture path needs `self`/`document`
// globals Node does not define.
import { afterEach, describe, expect, it, vi } from 'vitest';
import { existsSync, readFileSync } from 'node:fs';
import { resolve } from 'node:path';
import * as THREE from 'three';
import manifestJson from '../../../../../data/json/map_object_meshes.json';
import { meshNames, type MapObjectManifest } from './mapObjectMesh';

const MANIFEST = manifestJson as unknown as MapObjectManifest;
const MODELS_DIR = resolve(__dirname, '../../../../static/models/mapobjects');
// FileLoader's `new Request(url)` throws on a bare "/..." path outside a browser;
// an absolute dir sidesteps that without changing behaviour.
const DIR = 'http://mapobjects.test/models/mapobjects';

const NAMES = meshNames(MANIFEST);
// Ship two primitives/materials each -- the case a single shared material would
// render wrong for one half of the mesh.
const MULTI_MATERIAL_NAMES = ['SM_FastTravelStatueVariant_185d80', 'SM_JewelBase_496ae3'];

function samplePath(name: string): string {
	return resolve(MODELS_DIR, `${name}.glb`);
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

async function loadRealMeshes(names: string[]) {
	// three's FileLoader reports progress via a browser-only ProgressEvent; a
	// minimal stand-in is enough since only the final outcome is read here.
	(global as { ProgressEvent?: unknown }).ProgressEvent = class {
		constructor(
			public type: string,
			public init?: unknown
		) {}
	};

	// GLTFParser.loadImageSource reads `self.URL` unconditionally, and Node has no
	// `self`. Node's own URL supports createObjectURL, so aliasing suffices.
	(global as { self?: unknown }).self = globalThis;

	(global as { document?: unknown }).document = {
		createElementNS: (_ns: string, tag: string) => (tag === 'img' ? new FakeImageElement() : null)
	};

	global.fetch = vi.fn(async (input: RequestInfo | URL) => {
		const url = input instanceof Request ? input.url : String(input);
		const name = names.find((n) => url.endsWith(`${n}.glb`));
		if (!name) throw new Error(`unexpected fetch in test: ${url}`);
		const bytes = readFileSync(samplePath(name));
		return new Response(bytes.buffer.slice(bytes.byteOffset, bytes.byteOffset + bytes.byteLength));
	}) as typeof fetch;

	const { requestMapObjectMesh, mapObjectMeshFailed, onMapObjectMeshLoaded } = await import(
		'./mapObjectMeshLibrary'
	);

	const settled = () => names.every((n) => requestMapObjectMesh(n, DIR) !== null || mapObjectMeshFailed(n));
	for (const name of names) requestMapObjectMesh(name, DIR);

	if (!settled()) {
		await new Promise<void>((resolvePromise, rejectPromise) => {
			const timeout = setTimeout(() => rejectPromise(new Error('loads did not settle')), 20000);
			const unsubscribe = onMapObjectMeshLoaded(() => {
				if (!settled()) return;
				clearTimeout(timeout);
				unsubscribe();
				resolvePromise();
			});
		});
	}

	return { bundle: (name: string) => requestMapObjectMesh(name, DIR), meshFailed: mapObjectMeshFailed };
}

describe('the shipped map object meshes', () => {
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

	it.skipIf(NAMES.some((n) => !existsSync(samplePath(n))))(
		'all decode into non-empty, textured geometry',
		async () => {
			const { bundle, meshFailed } = await loadRealMeshes(NAMES);
			for (const name of NAMES) {
				expect({ name, failed: meshFailed(name) }).toEqual({ name, failed: false });
				const b = bundle(name)!;
				expect({ name, positions: b.geometry.attributes.position.count > 0 }).toEqual({
					name,
					positions: true
				});
				const materials = Array.isArray(b.material) ? b.material : [b.material];
				for (const mat of materials) {
					expect({ name, map: (mat as THREE.MeshStandardMaterial).map }).toEqual({
						name,
						map: expect.any(THREE.Texture)
					});
				}
			}
		},
		30000
	);

	it.skipIf(MULTI_MATERIAL_NAMES.some((n) => !existsSync(samplePath(n))))(
		'bundles a multi-material glb with one geometry group per material',
		async () => {
			const { bundle } = await loadRealMeshes(MULTI_MATERIAL_NAMES);
			for (const name of MULTI_MATERIAL_NAMES) {
				const b = bundle(name)!;
				expect(Array.isArray(b.material)).toBe(true);
				const materials = b.material as THREE.Material[];
				expect(materials).toHaveLength(2);
				expect(b.geometry.groups).toHaveLength(2);
				for (const mat of materials) {
					expect((mat as THREE.MeshStandardMaterial).map).toBeInstanceOf(THREE.Texture);
				}
			}
		},
		30000
	);

	// Expected values come from this GLB's own glTF JSON: POSITION min/max are
	// [0,0,0]/[10934, 16383, 6906] and the node scale is uniform 0.000229026889
	// with no rotation, so a correct box is (max - min) * scale per axis, times
	// the metre->centimetre factor. A statue 3.75 tall instead of 375 would look
	// plausible in a debugger and be invisible on the map.
	const STATUE = 'SM_FastTravelStatue_61071e';
	const CM_PER_UNIT = 100;
	const QUANT_SCALE = 0.000229026889;
	const EXPECTED_SIZE_CM = {
		x: 10934 * QUANT_SCALE * CM_PER_UNIT,
		y: 16383 * QUANT_SCALE * CM_PER_UNIT,
		z: 6906 * QUANT_SCALE * CM_PER_UNIT
	};

	it.skipIf(!existsSync(samplePath(STATUE)))(
		'stand in centimetres, not metres',
		async () => {
			const { bundle } = await loadRealMeshes([STATUE]);
			const geo = bundle(STATUE)!.geometry;
			geo.computeBoundingBox();
			const size = new THREE.Vector3();
			geo.boundingBox!.getSize(size);

			for (const axis of ['x', 'y', 'z'] as const) {
				expect(size[axis]).toBeGreaterThan(EXPECTED_SIZE_CM[axis] * 0.95);
				expect(size[axis]).toBeLessThan(EXPECTED_SIZE_CM[axis] * 1.05);
			}
		},
		30000
	);
});
