// Renders scenery instances as instanced three.js meshes. Positions are absolute
// world centimetres that already include terrain elevation, so unlike structures
// no DEM height is added.
//
// This layer owns no map subscription: update() re-derives the viewport-culled
// bucket set each call, so the caller must re-invoke it on camera move.
import { MercatorCoordinate, type CustomLayerInterface, type Map as MLMap } from 'maplibre-gl';
import * as THREE from 'three';
import { ueQuatToThree } from './coords3d';
import { lngLatToPixel, pixelToLngLat } from './mercator';
import { onMeshLoaded, requestMesh } from './meshLibrary';
import type { SceneryBucketData, SceneryRun, SceneryStream } from './sceneryFormat';
import {
	createSceneryMaterial,
	setSceneryMaterialMap,
	setSceneryMaterialOpacity
} from './sceneryMaterial';
import { mosaicTexture, type TintMosaic } from './sceneryTint';
import { MESH_FLIP, getSharedRenderer } from './structureLayer';
import { pixelToWorld, worldToPixel, type MapArea } from './utils';

const MODEL_DIR = '/models/scenery';

export type ViewBounds = { minX: number; minY: number; maxX: number; maxY: number };

// Bounds are world centimetres. Buckets are guaranteed to contain their own
// placements, so overlap-with-view is a sufficient visibility test.
export function selectBuckets(buckets: SceneryBucketData[], view: ViewBounds): number[] {
	const out: number[] = [];
	for (let i = 0; i < buckets.length; i++) {
		const b = buckets[i];
		if (b.maxX < view.minX || b.minX > view.maxX) continue;
		if (b.maxY < view.minY || b.minY > view.maxY) continue;
		out.push(i);
	}
	return out;
}

// The definition of the per-instance transform. rebuild() uses the faster
// bakeInstances/composeInstanceMatrices split instead, which the tests pin
// against this instance by instance -- keep the two in step.
export function sceneryInstanceMatrix(
	worldX: number,
	worldY: number,
	worldZ: number,
	quat: [number, number, number, number],
	scale: [number, number, number],
	area: MapArea,
	cmToMerc: number
): THREE.Matrix4 {
	const [px, py] = worldToPixel(worldX, worldY, area);
	const [lng, lat] = pixelToLngLat(px, py);
	// The vertical axis deliberately bypasses fromLngLat's altitude argument,
	// which would divide by this point's own latitude. MapLibre's terrain scales
	// every vertex in a frame by one factor derived from the camera centre's
	// latitude, and cmToMerc is that same factor -- so applying it directly keeps
	// scenery in the vertical space terrain uses. Going through the altitude
	// argument would apply both, correct only at the camera centre and drifting
	// everywhere else.
	const anchor = MercatorCoordinate.fromLngLat([lng, lat]);
	const anchorZ = worldZ * cmToMerc;
	const rotation = MESH_FLIP.clone().multiply(
		new THREE.Matrix4().makeRotationFromQuaternion(
			ueQuatToThree(quat[0], quat[1], quat[2], quat[3])
		)
	);
	// A dimensionless Scale3D multiplier applied on top of meshLibrary's
	// cm-per-unit geometry, in ghostLayer's (x, z, y) axis order.
	const scaleM = new THREE.Matrix4().makeScale(
		cmToMerc * scale[0],
		cmToMerc * scale[2],
		cmToMerc * scale[1]
	);
	return new THREE.Matrix4()
		.makeTranslation(anchor.x, anchor.y, anchorZ)
		.multiply(rotation)
		.multiply(scaleM);
}

// Below this on-screen diameter (CSS pixels) an instance is culled outright --
// a hard cut, not a fade, since a fade needs per-instance opacity.
export const SCENERY_MIN_PIXELS = 10;

const scaleColumn = new THREE.Vector3();

// On-screen diameter in CSS pixels of a sphere of `geometryRadius` under `matrix`.
// Takes the max of the three column lengths, not the average, so a tall thin
// spire isn't culled for being narrow. Reading scale off the columns keeps
// rotation and translation out of the estimate.
export function projectedPixelDiameter(
	matrix: THREE.Matrix4,
	geometryRadius: number,
	pixelsPerMercatorUnit: number
): number {
	scaleColumn.setFromMatrixColumn(matrix, 0);
	let maxScale = scaleColumn.length();
	scaleColumn.setFromMatrixColumn(matrix, 1);
	maxScale = Math.max(maxScale, scaleColumn.length());
	scaleColumn.setFromMatrixColumn(matrix, 2);
	maxScale = Math.max(maxScale, scaleColumn.length());
	return 2 * geometryRadius * maxScale * pixelsPerMercatorUnit;
}

