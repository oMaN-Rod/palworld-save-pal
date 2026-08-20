// Split into a camera-independent bake and a per-frame compose, like sceneryLayer.
// Instances are culled past the game's per-class cull distance, clamped to never
// go nearer than the camera can see (see viewRadiusCm) -- those distances are
// metres from a first-person camera and would carve a visible circle out of a
// map camera hundreds of metres up if applied literally.
//
// This layer owns no camera subscription, so the caller must re-invoke update() on camera move.
import * as THREE from 'three';
import { MercatorCoordinate, type CustomLayerInterface, type Map as MLMap } from 'maplibre-gl';
import manifestJson from '../../../../../data/json/map_object_meshes.json';
import {
	manifestParts,
	type MapObjectEntry,
	type MapObjectManifest,
	type MapObjectPart
} from './mapObjectMesh';
import { partLocalMatrix, ueEulerToThreeQuaternion } from './meshPlacement';
import { MESH_FLIP, getSharedRenderer } from './structureLayer';
import {
	onMapObjectMeshLoaded,
	requestMapObjectMesh,
	type MapObjectMeshBundle
} from './mapObjectMeshLibrary';
import { lngLatToPixel, pixelToLngLat } from './mercator';
import { pixelToWorld, worldToPixel, type MapArea } from './utils';
import { createMapObjectPortalMesh, disposeMapObjectPortalMesh } from './mapObjectPortal';

const MODEL_DIR = '/models/mapobjects';

const MANIFEST = manifestJson as unknown as MapObjectManifest;

export type MapObjectItem = {
	x: number;
	y: number;
	z: number;
	actorClass: string;
	scale: number;
	portalColor: string;
	// Required rather than defaulted: PORTAL_RADIUS_CM is pal-sized and meaningless
	// for a relic or statue, so the caller must supply its own marker-type constant.
	ringRadiusCm: number;
	// FRotator (pitch, yaw, roll) in degrees, applied before the part's own loc/rot/scale.
	rot: [number, number, number];
};

// A class with no extracted LDMaxDrawDistance is better drawn too far than silently missing.
export const MAP_OBJECT_DEFAULT_CULL_CM = 100000;

export function cullDistanceCmFor(entry: MapObjectEntry | undefined): number {
	return entry?.cullDistanceCm ?? MAP_OBJECT_DEFAULT_CULL_CM;
}

// Takes the furthest of the four corners because a pitched view puts the camera
// centre off the bounds rectangle's centre.
export function viewRadiusCm(
	sw: [number, number],
	ne: [number, number],
	area: MapArea,
	cameraWorldX: number,
	cameraWorldY: number
): number {
	const [pxSW, pySW] = lngLatToPixel(sw[0], sw[1]);
	const [pxNE, pyNE] = lngLatToPixel(ne[0], ne[1]);
	const wSW = pixelToWorld(pxSW, pySW, area);
	const wNE = pixelToWorld(pxNE, pyNE, area);
	const dx = Math.max(Math.abs(wSW.worldX - cameraWorldX), Math.abs(wNE.worldX - cameraWorldX));
	const dy = Math.max(Math.abs(wSW.worldY - cameraWorldY), Math.abs(wNE.worldY - cameraWorldY));
	return Math.hypot(dx, dy);
}

// Everything in mapObjectInstanceMatrix but the cmToMerc factor. Floats per
// instance:
//   0..1   mercator anchor x/y
//   2      world z, centimetres
//   3..4   world x/y, centimetres, for the cull below
//   5      this instance's cull distance, centimetres
//   6..14  MESH_FLIP * the item's own rotation * the part's own rotation and
//          scale, 3x3 column-major, times the item's own scale multiplier
//   15..17 MESH_FLIP * the item's own rotation * the part's own offset,
//          centimetres, times the item's own scale multiplier
export const BAKED_STRIDE = 18;

// partLocalMatrix is per part, so it's computed once and reused across instances;
// the item's own rotation is composed with MESH_FLIP separately, not cached here.
function partLocalCache() {
	const cache = new Map<string, THREE.Matrix4>();
	return (actorClass: string, part: MapObjectPart): THREE.Matrix4 => {
		const cacheKey = `${actorClass}|${part.mesh}`;
		let local = cache.get(cacheKey);
		if (!local) {
			local = partLocalMatrix(part);
			cache.set(cacheKey, local);
		}
		return local;
	};
}

