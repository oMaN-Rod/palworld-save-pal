// Reuses structureLayer's shared three.js renderer, scene/camera setup, MESH_FLIP,
// and per-instance transform recipe verbatim; picking/hover/colour and the proxy
// fallback are intentionally absent since a ghost is a transient, non-interactive preview.
//
// update() is driven by pointer-rate and frame-rate inputs -- a drag, the yaw and
// height sliders, and verticalScale, which changes on every pan frame -- so it must
// never be a teardown and rebuild. It records what was asked for, coalesces onto one
// animation frame, and does the least that answers the change: groups and materials
// survive an anchor move, and a camera-only move touches nothing but the baked
// matrices (see ghostInstances).
import * as THREE from 'three';
import { MercatorCoordinate, type CustomLayerInterface, type Map as MLMap } from 'maplibre-gl';
import type { BlueprintStructureGeometry, PlacementAnchor, Quat } from '$types';
import type { MapArea } from '../../geo/utils';
import { worldToPixel } from '../../geo/utils';
import { pixelToLngLat } from '../../geo/mercator';
import { composeWorldInto, emptyWorldTransform } from './ghostTransform';
import { MESH_FLIP, getSharedRenderer } from '../structures/structureLayer';
import { ueQuatToThree } from '../../geo/coords3d';
import { structureParts, requestMesh, onMeshLoaded, STRUCTURE_MODEL_DIR } from './meshLibrary';
import { partLocalMatrix } from './meshPlacement';
import {
	bakeGhostInstance,
	composeGhostMatrix,
	GHOST_BAKE_STRIDE,
	type GhostBake
} from './ghostInstances';

// Same recipe as structureLayer.meshInstanceMatrix, but fed an absolute world
// transform (composeWorld) instead of a BaseStructure, honouring the full
// quaternion and per-structure scale. The layer itself goes through the bake/compose
// split in ghostInstances; this stays as that split's oracle and its only caller is
// the test that pins the two together.
export function ghostInstanceMatrix(
	world: { translation: { x: number; y: number; z: number }; rotation: Quat; scale: { x: number; y: number; z: number } },
	part: Parameters<typeof partLocalMatrix>[0],
	area: MapArea,
	_verticalScale: number,
	cmToMerc: number
): THREE.Matrix4 {
	const [px, py] = worldToPixel(world.translation.x, world.translation.y, area);
	const [lng, lat] = pixelToLngLat(px, py);
	// Z goes through cmToMerc directly, not fromLngLat's altitude argument, which
	// would divide by this instance's own latitude rather than the camera
	// centre's (see structureLayer.meshInstanceMatrix).
	const anchor = MercatorCoordinate.fromLngLat([lng, lat]);
	const anchorZ = world.translation.z * cmToMerc;
	const rotation = MESH_FLIP.clone().multiply(
		new THREE.Matrix4().makeRotationFromQuaternion(
			ueQuatToThree(world.rotation.x, world.rotation.y, world.rotation.z, world.rotation.w)
		)
	);
	// Gap 2: per-structure scale, in the same (x,z,y) axis order partLocalMatrix
	// uses for the part's own scale, folded into the cm->mercator conversion.
	const s = world.scale;
	const scale = new THREE.Matrix4().makeScale(cmToMerc * s.x, cmToMerc * s.z, cmToMerc * s.y);
	return new THREE.Matrix4()
		.makeTranslation(anchor.x, anchor.y, anchorZ)
		.multiply(rotation)
		.multiply(scale)
		.multiply(partLocalMatrix(part));
}

export type GhostLayer = CustomLayerInterface & {
	update(
		geometry: BlueprintStructureGeometry[],
		anchor: PlacementAnchor,
		area: MapArea,
		verticalScale: number
	): void;
	dispose(): void;
	// Test-only: onAdd needs a live GL context, and the work is deferred to an
	// animation frame a node suite has no way to wait for.
	attachMapForTest(map: MLMap): void;
	flushForTest(): void;
	groupsForTest(): { mesh: THREE.InstancedMesh; material: THREE.Material; count: number }[];
	// How often each tier of work has actually run. The whole point of this layer is
	// that the expensive tiers run rarely, which is only assertable by counting them.
	statsForTest(): { applies: number; rebuilds: number; bakes: number; composes: number };
};

