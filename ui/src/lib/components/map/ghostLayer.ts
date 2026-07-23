// A MapLibre CustomLayerInterface that renders a blueprint's structures as a
// translucent "ghost" preview while the user positions it. It reuses
// structureLayer's shared three.js renderer, scene/camera setup, MESH_FLIP, and
// per-instance transform recipe (meshInstanceMatrix) verbatim -- the only new
// pieces are: absolute world transforms composed from the live anchor
// (composeWorld), full-quaternion rotation (gap 1), per-structure scale (gap 2),
// and a translucent material. Picking/hover/colour and the proxy fallback are
// intentionally absent: a ghost is a transient, non-interactive preview.
import * as THREE from 'three';
import { MercatorCoordinate, type CustomLayerInterface, type Map as MLMap } from 'maplibre-gl';
import type { BlueprintStructureGeometry, PlacementAnchor, Quat } from '$types';
import type { MapArea } from './utils';
import { worldToPixel } from './utils';
import { pixelToLngLat } from './mercator';
import { composeWorld } from './ghostTransform';
import { MESH_FLIP, getSharedRenderer } from './structureLayer';
import { structureParts, requestMesh, onMeshLoaded } from './meshLibrary';
import { partLocalMatrix } from './meshPlacement';

// Gap 1: the real render path only carries yaw (ueYawToThreeQuaternion), but a
// blueprint records a full quaternion. This mirrors that converter's convention
// for the whole quaternion: the ue.(x,y,z) -> three.(x,z,y) axis swap with the
// handedness flip. Concretely it is the same component map ueEulerToThreeQuaternion
// implements (ue roll->+X, ue pitch->+Z, ue yaw->-Y), so a yaw-only quat reduces
// exactly to ueYawToThreeQuaternion and pitched/rolled parts stay consistent with
// partLocalMatrix. Verified single-axis against coords3d/meshPlacement conventions.
function ueQuatToThree(q: Quat): THREE.Quaternion {
	return new THREE.Quaternion(q.x, -q.z, q.y, q.w);
}

type MeshBucket = { mesh: string; matrices: THREE.Matrix4[] };

// Same recipe as structureLayer.meshInstanceMatrix, but fed an absolute world
// transform (composeWorld) instead of a BaseStructure, honouring the full
// quaternion and per-structure scale.
function ghostInstanceMatrix(
	world: { translation: { x: number; y: number; z: number }; rotation: Quat; scale: { x: number; y: number; z: number } },
	part: Parameters<typeof partLocalMatrix>[0],
	area: MapArea,
	verticalScale: number,
	cmToMerc: number
): THREE.Matrix4 {
	const [px, py] = worldToPixel(world.translation.x, world.translation.y, area);
	const [lng, lat] = pixelToLngLat(px, py);
	const anchor = MercatorCoordinate.fromLngLat([lng, lat], world.translation.z * verticalScale);
	const rotation = MESH_FLIP.clone().multiply(
		new THREE.Matrix4().makeRotationFromQuaternion(ueQuatToThree(world.rotation))
	);
	// Gap 2: per-structure scale, in the same (x,z,y) axis order partLocalMatrix
	// uses for the part's own scale, folded into the cm->mercator conversion.
	const s = world.scale;
	const scale = new THREE.Matrix4().makeScale(cmToMerc * s.x, cmToMerc * s.z, cmToMerc * s.y);
	return new THREE.Matrix4()
		.makeTranslation(anchor.x, anchor.y, anchor.z)
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
};

export function createGhostLayer(opts: { id: string }): GhostLayer {
	const scene = new THREE.Scene();
	const camera = new THREE.Camera();
	let renderer: THREE.WebGLRenderer | null = null;
	let map: MLMap | null = null;
	const groups: THREE.InstancedMesh[] = [];
	let disposed = false;

	let lastArgs: Parameters<GhostLayer['update']> | null = null;
	let rebuildQueued = false;

	scene.add(new THREE.AmbientLight(0xffffff, 0.7));
	const dir = new THREE.DirectionalLight(0xffffff, 0.9);
	dir.position.set(0.5, 1, 0.3);
	scene.add(dir);

	function clearGroups() {
		for (const inst of groups) {
			scene.remove(inst);
			// Geometry lives in meshLibrary's module cache shared with the real layer;
			// only the per-update material and instance buffers belong to us.
			(inst.material as THREE.Material).dispose();
			inst.dispose();
		}
		groups.length = 0;
	}

	const layer: GhostLayer = {
		id: opts.id,
		type: 'custom',
		renderingMode: '3d',

		onAdd(m, gl) {
			map = m;
			renderer = getSharedRenderer(m.getCanvas(), gl as WebGLRenderingContext);
		},

		update(geometry, anchor, area, verticalScale) {
			lastArgs = [geometry, anchor, area, verticalScale];
			if (!map || disposed) return;
			clearGroups();

			const center = map.getCenter();
			const merc = MercatorCoordinate.fromLngLat([center.lng, center.lat], 0);
			const cmToMerc = verticalScale * merc.meterInMercatorCoordinateUnits();

			const buckets = new Map<string, MeshBucket>();

			for (const structure of geometry) {
				const world = composeWorld(anchor, {
					translation: structure.translation,
					rotation: structure.rotation,
					scale: structure.scale
				});
				const parts = structureParts(structure.map_object_id);
				if (!parts || parts.length === 0) continue;

				for (const part of parts) {
					const geom = requestMesh(part.mesh);
					if (!geom) continue; // loading or failed; onMeshLoaded requeues a rebuild
					let bucket = buckets.get(part.mesh);
					if (!bucket) {
						bucket = { mesh: part.mesh, matrices: [] };
						buckets.set(part.mesh, bucket);
					}
					bucket.matrices.push(
						ghostInstanceMatrix(world, part, area, verticalScale, cmToMerc)
					);
				}
			}

			for (const bucket of buckets.values()) {
				const geom = requestMesh(bucket.mesh);
				if (!geom) continue;
				const material = new THREE.MeshLambertMaterial({
					color: 0x88ccff,
					side: THREE.DoubleSide
				});
				const inst = new THREE.InstancedMesh(geom, material, bucket.matrices.length);
				inst.frustumCulled = false;
				bucket.matrices.forEach((mtx, i) => inst.setMatrixAt(i, mtx));
				inst.instanceMatrix.needsUpdate = true;
				scene.add(inst);
				groups.push(inst);
			}

			map.triggerRepaint();
		},

		render(_gl, args) {
			if (!renderer) return;
			const m = new THREE.Matrix4().fromArray(args.defaultProjectionData.mainMatrix);
			camera.projectionMatrix = m;
			renderer.resetState();
			// MapLibre's shared depth buffer isn't cleared for us; without this the
			// ghost inherits stale depth state and self-occludes incorrectly.
			renderer.clearDepth();
			renderer.render(scene, camera);
		},

		dispose() {
			disposed = true;
			unsubscribeMeshLoaded();
			clearGroups();
			// renderer is structureLayer's module-level shared renderer -- released, not
			// disposed (its GL context and cached buffers outlive this layer).
			renderer = null;
		}
	};

	// A mesh can finish loading after update() returns; rebuild so it appears,
	// coalescing a burst of settles into one rebuild (mirrors structureLayer).
	const unsubscribeMeshLoaded = onMeshLoaded(() => {
		if (rebuildQueued || disposed) return;
		rebuildQueued = true;
		queueMicrotask(() => {
			rebuildQueued = false;
			if (lastArgs && !disposed) layer.update(...lastArgs);
		});
	});

	return layer;
}
