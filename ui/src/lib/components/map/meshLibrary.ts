// Lazy, cached glTF geometry for structure meshes. The render layer cannot await,
// so requestMesh returns cached geometry or null and notifies when a load settles
// (successfully or permanently). Failed names are never retried.
import * as THREE from 'three';
import { GLTFLoader } from 'three/examples/jsm/loaders/GLTFLoader.js';
import { DRACOLoader } from 'three/examples/jsm/loaders/DRACOLoader.js';
import { MeshoptDecoder } from 'three/examples/jsm/libs/meshopt_decoder.module.js';
import { mergeGeometries } from 'three/examples/jsm/utils/BufferGeometryUtils.js';
import manifest from '../../../../../data/json/structure_meshes.json';
import type { MeshPart } from './meshPlacement';

export type ManifestPart = MeshPart & { mesh: string };
type Entry = { parts: ManifestPart[]; material?: string };

const MANIFEST = manifest as unknown as Record<string, Entry>;
export const STRUCTURE_MODEL_DIR = '/models/structures';
const MODEL_URL = STRUCTURE_MODEL_DIR;

// The glTF exporter applies a 0.01 scale-down, so shipped .glb positions are in
// metres while every other placement number is UE centimetres. Scaling once here
// keeps the library's contract "geometry in centimetres".
const CM_PER_UNIT = 100;

// Keyed by name alone, not (name, dir): structure manifest ids and scenery's
// hashed keys cannot collide today. A third mesh source reusing a name from
// either would collide here.
const cache = new Map<string, THREE.BufferGeometry>();
const inflight = new Set<string>();
const failed = new Set<string>();
// Scoped by request directory: the layers sharing this cache have very different
// mesh sets, and each settle wakes a listener into a full rebuild. Unscoped, 79
// of 82 structure rebuilds during one base load came from meshes that layer
// never draws. A listener with no directory still hears everything.
type Listener = { cb: () => void; dir?: string };
const listeners = new Set<Listener>();

let loader: GLTFLoader | null = null;
function gltfLoader(): GLTFLoader {
	if (!loader) {
		loader = new GLTFLoader();
		const draco = new DRACOLoader();
		draco.setDecoderPath('/draco/');
		loader.setDRACOLoader(draco);
		// Structure glbs need only Draco above; scenery glbs are meshopt-compressed,
		// and an unsatisfied extensionsRequired entry is a hard parse failure rather
		// than a soft fallback, so without this every scenery asset is rejected.
		loader.setMeshoptDecoder(MeshoptDecoder);
	}
	return loader;
}

// Pal and map-object glbs need meshopt but no Draco. Shared so the two callers'
// loader setup can't drift; each keeps its own singleton since they track
// separate cache/inflight/failed sets.
export function createMeshoptGLTFLoader(): GLTFLoader {
	const loader = new GLTFLoader();
	loader.setMeshoptDecoder(MeshoptDecoder);
	return loader;
}

// Corrects a GLTFLoader-parsed material for this map's rendering conventions.
// Each assignment is load-bearing:
//   metalness - glTF defaults it to 1.0, and a fully metallic material computes
//     diffuse as `rgb * (1 - metalness)`, so AmbientLight leaves it black.
//   side      - mercator mirrors handedness (see MESH_FLIP), reversing winding,
//     so the FrontSide default culls exactly the faces that should be visible.
//   emissive  - guarded on `map` because three only multiplies emissive by
//     emissiveMap when one is present; unguarded, untextured meshes glow grey.
//   vertexColors - UE exports an all-zero COLOR_0 that GLTFLoader enables on
//     sight, zeroing both diffuse and alpha (unlit surfaces, invisible glass).
export function configureTexturedMaterial(material: THREE.Material): void {
	const std = material as THREE.MeshStandardMaterial;
	std.metalness = 0;
	std.roughness = 0.9;
	std.side = THREE.DoubleSide;
	std.vertexColors = false;
	if (std.map) {
		std.emissive = new THREE.Color(0xffffff);
		std.emissiveMap = std.map;
		std.emissiveIntensity = 0.25;
	}
}

// Saves spell some ids with different casing than the data table row key
// ("Stone_Foundation" vs "Stone_foundation"). Exact match wins. The manifest has
// no case-colliding keys, so the lowercase fallback is unambiguous.
let lowerIndex: Map<string, Entry> | null = null;