type GhostGroup = {
	mesh: THREE.InstancedMesh;
	material: THREE.Material;
	// Parallel per-instance arrays; one entry per (structure, part) pair.
	structures: BlueprintStructureGeometry[];
	parts: Parameters<typeof partLocalMatrix>[0][];
	baked: GhostBake;
};

type Request = {
	geometry: BlueprintStructureGeometry[];
	anchor: PlacementAnchor;
	area: MapArea;
	verticalScale: number;
};

const useRaf = typeof requestAnimationFrame === 'function';
const schedule = useRaf
	? (cb: () => void) => requestAnimationFrame(cb)
	: (cb: () => void) => setTimeout(cb, 0) as unknown as number;
const unschedule = useRaf
	? (handle: number) => cancelAnimationFrame(handle)
	: (handle: number) => clearTimeout(handle as unknown as ReturnType<typeof setTimeout>);

function sameAnchor(a: PlacementAnchor, b: PlacementAnchor): boolean {
	return a.x === b.x && a.y === b.y && a.z === b.z && a.yaw === b.yaw;
}

export function createGhostLayer(opts: { id: string }): GhostLayer {
	const scene = new THREE.Scene();
	const camera = new THREE.Camera();
	let renderer: THREE.WebGLRenderer | null = null;
	let map: MLMap | null = null;
	const groups: GhostGroup[] = [];
	let disposed = false;

	// What the caller last asked for, and what is currently realised in `groups`.
	// The gap between them is exactly the work the next frame has to do.
	let requested: Request | null = null;
	let applied: Request | null = null;
	let meshesChanged = false;
	let frame: number | null = null;
	const stats = { applies: 0, rebuilds: 0, bakes: 0, composes: 0 };

	const scratchWorld = emptyWorldTransform();
	const scratchMatrix = new THREE.Matrix4();

	scene.add(new THREE.AmbientLight(0xffffff, 0.7));
	const dir = new THREE.DirectionalLight(0xffffff, 0.9);
	dir.position.set(0.5, 1, 0.3);
	scene.add(dir);

	function clearGroups() {
		for (const group of groups) {
			scene.remove(group.mesh);
			// Geometry lives in meshLibrary's module cache shared with the real layer;
			// only the per-update material and instance buffers belong to us.
			group.material.dispose();
			group.mesh.dispose();
		}
		groups.length = 0;
	}

	// Buckets the (structure, part) pairs whose mesh has already loaded, one
	// InstancedMesh per mesh. The only path that allocates GPU buffers, so it runs
	// only when the blueprint itself or the set of loaded meshes changes.
	function rebuildGroups(geometry: BlueprintStructureGeometry[]) {
		stats.rebuilds++;
		clearGroups();
		const buckets = new Map<string, Omit<GhostGroup, 'mesh' | 'material' | 'baked'>>();

		for (const structure of geometry) {
			const parts = structureParts(structure.map_object_id);
			if (!parts || parts.length === 0) continue;
			for (const part of parts) {
				// Loading or failed; the meshLibrary subscription requeues a rebuild.
				if (!requestMesh(part.mesh)) continue;
				let bucket = buckets.get(part.mesh);
				if (!bucket) {
					bucket = { structures: [], parts: [] };
					buckets.set(part.mesh, bucket);
				}
				bucket.structures.push(structure);
				bucket.parts.push(part);
			}
		}

		for (const [mesh, bucket] of buckets) {
			const geom = requestMesh(mesh);
			if (!geom) continue;
			const material = new THREE.MeshLambertMaterial({
				color: 0x88ccff,
				side: THREE.DoubleSide
			});
			const inst = new THREE.InstancedMesh(geom, material, bucket.structures.length);
			inst.frustumCulled = false;
			scene.add(inst);
			groups.push({
				mesh: inst,
				material,
				structures: bucket.structures,
				parts: bucket.parts,
				baked: new Float64Array(bucket.structures.length * GHOST_BAKE_STRIDE)
			});
		}
	}

	// Anchor- and area-dependent, camera-independent. A drag or a slider lands here.
	function bakeGroups(anchor: PlacementAnchor, area: MapArea) {
		stats.bakes++;
		for (const group of groups) {
			for (let i = 0; i < group.structures.length; i++) {
				composeWorldInto(anchor, group.structures[i], scratchWorld);
				bakeGhostInstance(scratchWorld, group.parts[i], area, group.baked, i * GHOST_BAKE_STRIDE);
			}
		}
	}

	// Camera-only. A pan lands here and nowhere else.
	function composeGroups(verticalScale: number) {
		if (!map) return;
		stats.composes++;
		const center = map.getCenter();
		const merc = MercatorCoordinate.fromLngLat([center.lng, center.lat], 0);
		const cmToMerc = verticalScale * merc.meterInMercatorCoordinateUnits();
		for (const group of groups) {
			for (let i = 0; i < group.structures.length; i++) {
				composeGhostMatrix(group.baked, i * GHOST_BAKE_STRIDE, cmToMerc, scratchMatrix);
				group.mesh.setMatrixAt(i, scratchMatrix);
			}
			group.mesh.instanceMatrix.needsUpdate = true;
		}
	}

	function apply() {
		if (!map || disposed || !requested) return;
		stats.applies++;
		const next = requested;
		const previous = applied;

		const rebuild = meshesChanged || !previous || previous.geometry !== next.geometry;
		const rebake =
			rebuild || previous.area !== next.area || !sameAnchor(previous.anchor, next.anchor);
		// cmToMerc is verticalScale times a function of the camera's own latitude, and
		// verticalScale is itself a function of that latitude, so an unchanged
		// verticalScale means an unchanged cmToMerc and identical matrices.
		const recompose = rebake || previous.verticalScale !== next.verticalScale;

		meshesChanged = false;
		applied = next;
		if (!recompose) return;

		if (rebuild) rebuildGroups(next.geometry);
		if (rebake) bakeGroups(next.anchor, next.area);
		composeGroups(next.verticalScale);
		map.triggerRepaint();
	}

	function queue() {
		if (frame !== null || disposed) return;
		frame = schedule(() => {
			frame = null;
			apply();
		});
	}

	const layer: GhostLayer = {
		id: opts.id,
		type: 'custom',
		renderingMode: '3d',

		onAdd(m, gl) {
			map = m;
			renderer = getSharedRenderer(m.getCanvas(), gl as WebGLRenderingContext);
			// update() may have run before the layer was mounted; nothing was applied then.
			if (requested) queue();
		},

		update(geometry, anchor, area, verticalScale) {
			requested = { geometry, anchor, area, verticalScale };
			queue();
		},

		render(_gl, args) {
			if (!renderer) return;
			const m = new THREE.Matrix4().fromArray(args.defaultProjectionData.mainMatrix);
			camera.projectionMatrix = m;
			renderer.resetState();
			renderer.render(scene, camera);
		},

		dispose() {
			disposed = true;
			unsubscribeMeshLoaded();
			if (frame !== null) unschedule(frame);
			frame = null;
			requested = null;
			applied = null;
			clearGroups();
			// renderer is structureLayer's module-level shared renderer -- released, not
			// disposed (its GL context and cached buffers outlive this layer).
			renderer = null;
		},

		attachMapForTest(m) {
			map = m;
		},

		flushForTest() {
			if (frame !== null) {
				unschedule(frame);
				frame = null;
			}
			apply();
		},

		statsForTest() {
			return { ...stats };
		},

		groupsForTest() {
			return groups.map((g) => ({
				mesh: g.mesh,
				material: g.material,
				count: g.structures.length
			}));
		}
	};

	// A mesh can finish loading after update() returns; rebuild so it appears.
	// Scoped to the structure mesh directory: the cache is shared with the scenery
	// layer, whose hundreds of meshes would otherwise each force a rebuild here.
	const unsubscribeMeshLoaded = onMeshLoaded(() => {
		if (disposed) return;
		meshesChanged = true;
		queue();
	}, STRUCTURE_MODEL_DIR);

	return layer;
}