export function bakeMapObjectInstances(
	items: MapObjectItem[],
	area: MapArea,
	manifest: MapObjectManifest = MANIFEST
): Map<string, Float32Array> {
	const byMesh = new Map<string, number[]>();
	const partLocal = partLocalCache();

	for (const item of items) {
		const parts = manifestParts(manifest, item.actorClass);
		if (parts.length === 0) continue;
		const [px, py] = worldToPixel(item.x, item.y, area);
		const [lng, lat] = pixelToLngLat(px, py);
		const anchor = MercatorCoordinate.fromLngLat([lng, lat]);
		const cullCm = cullDistanceCmFor(manifest[item.actorClass]);
		// Like structureLayer's yawRotation, but carrying all three FRotator axes.
		const itemRotation = new THREE.Matrix4().makeRotationFromQuaternion(
			ueEulerToThreeQuaternion(item.rot[0], item.rot[1], item.rot[2])
		);
		const flipRotation = MESH_FLIP.clone().multiply(itemRotation);

		for (const part of parts) {
			const local = flipRotation.clone().multiply(partLocal(item.actorClass, part));
			const e = local.elements;
			let out = byMesh.get(part.mesh);
			if (!out) {
				out = [];
				byMesh.set(part.mesh, out);
			}
			const s = item.scale;
			out.push(
				anchor.x, anchor.y, item.z, item.x, item.y, cullCm,
				e[0] * s, e[1] * s, e[2] * s,
				e[4] * s, e[5] * s, e[6] * s,
				e[8] * s, e[9] * s, e[10] * s,
				e[12] * s, e[13] * s, e[14] * s
			);
		}
	}

	const baked = new Map<string, Float32Array>();
	for (const [mesh, values] of byMesh) baked.set(mesh, new Float32Array(values));
	return baked;
}

// Each baked cull distance is raised to `minCullCm` -- pass the camera's view radius
// so nothing on screen is culled.
export function composeMapObjectMatrices(
	baked: Float32Array,
	cmToMerc: number,
	cameraWorldX: number,
	cameraWorldY: number,
	minCullCm: number,
	target: Float32Array,
	offset: number
): number {
	let written = 0;

	for (let o = 0; o < baked.length; o += BAKED_STRIDE) {
		const dx = baked[o + 3] - cameraWorldX;
		const dy = baked[o + 4] - cameraWorldY;
		const cull = Math.max(baked[o + 5], minCullCm);
		if (dx * dx + dy * dy > cull * cull) continue;

		const t = (offset + written) * 16;
		target[t] = cmToMerc * baked[o + 6];
		target[t + 1] = cmToMerc * baked[o + 7];
		target[t + 2] = cmToMerc * baked[o + 8];
		target[t + 3] = 0;
		target[t + 4] = cmToMerc * baked[o + 9];
		target[t + 5] = cmToMerc * baked[o + 10];
		target[t + 6] = cmToMerc * baked[o + 11];
		target[t + 7] = 0;
		target[t + 8] = cmToMerc * baked[o + 12];
		target[t + 9] = cmToMerc * baked[o + 13];
		target[t + 10] = cmToMerc * baked[o + 14];
		target[t + 11] = 0;
		target[t + 12] = baked[o] + cmToMerc * baked[o + 15];
		target[t + 13] = baked[o + 1] + cmToMerc * baked[o + 16];
		target[t + 14] = cmToMerc * (baked[o + 2] + baked[o + 17]);
		target[t + 15] = 1;
		written++;
	}
	return written;
}

// One row per item with a mesh to stand a beam on, however many parts that mesh
// bakes into above. Floats per instance:
//   0..1 mercator anchor x/y
//   2    world z, centimetres
//   3..4 world x/y, centimetres, for the cull below
//   5    this item's cull distance, centimetres -- same source as its mesh's
//   6    the item's own scale multiplier
//   7..9 portalColor as linear r/g/b
export const PORTAL_BAKED_STRIDE = 10;

export function bakeMapObjectPortalInstances(
	items: MapObjectItem[],
	area: MapArea,
	manifest: MapObjectManifest = MANIFEST
): Float32Array {
	const values: number[] = [];
	const color = new THREE.Color();
	for (const item of items) {
		if (manifestParts(manifest, item.actorClass).length === 0) continue;
		const [px, py] = worldToPixel(item.x, item.y, area);
		const [lng, lat] = pixelToLngLat(px, py);
		const anchor = MercatorCoordinate.fromLngLat([lng, lat]);
		const cullCm = cullDistanceCmFor(manifest[item.actorClass]);
		color.set(item.portalColor);
		values.push(
			anchor.x, anchor.y, item.z, item.x, item.y, cullCm, item.scale,
			color.r, color.g, color.b
		);
	}
	return new Float32Array(values);
}