// The screen-size cull rule. composeInstanceMatrices applies it in an
// equivalent pre-multiplied form.
export function meetsScreenSizeThreshold(diameterPx: number): boolean {
	return diameterPx >= SCENERY_MIN_PIXELS;
}

// One entry per distinct mesh index, so rebuild() allocates one InstancedMesh
// per mesh rather than one per (bucket x mesh) pair.
export function mergeRunsByMesh(
	stream: SceneryStream,
	bucketIndices: number[]
): Map<number, SceneryRun[]> {
	const out = new Map<number, SceneryRun[]>();
	for (const bucketIndex of bucketIndices) {
		const bucket = stream.buckets[bucketIndex];
		if (!bucket) continue;
		for (const run of bucket.runs) {
			let runs = out.get(run.meshIndex);
			if (!runs) {
				runs = [];
				out.set(run.meshIndex, runs);
			}
			runs.push(run);
		}
	}
	return out;
}

// Everything in sceneryInstanceMatrix except cmToMerc depends only on the stream
// and the area, so it is computed once and reused for the life of the stream.
// Floats per instance:
//   0..1   mercator anchor x/y
//   2      world z, centimetres
//   3..11  rotation * base scale as a 3x3, column-major, at cmToMerc = 1
//   12     the longest of those three column lengths, for the screen-size cull
const BAKED_STRIDE = 13;

const bakeRotation = new THREE.Matrix4();

export function bakeInstances(runs: SceneryRun[], area: MapArea): Float32Array {
	let total = 0;
	for (const run of runs) total += run.count;
	const baked = new Float32Array(total * BAKED_STRIDE);

	let o = 0;
	for (const run of runs) {
		for (let i = 0; i < run.count; i++) {
			const [px, py] = worldToPixel(run.positions[i * 3], run.positions[i * 3 + 1], area);
			const [lng, lat] = pixelToLngLat(px, py);
			const anchor = MercatorCoordinate.fromLngLat([lng, lat]);

			bakeRotation.makeRotationFromQuaternion(
				ueQuatToThree(
					run.quats[i * 4],
					run.quats[i * 4 + 1],
					run.quats[i * 4 + 2],
					run.quats[i * 4 + 3]
				)
			);
			bakeRotation.premultiply(MESH_FLIP);
			const r = bakeRotation.elements;

			// three.js-space scale diagonal, in sceneryInstanceMatrix's (x, z, y) order.
			const scaleX = run.scales[i * 3];
			const scaleY = run.scales[i * 3 + 2];
			const scaleZ = run.scales[i * 3 + 1];

			baked[o] = anchor.x;
			baked[o + 1] = anchor.y;
			baked[o + 2] = run.positions[i * 3 + 2];
			baked[o + 3] = r[0] * scaleX;
			baked[o + 4] = r[1] * scaleX;
			baked[o + 5] = r[2] * scaleX;
			baked[o + 6] = r[4] * scaleY;
			baked[o + 7] = r[5] * scaleY;
			baked[o + 8] = r[6] * scaleY;
			baked[o + 9] = r[8] * scaleZ;
			baked[o + 10] = r[9] * scaleZ;
			baked[o + 11] = r[10] * scaleZ;
			baked[o + 12] = Math.max(
				Math.hypot(baked[o + 3], baked[o + 4], baked[o + 5]),
				Math.hypot(baked[o + 6], baked[o + 7], baked[o + 8]),
				Math.hypot(baked[o + 9], baked[o + 10], baked[o + 11])
			);
			o += BAKED_STRIDE;
		}
	}
	return baked;
}