export function structureParts(mapObjectId: string): ManifestPart[] | null {
	const exact = MANIFEST[mapObjectId];
	if (exact) return exact.parts;
	if (!lowerIndex) {
		lowerIndex = new Map();
		for (const [key, value] of Object.entries(MANIFEST)) {
			lowerIndex.set(key.toLowerCase(), value);
		}
	}
	return lowerIndex.get(mapObjectId.toLowerCase())?.parts ?? null;
}

export function onMeshLoaded(cb: () => void, dir?: string): () => void {
	const listener: Listener = { cb, dir };
	listeners.add(listener);
	return () => listeners.delete(listener);
}

export function meshFailed(name: string): boolean {
	return failed.has(name);
}

function settle(name: string, dir: string): void {
	inflight.delete(name);
	for (const listener of listeners) {
		if (listener.dir === undefined || listener.dir === dir) listener.cb();
	}
}

// Attributes differing across meshes (one has UVs, another doesn't) make
// mergeGeometries() return null. Retry with only the shared attributes so no
// geometry is silently dropped.
function normalizeForMerge(geometries: THREE.BufferGeometry[]): THREE.BufferGeometry[] {
	const allIndexed = geometries.every((g) => g.index !== null);
	const anyIndexed = geometries.some((g) => g.index !== null);
	const deindex = anyIndexed && !allIndexed;

	const attrNames = geometries.map((geo) => Object.keys(geo.attributes));
	const commonAttrs: string[] = attrNames.reduce(
		(acc, names) => acc.filter((n) => names.includes(n)),
		attrNames[0] ?? []
	);

	return geometries.map((geo) => {
		const normalized = deindex && geo.index ? geo.toNonIndexed() : geo;
		for (const name of Object.keys(normalized.attributes)) {
			if (!commonAttrs.includes(name)) normalized.deleteAttribute(name);
		}
		return normalized;
	});
}

function mergeAll(geometries: THREE.BufferGeometry[]): THREE.BufferGeometry | null {
	return mergeGeometries(geometries, false) ?? mergeGeometries(normalizeForMerge(geometries), false);
}

const COMPONENT_GETTERS = ['getX', 'getY', 'getZ', 'getW'] as const;

// Copies a quantized or interleaved attribute into its own dense Float32Array.
// Reads through get*() rather than `Float32Array.from(attr.array)`: for an
// InterleavedBufferAttribute `.array` is the entire shared buffer, not this
// attribute's slice, so a raw copy pulls in neighbouring attributes' bytes.
// get*() resolves stride/offset and undoes `normalized` scaling either way.
// The result is deliberately not marked `normalized` -- it holds real floats now.
export function dequantizeToFloat32(
	attr: THREE.BufferAttribute | THREE.InterleavedBufferAttribute
): THREE.BufferAttribute {
	const itemSize = attr.itemSize;
	const out = new Float32Array(attr.count * itemSize);
	for (let i = 0; i < attr.count; i++) {
		for (let c = 0; c < itemSize; c++) {
			out[i * itemSize + c] = attr[COMPONENT_GETTERS[c]](i);
		}
	}
	return new THREE.BufferAttribute(out, itemSize, false);
}

export type TexturedMeshBundle = {
	geometry: THREE.BufferGeometry;
	material: THREE.Material | THREE.Material[];
};

// Bundles one glb's per-primitive geometries and materials into what an
// InstancedMesh needs. Multi-material glbs merge with groups, and
// mergeGeometries sets each group's materialIndex to the input array position --
// the same order the caller built `materials` in, so the two stay matched
// regardless of the glb's own primitive order.
export function bundleMapObjectMesh(
	geometries: THREE.BufferGeometry[],
	materials: THREE.Material[]
): TexturedMeshBundle | null {
	if (geometries.length === 0) return null;
	if (geometries.length === 1) return { geometry: geometries[0], material: materials[0] };
	const geometry = mergeGeometries(geometries, true);
	if (!geometry) return null;
	return { geometry, material: materials };
}

const texturedCache = new Map<string, TexturedMeshBundle>();
const texturedInflight = new Set<string>();
const texturedFailed = new Set<string>();
const texturedListeners = new Set<Listener>();

export function onTexturedMeshLoaded(cb: () => void, dir?: string): () => void {
	const listener: Listener = { cb, dir };
	texturedListeners.add(listener);
	return () => texturedListeners.delete(listener);
}