// colorTarget is written at the same compacted index as matrixTarget so a culled
// instance never leaves the two out of step.
export function composeMapObjectPortalMatrices(
	baked: Float32Array,
	cmToMerc: number,
	cameraWorldX: number,
	cameraWorldY: number,
	minCullCm: number,
	matrixTarget: Float32Array,
	colorTarget: THREE.InstancedBufferAttribute,
	offset: number
): number {
	let written = 0;

	for (let o = 0; o < baked.length; o += PORTAL_BAKED_STRIDE) {
		const dx = baked[o + 3] - cameraWorldX;
		const dy = baked[o + 4] - cameraWorldY;
		const cull = Math.max(baked[o + 5], minCullCm);
		if (dx * dx + dy * dy > cull * cull) continue;

		const i = offset + written;
		const scale = baked[o + 6] * cmToMerc;
		const t = i * 16;
		matrixTarget[t] = scale;
		matrixTarget[t + 1] = 0;
		matrixTarget[t + 2] = 0;
		matrixTarget[t + 3] = 0;
		matrixTarget[t + 4] = 0;
		matrixTarget[t + 5] = scale;
		matrixTarget[t + 6] = 0;
		matrixTarget[t + 7] = 0;
		matrixTarget[t + 8] = 0;
		matrixTarget[t + 9] = 0;
		matrixTarget[t + 10] = scale;
		matrixTarget[t + 11] = 0;
		matrixTarget[t + 12] = baked[o];
		matrixTarget[t + 13] = baked[o + 1];
		matrixTarget[t + 14] = cmToMerc * baked[o + 2];
		matrixTarget[t + 15] = 1;
		colorTarget.setXYZ(i, baked[o + 7], baked[o + 8], baked[o + 9]);
		written++;
	}
	return written;
}

export type MapObjectLayer = CustomLayerInterface & {
	update(items: MapObjectItem[], area: MapArea, verticalScale: number): void;
	// Public so a test can pin that moving the camera never re-bakes.
	compose(): void;
	bakeCount(): number;
	composeCount(): number;
	instanceCount(): number;
	// Test-only. Only the linear 3x3 is meaningful.
	bakedMatrixFor(index: number): THREE.Matrix4;
	dispose(): void;
	// Test-only: composing needs no GL context, so tests attach a stub map
	// directly rather than going through onAdd.
	attachMapForTest(map: MLMap): void;
	groupsForTest(): THREE.InstancedMesh[];
};

function sameItems(a: MapObjectItem[], b: MapObjectItem[]): boolean {
	if (a.length !== b.length) return false;
	for (let i = 0; i < a.length; i++) {
		const p = a[i];
		const q = b[i];
		if (p === q) continue;
		if (
			p.x !== q.x ||
			p.y !== q.y ||
			p.z !== q.z ||
			p.actorClass !== q.actorClass ||
			p.scale !== q.scale ||
			p.portalColor !== q.portalColor ||
			p.ringRadiusCm !== q.ringRadiusCm ||
			p.rot[0] !== q.rot[0] ||
			p.rot[1] !== q.rot[1] ||
			p.rot[2] !== q.rot[2]
		)
			return false;
	}
	return true;
}

