// Lazy, cached textured geometry+material for map objects (relics, fast travel
// statues). These glbs are meshopt, not Draco like structures, so this keeps its
// own loader. Unlike palMeshLibrary these are instanced, and an InstancedMesh
// needs one geometry plus one material (or an array keyed by geometry.groups)
// rather than a scene graph -- hence a bundle per mesh name, not an Object3D.
import * as THREE from 'three';
import type { GLTFLoader } from 'three/examples/jsm/loaders/GLTFLoader.js';
import {
	bundleMapObjectMesh,
	configureTexturedMaterial,
	createMeshoptGLTFLoader,
	dequantizeToFloat32,
	disposeTexturedBundle,
	type TexturedMeshBundle
} from './meshLibrary';

export { bundleMapObjectMesh };

const MODEL_URL = '/models/mapobjects';

// The exporter's 0.01 scale-down leaves positions in metres; placement numbers
// are UE centimetres.
const CM_PER_UNIT = 100;

export type MapObjectMeshBundle = TexturedMeshBundle;

const cache = new Map<string, MapObjectMeshBundle>();
const inflight = new Set<string>();
const failed = new Set<string>();
const listeners = new Set<() => void>();
const lastTouch = new Map<string, number>();

/** Drops cached bundles not in `active` (see meshUsage.ts) whose last request
 * is older than `maxAgeMs`, disposing geometry and texture memory. */
export function sweepMapObjectMeshes(
	active: ReadonlySet<string>,
	maxAgeMs: number,
	now: number = Date.now()
): { swept: number } {
	let swept = 0;
	for (const [name, bundle] of cache) {
		if (active.has(name)) continue;
		const touched = lastTouch.get(name) ?? 0;
		if (now - touched < maxAgeMs) continue;
		disposeTexturedBundle(bundle);
		cache.delete(name);
		lastTouch.delete(name);
		swept += 1;
	}
	return { swept };
}

// These declare EXT_meshopt_compression, so without the decoder every one fails
// to parse.
let loader: GLTFLoader | null = null;
function gltfLoader(): GLTFLoader {
	if (!loader) loader = createMeshoptGLTFLoader();
	return loader;
}

export function onMapObjectMeshLoaded(cb: () => void): () => void {
	listeners.add(cb);
	return () => listeners.delete(cb);
}

export function mapObjectMeshFailed(name: string): boolean {
	return failed.has(name);
}

function settle(name: string): void {
	inflight.delete(name);
	for (const cb of listeners) cb();
}

export function requestMapObjectMesh(
	name: string,
	baseUrl: string = MODEL_URL
): MapObjectMeshBundle | null {
	const hit = cache.get(name);
	if (hit) {
		lastTouch.set(name, Date.now());
		return hit;
	}
	if (inflight.has(name) || failed.has(name)) return null;

	inflight.add(name);
	const onError = (err: unknown) => {
		console.warn(`[mapObjectMeshLibrary] ${name}: load failed`, err);
		failed.add(name);
		settle(name);
	};
	try {
		gltfLoader().load(
			`${baseUrl}/${name}.glb`,
			(gltf) => {
				const root = gltf.scene;
				// Makes the quantization node's translation+scale available via
				// matrixWorld before it is baked into geometry below.
				root.updateMatrixWorld(true);

				const geometries: THREE.BufferGeometry[] = [];
				const materials: THREE.Material[] = [];
				root.traverse((child) => {
					const mesh = child as THREE.Mesh;
					if (!mesh.isMesh || !mesh.geometry) return;

					const geo = mesh.geometry;
					for (const key of Object.keys(geo.attributes)) {
						const attr = geo.attributes[key];
						if (!(attr.array instanceof Float32Array)) {
							geo.setAttribute(key, dequantizeToFloat32(attr));
						}
					}
					geo.applyMatrix4(mesh.matrixWorld);
					geo.scale(CM_PER_UNIT, CM_PER_UNIT, CM_PER_UNIT);
					geometries.push(geo);

					const material = Array.isArray(mesh.material) ? mesh.material[0] : mesh.material;
					configureTexturedMaterial(material);
					materials.push(material);
				});

				const bundle = bundleMapObjectMesh(geometries, materials);
				if (!bundle) {
					console.warn(`[mapObjectMeshLibrary] ${name}: no usable mesh in glb`);
					failed.add(name);
					settle(name);
					return;
				}

				cache.set(name, bundle);
				lastTouch.set(name, Date.now());
				settle(name);
			},
			undefined,
			onError
		);
	} catch (err) {
		// FileLoader throws synchronously rather than via onError when fetch cannot
		// resolve a relative URL (Node). Treat it as an ordinary load failure.
		onError(err);
	}
	return null;
}
