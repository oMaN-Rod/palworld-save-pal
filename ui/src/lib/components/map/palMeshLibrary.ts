// Lazy, cached textured glTF models for Pals. Unlike the structure/scenery
// library this keeps materials, so it returns a whole Object3D rather than
// merged geometry. That Object3D is shared and cached -- callers must clone
// before display (see requestPalMesh).
//
// Contract is "geometry in centimetres", matching meshLibrary: the conversion
// and the dequantization transform are baked into geometry before caching, so
// consumers get an object whose own transform is identity.
import * as THREE from 'three';
import type { GLTFLoader } from 'three/examples/jsm/loaders/GLTFLoader.js';
import manifest from '../../../../../data/json/pal_meshes.json';
import { configureTexturedMaterial, createMeshoptGLTFLoader, dequantizeToFloat32 } from './meshLibrary';
import { resolvePalModelKey } from './palModelKey';

type Entry = { file: string; bytes: number; triangles: number };
const MANIFEST = manifest as unknown as Record<string, Entry>;
const MODEL_URL = '/models/pals';

// Positions arrive in metres from the exporter's 0.01 scale-down; every
// placement number on the map is UE centimetres.
const CM_PER_UNIT = 100;

const cache = new Map<string, THREE.Object3D>();
const inflight = new Set<string>();
const failed = new Set<string>();
const listeners = new Set<() => void>();

// Pal models declare EXT_meshopt_compression in extensionsRequired, and glTF
// requires loaders to reject assets whose required extensions they cannot
// satisfy -- without the meshopt decoder every Pal fails to parse.
let loader: GLTFLoader | null = null;
function gltfLoader(): GLTFLoader {
	if (!loader) loader = createMeshoptGLTFLoader();
	return loader;
}

const manifestHas = (key: string) => key in MANIFEST;

// The identity every map here is keyed by. Callers arrive with whatever the save
// gave them ("Anubis", "BOSS_KingWhale_Otomo"); resolving first is what lets two
// Pals sharing a model share one download and one recorded failure. A key that
// resolves to nothing still needs a stable identity, hence the fallback.
function identity(rawKey: string): string {
	return resolvePalModelKey(rawKey, manifestHas) ?? rawKey.toLowerCase();
}

// Real-artifact tests override baseUrl with an absolute origin: Node's fetch has
// no document/location base and cannot resolve a root-relative URL.
export function palModelUrl(key: string, baseUrl: string = MODEL_URL): string | null {
	const resolved = resolvePalModelKey(key, manifestHas);
	return resolved ? `${baseUrl}/${MANIFEST[resolved].file}` : null;
}

export function onPalMeshLoaded(cb: () => void): () => void {
	listeners.add(cb);
	return () => listeners.delete(cb);
}

export function palMeshFailed(key: string): boolean {
	return failed.has(identity(key));
}

function settle(key: string): void {
	inflight.delete(key);
	for (const cb of listeners) cb();
}

// Returns the SAME cached Object3D on every call, so a glb is fetched and parsed
// once however many spawn points use it. An Object3D can only have one parent,
// so adding this to a scene twice silently reparents it -- and "grassgolem"
// really does appear at two spawn points. Callers MUST clone before adding.
export function requestPalMesh(rawKey: string, baseUrl: string = MODEL_URL): THREE.Object3D | null {
	const key = identity(rawKey);
	const hit = cache.get(key);
	if (hit) return hit;
	if (inflight.has(key) || failed.has(key)) return null;

	const url = palModelUrl(key, baseUrl);
	if (!url) {
		// Not every boss has a model; record it once so the layer stops asking.
		console.warn(`[palMeshLibrary] ${key}: no manifest entry, no model will render`);
		failed.add(key);
		return null;
	}

	inflight.add(key);
	const onError = (err: unknown) => {
		console.warn(`[palMeshLibrary] ${key}: load failed`, err);
		failed.add(key);
		settle(key);
	};
	try {
		gltfLoader().load(
			url,
			(gltf) => {
				const root = gltf.scene;
				// Dequantization is stored as a node translation *and* scale, not scale
				// alone; this makes it available per mesh via matrixWorld for the bake
				// below.
				root.updateMatrixWorld(true);

				const meshes: THREE.Mesh[] = [];
				root.traverse((child) => {
					const mesh = child as THREE.Mesh;
					if (!mesh.isMesh || !mesh.geometry) return;
					// Quantized attributes are integer typed arrays; an in-place matrix
					// application would write floats back into them and truncate every
					// component.
					for (const name of Object.keys(mesh.geometry.attributes)) {
						const attr = mesh.geometry.attributes[name];
						if (!(attr.array instanceof Float32Array)) {
							mesh.geometry.setAttribute(name, dequantizeToFloat32(attr));
						}
					}
					// Bake the ancestor transform in before the scale below, not after:
					// the dequantization translation is comparable in magnitude to the
					// scaled term, so scaling the raw geometry first and leaving the node
					// to apply at render time would give the translation 1x and the
					// quantized term 100x -- a displaced mesh, not just a mis-scaled one.
					mesh.geometry.applyMatrix4(mesh.matrixWorld);
					mesh.geometry.scale(CM_PER_UNIT, CM_PER_UNIT, CM_PER_UNIT);
					for (const mat of Array.isArray(mesh.material) ? mesh.material : [mesh.material]) {
						configureTexturedMaterial(mat);
					}
					meshes.push(mesh);
				});

				// Geometry now holds final absolute coordinates, so reparent onto `root`
				// with an identity local transform -- otherwise the quantization node
				// still above it would compose into matrixWorld and double-apply.
				for (const mesh of meshes) {
					mesh.position.set(0, 0, 0);
					mesh.quaternion.identity();
					mesh.scale.set(1, 1, 1);
					root.add(mesh);
				}

				cache.set(key, root);
				settle(key);
			},
			undefined,
			onError
		);
	} catch (err) {
		// FileLoader throws synchronously rather than via onError when fetch cannot
		// resolve a relative URL (Node, which has no document base). Treat it as an
		// ordinary load failure instead of letting it escape to the caller.
		onError(err);
	}
	return null;
}