export function createMapObjectLayer(id: string): MapObjectLayer {
	const scene = new THREE.Scene();
	const camera = new THREE.Camera();
	let renderer: THREE.WebGLRenderer | null = null;
	let map: MLMap | null = null;
	let items: MapObjectItem[] = [];
	let area: MapArea = 'MainMap';
	let verticalScale = 1;
	let disposed = false;
	let composeQueued = false;

	let bakes = 0;
	let composes = 0;
	let baked = new Map<string, Float32Array>();
	let bakedPortalsByRadius = new Map<number, Float32Array>();

	const meshObjects = new Map<string, THREE.InstancedMesh>();
	// Bucketed by ring radius: fast travel and relic items share this path but
	// their rings differ in size, so their beams can't be one mesh.
	const portalMeshes = new Map<number, THREE.InstancedMesh>();
	let lastCmToMerc = Number.NaN;
	let lastCameraX = Number.NaN;
	let lastCameraY = Number.NaN;
	let lastViewCm = Number.NaN;
	// Forces the next compose past the early-out for changes the camera
	// comparison cannot see (a mesh finishing loading, a new item list).
	let dirty = true;

	// Matches palLayer's lighting so a relic and a Pal in the same frame sit in
	// the same tonal space.
	scene.add(new THREE.AmbientLight(0xffffff, 2.2));
	const hemi = new THREE.HemisphereLight(0xffffff, 0x8899aa, 1.2);
	hemi.position.set(0, 0, 1);
	scene.add(hemi);
	const sun = new THREE.DirectionalLight(0xffffff, 1.4);
	// Up is +z in mercator, not three's default +y, so the key light leads with z
	// to arrive overhead rather than side-on.
	sun.position.set(0.3, -0.5, 1);
	scene.add(sun);

	function releaseMesh(inst: THREE.InstancedMesh) {
		scene.remove(inst);
		// Geometry and material both belong to mapObjectMeshLibrary's shared cache;
		// disposing either here would free GPU resources the cache keeps handing
		// out. Only the instance-matrix buffer belongs to us.
		inst.dispose();
	}

	function releasePortalMesh(radiusCm: number) {
		const mesh = portalMeshes.get(radiusCm);
		if (!mesh) return;
		scene.remove(mesh);
		disposeMapObjectPortalMesh(mesh);
		portalMeshes.delete(radiusCm);
	}

	function releaseAllPortalMeshes() {
		for (const radiusCm of [...portalMeshes.keys()]) releasePortalMesh(radiusCm);
	}

	function clearGroups() {
		for (const inst of meshObjects.values()) releaseMesh(inst);
		meshObjects.clear();
		releaseAllPortalMeshes();
		lastCmToMerc = Number.NaN;
		lastCameraX = Number.NaN;
		lastCameraY = Number.NaN;
		lastViewCm = Number.NaN;
		dirty = true;
	}

	function itemsByRingRadius(source: MapObjectItem[]): Map<number, MapObjectItem[]> {
		const byRadius = new Map<number, MapObjectItem[]>();
		for (const item of source) {
			const radiusCm = item.ringRadiusCm;
			let bucket = byRadius.get(radiusCm);
			if (!bucket) {
				bucket = [];
				byRadius.set(radiusCm, bucket);
			}
			bucket.push(item);
		}
		return byRadius;
	}

	function bake() {
		bakes++;
		baked = bakeMapObjectInstances(items, area);
		bakedPortalsByRadius = new Map();
		for (const [radiusCm, group] of itemsByRingRadius(items)) {
			bakedPortalsByRadius.set(radiusCm, bakeMapObjectPortalInstances(group, area));
		}
		dirty = true;
	}

	// Allocates with headroom so one more item of the same mesh refills the
	// existing buffer instead of reallocating it.
	function meshFor(
		mesh: string,
		bundle: MapObjectMeshBundle,
		capacity: number
	): THREE.InstancedMesh {
		const existing = meshObjects.get(mesh);
		if (
			existing &&
			existing.geometry === bundle.geometry &&
			existing.instanceMatrix.count >= capacity
		) {
			return existing;
		}
		if (existing) releaseMesh(existing);
		const inst = new THREE.InstancedMesh(bundle.geometry, bundle.material, Math.ceil(capacity * 1.25) + 1);
		inst.frustumCulled = false;
		scene.add(inst);
		meshObjects.set(mesh, inst);
		return inst;
	}

	function portalMeshFor(radiusCm: number, capacity: number): THREE.InstancedMesh {
		const existing = portalMeshes.get(radiusCm);
		if (existing && existing.instanceMatrix.count >= capacity) return existing;
		if (existing) releasePortalMesh(radiusCm);
		const mesh = createMapObjectPortalMesh(Math.ceil(capacity * 1.25) + 1, radiusCm);
		scene.add(mesh);
		portalMeshes.set(radiusCm, mesh);
		return mesh;
	}

	const layer: MapObjectLayer = {
		id,
		type: 'custom',
		renderingMode: '3d',

		onAdd(m, gl) {
			map = m;
			renderer = getSharedRenderer(m.getCanvas(), gl as WebGLRenderingContext);
		},

		// State is saved unconditionally so a bake is never lost to mounting order;
		// only the compose needs a map.
		update(nextItems, nextArea, nextScale) {
			const changed = nextArea !== area || !sameItems(items, nextItems);
			items = nextItems;
			area = nextArea;
			verticalScale = nextScale;
			if (changed) bake();
			if (!map || disposed) return;
			layer.compose();
		},

		compose() {
			composes++;
			if (!map || disposed) return;

			const center = map.getCenter();
			const merc = MercatorCoordinate.fromLngLat([center.lng, center.lat], 0);
			const cmToMerc = verticalScale * merc.meterInMercatorCoordinateUnits();
			const [cpx, cpy] = lngLatToPixel(center.lng, center.lat);
			const cameraWorld = pixelToWorld(cpx, cpy, area);
			const bounds = map.getBounds();
			const sw = bounds.getSouthWest();
			const ne = bounds.getNorthEast();
			const viewCm = viewRadiusCm(
				[sw.lng, sw.lat],
				[ne.lng, ne.lat],
				area,
				cameraWorld.worldX,
				cameraWorld.worldY
			);

			// The scene is a function of exactly these four things, and rotating in
			// place leaves all four untouched.
			if (
				!dirty &&
				cmToMerc === lastCmToMerc &&
				cameraWorld.worldX === lastCameraX &&
				cameraWorld.worldY === lastCameraY &&
				viewCm === lastViewCm
			) {
				return;
			}
			dirty = false;
			lastCmToMerc = cmToMerc;
			lastCameraX = cameraWorld.worldX;
			lastCameraY = cameraWorld.worldY;
			lastViewCm = viewCm;

			for (const [mesh, chunk] of baked) {
				const bundle = requestMapObjectMesh(mesh, MODEL_DIR);
				// Loading or failed; onMapObjectMeshLoaded requeues a compose. Leave
				// what is already on screen alone.
				if (!bundle) continue;

				const inst = meshFor(mesh, bundle, chunk.length / BAKED_STRIDE);
				const count = composeMapObjectMatrices(
					chunk,
					cmToMerc,
					cameraWorld.worldX,
					cameraWorld.worldY,
					viewCm,
					inst.instanceMatrix.array as Float32Array,
					0
				);
				inst.count = count;
				inst.visible = count > 0;
				inst.instanceMatrix.needsUpdate = true;
			}

			for (const [mesh, inst] of meshObjects) {
				if (baked.has(mesh)) continue;
				releaseMesh(inst);
				meshObjects.delete(mesh);
			}

			for (const [radiusCm, chunk] of bakedPortalsByRadius) {
				if (chunk.length === 0) continue;
				const beam = portalMeshFor(radiusCm, chunk.length / PORTAL_BAKED_STRIDE);
				const colorAttr = beam.geometry.getAttribute('aColor') as THREE.InstancedBufferAttribute;
				const count = composeMapObjectPortalMatrices(
					chunk,
					cmToMerc,
					cameraWorld.worldX,
					cameraWorld.worldY,
					viewCm,
					beam.instanceMatrix.array as Float32Array,
					colorAttr,
					0
				);
				beam.count = count;
				beam.visible = count > 0;
				beam.instanceMatrix.needsUpdate = true;
				colorAttr.needsUpdate = true;
			}
			for (const radiusCm of [...portalMeshes.keys()]) {
				const chunk = bakedPortalsByRadius.get(radiusCm);
				if (chunk && chunk.length > 0) continue;
				releasePortalMesh(radiusCm);
			}

			map.triggerRepaint();
		},

		render(_gl, args) {
			if (!renderer) return;
			camera.projectionMatrix = new THREE.Matrix4().fromArray(args.defaultProjectionData.mainMatrix);
			renderer.resetState();
			renderer.render(scene, camera);
		},

		bakeCount() {
			return bakes;
		},

		composeCount() {
			return composes;
		},

		instanceCount() {
			let total = 0;
			for (const chunk of baked.values()) total += chunk.length / BAKED_STRIDE;
			return total;
		},

		bakedMatrixFor(index) {
			let i = index;
			for (const chunk of baked.values()) {
				const count = chunk.length / BAKED_STRIDE;
				if (i < count) {
					const o = i * BAKED_STRIDE;
					return new THREE.Matrix4().set(
						chunk[o + 6], chunk[o + 9], chunk[o + 12], chunk[o + 15],
						chunk[o + 7], chunk[o + 10], chunk[o + 13], chunk[o + 16],
						chunk[o + 8], chunk[o + 11], chunk[o + 14], chunk[o + 17],
						0, 0, 0, 1
					);
				}
				i -= count;
			}
			throw new Error(`no baked instance at index ${index}`);
		},

		dispose() {
			disposed = true;
			unsubscribeMeshLoaded();
			clearGroups();
			baked = new Map();
			bakedPortalsByRadius = new Map();
			// Shared across layers: released here, never disposed.
			renderer = null;
			map = null;
		},

		attachMapForTest(m) {
			map = m;
		},

		groupsForTest() {
			return [...meshObjects.values(), ...portalMeshes.values()];
		}
	};

	// requestMapObjectMesh() can settle synchronously while compose() is still
	// iterating meshes; the microtask keeps it from re-entering a running
	// compose, and composeQueued coalesces a burst of settles into one.
	function queueCompose() {
		if (composeQueued || disposed) return;
		composeQueued = true;
		queueMicrotask(() => {
			composeQueued = false;
			dirty = true;
			layer.compose();
		});
	}

	const unsubscribeMeshLoaded = onMapObjectMeshLoaded(() => queueCompose());

	return layer;
}