// Applies the frame's cmToMerc to baked instances and appends those passing the
// screen-size cull to `target` at instance `offset`, returning how many were
// written. Culls by the same rule as meetsScreenSizeThreshold: scaling the 3x3
// by cmToMerc scales each column length by it, so the longest baked column times
// cmToMerc is the matrix's largest column length.
export function composeInstanceMatrices(
	baked: Float32Array,
	geometryRadius: number,
	cmToMerc: number,
	pixelsPerMercatorUnit: number,
	target: Float32Array,
	offset: number
): number {
	const minScale =
		SCENERY_MIN_PIXELS / (2 * geometryRadius * Math.abs(cmToMerc) * pixelsPerMercatorUnit);
	let written = 0;

	for (let o = 0; o < baked.length; o += BAKED_STRIDE) {
		if (baked[o + 12] < minScale) continue;

		let t = (offset + written) * 16;
		target[t] = cmToMerc * baked[o + 3];
		target[t + 1] = cmToMerc * baked[o + 4];
		target[t + 2] = cmToMerc * baked[o + 5];
		target[t + 3] = 0;
		target[t + 4] = cmToMerc * baked[o + 6];
		target[t + 5] = cmToMerc * baked[o + 7];
		target[t + 6] = cmToMerc * baked[o + 8];
		target[t + 7] = 0;
		target[t + 8] = cmToMerc * baked[o + 9];
		target[t + 9] = cmToMerc * baked[o + 10];
		target[t + 10] = cmToMerc * baked[o + 11];
		target[t + 11] = 0;
		target[t + 12] = baked[o];
		target[t + 13] = baked[o + 1];
		target[t + 14] = cmToMerc * baked[o + 2];
		target[t + 15] = 1;
		written++;
	}
	return written;
}

// cmToMerc follows the camera's centre latitude, so most frames of a pan shift it
// by a fraction of a percent -- enough to invalidate every cached matrix, far too
// little to see. A new value is adopted only past this relative threshold.
export const SCENERY_SCALAR_EPSILON = 1e-3;

export function stabilizeScalar(
	next: number,
	current: number,
	epsilon = SCENERY_SCALAR_EPSILON
): number {
	if (!Number.isFinite(current) || current === 0) return next;
	return Math.abs(next - current) <= Math.abs(current) * epsilon ? current : next;
}

// Fallback for the shader's uBase uniform wherever no map texture is bound.
export const SCENERY_BASE_COLOR = 0x8a8578;

export type SceneryLayer = CustomLayerInterface & {
	update(stream: SceneryStream, area: MapArea, verticalScale: number): void;
	// Swaps the material's map texture in place; instances re-sample it on the
	// next paint, so no InstancedMesh rebuild is needed.
	setTint(mosaic: TintMosaic | null): void;
	setOpacity(opacity: number): void;
	dispose(): void;
	visibleBucketsForTest(bounds: ViewBounds): number[];
};

