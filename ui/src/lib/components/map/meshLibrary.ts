// Lazy, cached glTF geometry for structure meshes. The render layer cannot await,
// so requestMesh returns cached geometry or null and notifies when a load settles
// (successfully or permanently). Failed names are never retried.
import * as THREE from 'three';
import { GLTFLoader } from 'three/examples/jsm/loaders/GLTFLoader.js';
import { DRACOLoader } from 'three/examples/jsm/loaders/DRACOLoader.js';
import { mergeGeometries } from 'three/examples/jsm/utils/BufferGeometryUtils.js';
import manifest from '../../../../../data/json/structure_meshes.json';
import type { MeshPart } from './meshPlacement';

export type ManifestPart = MeshPart & { mesh: string };
type Entry = { parts: ManifestPart[]; material?: string };

const MANIFEST = manifest as unknown as Record<string, Entry>;
const MODEL_URL = '/models/structures';

// FModel/CUE4Parse's glTF exporter applies a 0.01 scale-down, so shipped .glb
// vertex positions are in metres while every other structure-placement number
// (partLocalMatrix, structurePlacement, buildArchetypeGeometry) is UE
// centimetres. Scaling here, once, keeps the library's contract "geometry in
// centimetres" for every consumer.
const CM_PER_UNIT = 100;

const cache = new Map<string, THREE.BufferGeometry>();
const inflight = new Set<string>();
const failed = new Set<string>();
const listeners = new Set<() => void>();

let loader: GLTFLoader | null = null;
function gltfLoader(): GLTFLoader {
	if (!loader) {
		loader = new GLTFLoader();
		const draco = new DRACOLoader();
		draco.setDecoderPath('/draco/');
		loader.setDRACOLoader(draco);
	}
	return loader;
}

// Saves spell some ids with different casing than the data table row key
// ("Stone_Foundation" vs "Stone_foundation"). Exact match wins; the lowercase
// index is only a fallback. Unlike the footprint registry the manifest has no
// case-colliding keys, so this mapping is unambiguous.
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

export function onMeshLoaded(cb: () => void): () => void {
	listeners.add(cb);
	return () => listeners.delete(cb);
}

export function meshFailed(name: string): boolean {
	return failed.has(name);
}

function settle(name: string): void {
	inflight.delete(name);
	for (const cb of listeners) cb();
}

// Attributes differing across meshes (e.g. one has UVs, another doesn't) make
// mergeGeometries() return null. Retry using only the attributes every geometry
// shares, so the merge still succeeds and no geometry is silently dropped.
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

export function requestMesh(name: string): THREE.BufferGeometry | null {
	const hit = cache.get(name);
	if (hit) return hit;
	if (inflight.has(name) || failed.has(name)) return null;

	inflight.add(name);
	gltfLoader().load(
		`${MODEL_URL}/${name}.glb`,
		(gltf) => {
			const geometries: THREE.BufferGeometry[] = [];
			gltf.scene.updateMatrixWorld(true);
			gltf.scene.traverse((child) => {
				const mesh = child as THREE.Mesh;
				if (mesh.isMesh && mesh.geometry) {
					const geo = mesh.geometry.clone();
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
				// structure silently drawn as a proxy box with no way to diagnose it.
				console.warn(
					`[meshLibrary] ${name}: ${geometries.length} mesh(es) in glb, merge produced no geometry`
				);
				failed.add(name);
			}
			settle(name);
		},
		undefined,
		(err) => {
			console.warn(`[meshLibrary] ${name}: load failed`, err);
			failed.add(name);
			settle(name);
		}
	);
	return null;
}