export function texturedMeshFailed(name: string): boolean {
	return texturedFailed.has(name);
}

function settleTextured(name: string, dir: string): void {
	texturedInflight.delete(name);
	for (const listener of texturedListeners) {
		if (listener.dir === undefined || listener.dir === dir) listener.cb();
	}
}

// The textured sibling of requestMesh, keeping each primitive's own material
// instead of merging into one untextured geometry. requestMesh's merge discards
// material assignment, so a structure wanting its glb's texture is parsed again
// through this separate cache; both may hold copies of the same glb at once.
export function requestTexturedMesh(name: string, dir: string = MODEL_URL): TexturedMeshBundle | null {
	const hit = texturedCache.get(name);
	if (hit) return hit;
	if (texturedInflight.has(name) || texturedFailed.has(name)) return null;

	texturedInflight.add(name);
	gltfLoader().load(
		`${dir}/${name}.glb`,
		(gltf) => {
			const geometries: THREE.BufferGeometry[] = [];
			const materials: THREE.Material[] = [];
			gltf.scene.updateMatrixWorld(true);
			gltf.scene.traverse((child) => {
				const mesh = child as THREE.Mesh;
				if (!mesh.isMesh || !mesh.geometry) return;
				const geo = mesh.geometry.clone();
				for (const key of Object.keys(geo.attributes)) {
					const attr = geo.attributes[key];
					if (!(attr.array instanceof Float32Array)) {
						geo.setAttribute(key, dequantizeToFloat32(attr));
					}
				}
				geo.applyMatrix4(mesh.matrixWorld);
				geometries.push(geo);
				const material = Array.isArray(mesh.material) ? mesh.material[0] : mesh.material;
				configureTexturedMaterial(material);
				materials.push(material);
			});
			const bundle = bundleMapObjectMesh(geometries, materials);
			if (!bundle) {
				console.warn(`[meshLibrary] ${name}: no usable textured mesh in glb`);
				texturedFailed.add(name);
				settleTextured(name, dir);
				return;
			}
			bundle.geometry.scale(CM_PER_UNIT, CM_PER_UNIT, CM_PER_UNIT);
			texturedCache.set(name, bundle);
			settleTextured(name, dir);
		},
		undefined,
		(err) => {
			console.warn(`[meshLibrary] ${name}: textured load failed`, err);
			texturedFailed.add(name);
			settleTextured(name, dir);
		}
	);
	return null;
}

export function requestMesh(name: string, dir: string = MODEL_URL): THREE.BufferGeometry | null {
	const hit = cache.get(name);
	if (hit) return hit;
	if (inflight.has(name) || failed.has(name)) return null;

	inflight.add(name);
	gltfLoader().load(
		`${dir}/${name}.glb`,
		(gltf) => {
			const geometries: THREE.BufferGeometry[] = [];
			gltf.scene.updateMatrixWorld(true);
			gltf.scene.traverse((child) => {
				const mesh = child as THREE.Mesh;
				if (mesh.isMesh && mesh.geometry) {
					const geo = mesh.geometry.clone();
					// Quantized attributes arrive as integer typed arrays. applyMatrix4()
					// writes float results back into the attribute's own array, which
					// against a Uint16Array clamps every component to 0..65535 and
					// destroys the mesh. Convert to Float32 first.
					for (const key of Object.keys(geo.attributes)) {
						const attr = geo.attributes[key];
						if (!(attr.array instanceof Float32Array)) {
							geo.setAttribute(key, dequantizeToFloat32(attr));
						}
					}
					geo.applyMatrix4(mesh.matrixWorld);
					geometries.push(geo);
				}
			});
			const merged = geometries.length > 0 ? mergeAll(geometries) : null;
			if (merged) {
				merged.scale(CM_PER_UNIT, CM_PER_UNIT, CM_PER_UNIT);
				cache.set(name, merged);
			} else {
				// Blacklisting is permanent, so say why once rather than leaving a
				// structure silently drawn as a proxy box.
				console.warn(
					`[meshLibrary] ${name}: ${geometries.length} mesh(es) in glb, merge produced no geometry`
				);
				failed.add(name);
			}
			settle(name, dir);
		},
		undefined,
		(err) => {
			console.warn(`[meshLibrary] ${name}: load failed`, err);
			failed.add(name);
			settle(name, dir);
		}
	);
	return null;
}