export function createSceneryLayer(opts: { id: string }): SceneryLayer {
	const scene = new THREE.Scene();
	const camera = new THREE.Camera();
	let renderer: THREE.WebGLRenderer | null = null;
	let map: MLMap | null = null;
	let stream: SceneryStream | null = null;
	let area: MapArea = 'MainMap';
	let verticalScale = 1;
	let tint: TintMosaic | null = null;
	let tintTexture: THREE.Texture | null = null;
	let disposed = false;
	let rebuildQueued = false;

	// One InstancedMesh per distinct mesh index, refilled in place across frames
	// rather than reallocated.
	const meshObjects = new Map<number, THREE.InstancedMesh>();
	// Camera-independent per-bucket caches, valid for the life of the stream:
	// runs grouped by mesh, and baked instance data keyed `${bucket}:${mesh}`.
	const bucketRuns = new Map<number, Map<number, SceneryRun[]>>();
	const bakedChunks = new Map<string, Float32Array>();
	// The camera state meshObjects were last composed for.
	let lastCmToMerc = Number.NaN;
	let lastPixelsPerMercatorUnit = Number.NaN;
	let lastBucketIndices: number[] = [];
	// Forces the next rebuild past the early-out for changes the camera
	// comparison cannot see (a mesh finishing loading, a new stream).
	let dirty = true;

	const material = createSceneryMaterial();
	material.uniforms.uBase.value = new THREE.Color(SCENERY_BASE_COLOR);

	function releaseMesh(inst: THREE.InstancedMesh) {
		scene.remove(inst);
		// Geometry belongs to meshLibrary's shared cache -- disposing it here would
		// free its GPU buffers while the object stays cached, so a later cache hit
		// would render nothing. Only the instance-matrix buffer belongs to us.
		inst.dispose();
	}

	function clearGroups() {
		for (const inst of meshObjects.values()) releaseMesh(inst);
		meshObjects.clear();
		lastBucketIndices = [];
		lastCmToMerc = Number.NaN;
		lastPixelsPerMercatorUnit = Number.NaN;
		dirty = true;
	}

	// World-cm view rectangle for the current camera. Corners are min/maxed rather
	// than assumed ordered, since the game's map axes don't match lng/lat directly.
	function currentViewBounds(): ViewBounds | null {
		if (!map) return null;
		const bounds = map.getBounds();
		const sw = bounds.getSouthWest();
		const ne = bounds.getNorthEast();
		const [pxSW, pySW] = lngLatToPixel(sw.lng, sw.lat);
		const [pxNE, pyNE] = lngLatToPixel(ne.lng, ne.lat);
		const wSW = pixelToWorld(pxSW, pySW, area);
		const wNE = pixelToWorld(pxNE, pyNE, area);
		return {
			minX: Math.min(wSW.worldX, wNE.worldX),
			maxX: Math.max(wSW.worldX, wNE.worldX),
			minY: Math.min(wSW.worldY, wNE.worldY),
			maxY: Math.max(wSW.worldY, wNE.worldY)
		};
	}

	function runsFor(bucketIndex: number): Map<number, SceneryRun[]> {
		let grouped = bucketRuns.get(bucketIndex);
		if (!grouped) {
			grouped = mergeRunsByMesh(stream!, [bucketIndex]);
			bucketRuns.set(bucketIndex, grouped);
		}
		return grouped;
	}

	function bakedFor(bucketIndex: number, meshIndex: number): Float32Array {
		const key = `${bucketIndex}:${meshIndex}`;
		let chunk = bakedChunks.get(key);
		if (!chunk) {
			chunk = bakeInstances(runsFor(bucketIndex).get(meshIndex) ?? [], area);
			bakedChunks.set(key, chunk);
		}
		return chunk;
	}

	// Allocates with headroom so a pan bringing one more bucket of the same mesh
	// into view refills the existing buffer instead of reallocating it.
	function meshFor(
		meshIndex: number,
		geometry: THREE.BufferGeometry,
		capacity: number
	): THREE.InstancedMesh {
		const existing = meshObjects.get(meshIndex);
		if (existing && existing.geometry === geometry && existing.instanceMatrix.count >= capacity) {
			return existing;
		}
		if (existing) releaseMesh(existing);
		const inst = new THREE.InstancedMesh(geometry, material, Math.ceil(capacity * 1.25) + 1);
		inst.frustumCulled = false;
		scene.add(inst);
		meshObjects.set(meshIndex, inst);
		return inst;
	}

	function sameBuckets(a: number[], b: number[]): boolean {
		if (a.length !== b.length) return false;
		for (let i = 0; i < a.length; i++) if (a[i] !== b[i]) return false;
		return true;
	}

	function rebuild() {
		if (!stream || !map || disposed) return;

		const center = map.getCenter();
		const merc = MercatorCoordinate.fromLngLat([center.lng, center.lat], 0);
		const cmToMerc = stabilizeScalar(
			verticalScale * merc.meterInMercatorCoordinateUnits(),
			lastCmToMerc
		);
		// Read fresh rather than cached: worldSize changes with zoom.
		const pixelsPerMercatorUnit = stabilizeScalar(
			map.transform.worldSize,
			lastPixelsPerMercatorUnit
		);

		const view = currentViewBounds();
		const bucketIndices = view
			? selectBuckets(stream.buckets, view)
			: stream.buckets.map((_, i) => i);

		// The scene is a function of exactly these three things. MapLibre calls this
		// every frame of a camera move, but panning within one bucket, and rotating
		// or tilting at all, leave all three untouched.
		if (
			!dirty &&
			cmToMerc === lastCmToMerc &&
			pixelsPerMercatorUnit === lastPixelsPerMercatorUnit &&
			sameBuckets(bucketIndices, lastBucketIndices)
		) {
			return;
		}
		dirty = false;
		lastCmToMerc = cmToMerc;
		lastPixelsPerMercatorUnit = pixelsPerMercatorUnit;
		lastBucketIndices = bucketIndices;

		// Which visible buckets contribute to each mesh, so the draw-call count
		// stays per distinct mesh rather than per (bucket x mesh) pair.
		const bucketsByMesh = new Map<number, number[]>();
		for (const bucketIndex of bucketIndices) {
			if (!stream.buckets[bucketIndex]) continue;
			for (const meshIndex of runsFor(bucketIndex).keys()) {
				const list = bucketsByMesh.get(meshIndex);
				if (list) list.push(bucketIndex);
				else bucketsByMesh.set(meshIndex, [bucketIndex]);
			}
		}

		for (const [meshIndex, buckets] of bucketsByMesh) {
			const name = stream.meshes[meshIndex];
			const geometry = requestMesh(name, MODEL_DIR);
			// Loading or failed; onMeshLoaded requeues a rebuild. Leave what is
			// already on screen alone.
			if (!geometry) continue;

			if (!geometry.boundingSphere) geometry.computeBoundingSphere();
			const geometryRadius = geometry.boundingSphere!.radius;

			let capacity = 0;
			for (const bucketIndex of buckets) {
				capacity += bakedFor(bucketIndex, meshIndex).length / BAKED_STRIDE;
			}

			const inst = meshFor(meshIndex, geometry, capacity);
			const target = inst.instanceMatrix.array as Float32Array;
			let count = 0;
			for (const bucketIndex of buckets) {
				count += composeInstanceMatrices(
					bakedFor(bucketIndex, meshIndex),
					geometryRadius,
					cmToMerc,
					pixelsPerMercatorUnit,
					target,
					count
				);
			}
			inst.count = count;
			inst.visible = count > 0;
			inst.instanceMatrix.needsUpdate = true;
		}

		for (const [meshIndex, inst] of meshObjects) {
			if (bucketsByMesh.has(meshIndex)) continue;
			releaseMesh(inst);
			meshObjects.delete(meshIndex);
		}

		map.triggerRepaint();
	}

	const layer: SceneryLayer = {
		id: opts.id,
		type: 'custom',
		renderingMode: '3d',

		onAdd(m, gl) {
			map = m;
			renderer = getSharedRenderer(m.getCanvas(), gl as WebGLRenderingContext);
		},

		// State is saved unconditionally, not gated on `map`, so
		// visibleBucketsForTest works without a live GL context.
		update(next, nextArea, nextScale) {
			// The caches depend on the stream and area and nothing else --
			// verticalScale reaches the scene only via cmToMerc, reapplied per compose.
			if (next !== stream || nextArea !== area) {
				bucketRuns.clear();
				bakedChunks.clear();
				clearGroups();
			}
			stream = next;
			area = nextArea;
			verticalScale = nextScale;
			if (!map || disposed) return;
			rebuild();
		},

		setTint(mosaic) {
			if (mosaic === tint) return;
			tint = mosaic;
			const previousTexture = tintTexture;
			tintTexture = mosaic ? mosaicTexture(mosaic) : null;
			setSceneryMaterialMap(material, tintTexture);
			// A 2048x2048 RGBA texture is 16 MB of VRAM; without this, one leaks per
			// area switch.
			previousTexture?.dispose();
			map?.triggerRepaint();
		},

		setOpacity(opacity) {
			setSceneryMaterialOpacity(material, opacity);
			map?.triggerRepaint();
		},

		render(_gl, args) {
			if (!renderer) return;
			camera.projectionMatrix = new THREE.Matrix4().fromArray(
				args.defaultProjectionData.mainMatrix
			);
			renderer.resetState();
			renderer.render(scene, camera);
		},

		dispose() {
			disposed = true;
			unsubscribeMeshLoaded();
			clearGroups();
			bucketRuns.clear();
			bakedChunks.clear();
			material.dispose();
			tintTexture?.dispose();
			// Shared across layers: released here, never disposed.
			renderer = null;
			map = null;
		},

		visibleBucketsForTest(bounds) {
			return stream ? selectBuckets(stream.buckets, bounds) : [];
		}
	};

	// requestMesh() can settle synchronously while rebuild() is still iterating
	// buckets; the microtask keeps it from re-entering a running rebuild, and
	// rebuildQueued coalesces a burst of settles into one.
	function queueRebuild() {
		if (rebuildQueued || disposed) return;
		rebuildQueued = true;
		queueMicrotask(() => {
			rebuildQueued = false;
			dirty = true;
			rebuild();
		});
	}

	const unsubscribeMeshLoaded = onMeshLoaded(() => queueRebuild(), MODEL_DIR);

	return layer;
}
