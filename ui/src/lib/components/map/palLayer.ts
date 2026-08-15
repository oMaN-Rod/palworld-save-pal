// Renders boss and alpha Pal spawn points as textured three.js meshes. With at
// most ~90 spawns map-wide and every model distinct, this places one Object3D
// per spawn rather than instancing -- there is nothing to instance. update() is
// caller-driven; the layer holds no camera listener of its own.
import * as THREE from 'three';
import { MercatorCoordinate, type CustomLayerInterface, type Map as MLMap } from 'maplibre-gl';
import type { PredatorSpawn } from '$types';
import { requestPalMesh, onPalMeshLoaded } from './palMeshLibrary';
import { MESH_FLIP, getSharedRenderer } from './structureLayer';
import { worldToPixel, type MapArea } from './utils';
import { pixelToLngLat } from './mercator';
import { ueYawToThreeQuaternion } from './coords3d';
import {
	createPortalMeshes,
	disposePortalMeshes,
	portalInstanceMatrix,
	portalIntensity,
	PORTAL_RADIUS_CM
} from './palPortal';
import {
	createMapObjectPortalMesh,
	disposeMapObjectPortalMesh,
	mapObjectPortalMatrix,
	palRingColor
} from './mapObjectPortal';
import { PAL_SCALE_DEFAULT } from './palSize';

export type PalBoss = { key: string; x: number; y: number; z: number; defeated: boolean };

// Predators have no "defeated" concept, so they resolve a model key straight
// from `pal` and get no portal-intensity dimming.
export type PalPredator = { key: string; x: number; y: number; z: number };

export function predatorPalBosses(
	predators: Pick<PredatorSpawn, 'x' | 'y' | 'z' | 'pal'>[]
): PalPredator[] {
	return predators.map((p) => ({ key: p.pal, x: p.x, y: p.y, z: p.z }));
}

export type PalDisplay = { scale: number; heightCm: number; autoFollow: boolean; xray: boolean };

// MESH_FLIP lands the models' forward axis on mercator +x (due east), which at
// bearing 0 reads as screen-right rather than toward the viewer. This quarter
// turn is the correction.
export const PAL_YAW_OFFSET = Math.PI / 2;

export function palInstanceMatrix(
	worldX: number,
	worldY: number,
	worldZ: number,
	bearingRad: number,
	area: MapArea,
	cmToMerc: number,
	scale: number,
	heightCm: number
): THREE.Matrix4 {
	const [px, py] = worldToPixel(worldX, worldY, area);
	const [lng, lat] = pixelToLngLat(px, py);
	// Altitude goes through cmToMerc directly, not fromLngLat's altitude argument,
	// which would divide by this point's own latitude rather than the camera
	// centre's (see sceneryLayer.sceneryInstanceMatrix).
	const anchor = MercatorCoordinate.fromLngLat([lng, lat]);
	const anchorZ = (worldZ + heightCm) * cmToMerc;
	const rotation = MESH_FLIP.clone().multiply(
		new THREE.Matrix4().makeRotationFromQuaternion(
			ueYawToThreeQuaternion(bearingRad + PAL_YAW_OFFSET)
		)
	);
	const scaleMatrix = new THREE.Matrix4().makeScale(
		scale * cmToMerc,
		scale * cmToMerc,
		scale * cmToMerc
	);
	return new THREE.Matrix4()
		.makeTranslation(anchor.x, anchor.y, anchorZ)
		.multiply(rotation)
		.multiply(scaleMatrix);
}

export type PalLayer = CustomLayerInterface & {
	update(
		bosses: PalBoss[],
		area: MapArea,
		verticalScale: number,
		display: PalDisplay,
		predators?: PalPredator[]
	): void;
	dispose(): void;
	// Test-only: group building needs no GL context, so tests attach a stub map
	// directly rather than going through onAdd.
	attachMapForTest(map: MLMap): void;
	groupsForTest(): THREE.Object3D[];
};

export function createPalLayer(opts: { id: string }): PalLayer {
	const scene = new THREE.Scene();
	const camera = new THREE.Camera();
	let renderer: THREE.WebGLRenderer | null = null;
	let map: MLMap | null = null;
	const groups: THREE.Object3D[] = [];
	// Parallel to `groups`: a boss whose mesh has not resolved yet produces no
	// group, so the two lists cannot be indexed by the same cursor as `bosses`.
	const groupBosses: PalBoss[] = [];
	let builtBosses: PalBoss[] | null = null;
	// Kept separate from `groups`/`groupBosses` rather than folded in: predators
	// carry no `defeated`, so sharing the list would push a missing field onto
	// every consumer instead of the two spots that actually differ.
	const predatorGroups: THREE.Object3D[] = [];
	const groupPredators: PalPredator[] = [];
	let builtPredators: PalPredator[] | null = null;
	let builtArea: MapArea | null = null;
	let structureDirty = true;
	let portals: { column: THREE.InstancedMesh } | null = null;
	// Uses mapObjectPortal's per-instance-colour mesh rather than palPortal's,
	// whose colours are one uniform for the whole InstancedMesh -- a red predator
	// ring cannot share that mesh with the blue boss ring.
	let predatorPortal: THREE.InstancedMesh | null = null;
	let disposed = false;

	let bosses: PalBoss[] = [];
	let predators: PalPredator[] = [];
	let area: MapArea = 'MainMap';
	let verticalScale = 1;
	let display: PalDisplay = {
		scale: PAL_SCALE_DEFAULT,
		heightCm: 0,
		autoFollow: true,
		xray: false
	};
	let rebuildQueued = false;

	scene.add(new THREE.AmbientLight(0xffffff, 2.2));
	const hemi = new THREE.HemisphereLight(0xffffff, 0x8899aa, 1.2);
	// HemisphereLight has no direction property; three takes its sky/ground blend
	// axis from position, which defaults to (0,1,0). Up is +z here, so the default
	// would blend sideways across the map plane instead of over a Pal's top.
	hemi.position.set(0, 0, 1);
	scene.add(hemi);
	const dir = new THREE.DirectionalLight(0xffffff, 1.4);
	// Up is +z in mercator, so the key light leads with z to arrive overhead.
	dir.position.set(0.3, -0.5, 1);
	scene.add(dir);

	function clearGroups() {
		for (const g of groups) scene.remove(g);
		groups.length = 0;
		groupBosses.length = 0;
		for (const g of predatorGroups) scene.remove(g);
		predatorGroups.length = 0;
		groupPredators.length = 0;
		if (portals) {
			scene.remove(portals.column);
			disposePortalMeshes(portals);
			portals = null;
		}
		if (predatorPortal) {
			scene.remove(predatorPortal);
			disposeMapObjectPortalMesh(predatorPortal);
			predatorPortal = null;
		}
	}

	function sameList<T extends { key: string; x: number; y: number; z: number }>(
		previous: T[] | null,
		next: T[]
	): boolean {
		if (!previous || previous.length !== next.length) return false;
		for (let i = 0; i < previous.length; i++) {
			const a = previous[i];
			const b = next[i];
			if (a === b) continue;
			if (a.key !== b.key || a.x !== b.x || a.y !== b.y || a.z !== b.z) return false;
		}
		return true;
	}

	// Everything a camera move changes lives in the transforms: cmToMerc and the
	// yaw Pals face. Splitting this out is what lets update() run per frame --
	// rebuilding the scene graph would clone an Object3D per boss every frame.
	function refreshTransforms() {
		if (!map) return;

		const center = map.getCenter();
		const merc = MercatorCoordinate.fromLngLat([center.lng, center.lat], 0);
		const cmToMerc = verticalScale * merc.meterInMercatorCoordinateUnits();
		const bearingRad = display.autoFollow ? (map.getBearing() * Math.PI) / 180 : 0;

		for (let i = 0; i < groups.length; i++) {
			const boss = groupBosses[i];
			groups[i].matrix.copy(
				palInstanceMatrix(
					boss.x, boss.y, boss.z, bearingRad, area, cmToMerc, display.scale, display.heightCm
				)
			);
		}

		for (let i = 0; i < predatorGroups.length; i++) {
			const predator = groupPredators[i];
			predatorGroups[i].matrix.copy(
				palInstanceMatrix(
					predator.x, predator.y, predator.z, bearingRad, area, cmToMerc, display.scale, display.heightCm
				)
			);
		}

		if (portals) {
			const intensityAttr = portals.column.geometry.getAttribute('aIntensity') as THREE.BufferAttribute;
			for (let i = 0; i < bosses.length; i++) {
				const b = bosses[i];
				portals.column.setMatrixAt(i, portalInstanceMatrix(b.x, b.y, b.z, area, cmToMerc, display.scale));
				intensityAttr.setX(i, portalIntensity(b.defeated));
			}
			portals.column.instanceMatrix.needsUpdate = true;
			intensityAttr.needsUpdate = true;
		}

		if (predatorPortal) {
			for (let i = 0; i < predators.length; i++) {
				const p = predators[i];
				predatorPortal.setMatrixAt(i, mapObjectPortalMatrix(p.x, p.y, p.z, area, cmToMerc, display.scale));
			}
			predatorPortal.instanceMatrix.needsUpdate = true;
		}

		map.triggerRepaint();
	}

	function rebuild() {
		if (!map || disposed) return;

		if (
			!structureDirty &&
			builtArea === area &&
			sameList(builtBosses, bosses) &&
			sameList(builtPredators, predators)
		) {
			refreshTransforms();
			return;
		}

		clearGroups();

		for (const boss of bosses) {
			const source = requestPalMesh(boss.key);
			if (!source) continue; // loading or failed; onPalMeshLoaded requeues a rebuild
			// requestPalMesh returns a shared cached Object3D, and an Object3D can
			// only have one parent -- without this clone a second boss sharing the
			// key ("grassgolem") would silently steal it from the first.
			const instance = source.clone();
			instance.matrixAutoUpdate = false;
			scene.add(instance);
			groups.push(instance);
			groupBosses.push(boss);
		}

		for (const predator of predators) {
			const source = requestPalMesh(predator.key);
			if (!source) continue;
			const instance = source.clone();
			instance.matrixAutoUpdate = false;
			scene.add(instance);
			predatorGroups.push(instance);
			groupPredators.push(predator);
		}

		if (bosses.length > 0) {
			portals = createPortalMeshes(bosses.length);
			scene.add(portals.column);
		}

		if (predators.length > 0) {
			predatorPortal = createMapObjectPortalMesh(predators.length, PORTAL_RADIUS_CM);
			const colorAttr = predatorPortal.geometry.getAttribute(
				'aColor'
			) as THREE.InstancedBufferAttribute;
			const { r, g, b } = palRingColor('predator');
			for (let i = 0; i < predators.length; i++) colorAttr.setXYZ(i, r, g, b);
			scene.add(predatorPortal);
		}

		builtBosses = bosses.slice();
		builtPredators = predators.slice();
		builtArea = area;
		structureDirty = false;

		refreshTransforms();
	}

	const layer: PalLayer = {
		id: opts.id,
		type: 'custom',
		renderingMode: '3d',

		onAdd(m, gl) {
			map = m;
			renderer = getSharedRenderer(m.getCanvas(), gl as WebGLRenderingContext);
		},

		update(nextBosses, nextArea, nextScale, nextDisplay, nextPredators = []) {
			bosses = nextBosses;
			predators = nextPredators;
			area = nextArea;
			verticalScale = nextScale;
			display = nextDisplay;
			if (!map || disposed) return;
			rebuild();
		},

		render(_gl, args) {
			if (!renderer) return;
			camera.projectionMatrix = new THREE.Matrix4().fromArray(args.defaultProjectionData.mainMatrix);
			renderer.resetState();
			// Discard depth already in the buffer so nothing drawn earlier occludes a
			// Pal. Depth testing stays on within this scene -- turning it off instead
			// makes far limbs paint over near ones. Skipped when there is nothing to
			// draw, so an empty scene never strips depth from the layers above.
			if (
				display.xray &&
				(groups.length > 0 || portals || predatorGroups.length > 0 || predatorPortal)
			)
				renderer.clearDepth();
			renderer.render(scene, camera);
		},

		dispose() {
			disposed = true;
			unsubscribePalMeshLoaded();
			clearGroups();
			// Shared across layers: released here, never disposed.
			renderer = null;
			map = null;
		},

		attachMapForTest(m) {
			map = m;
		},

		groupsForTest() {
			const all = [...groups, ...predatorGroups];
			if (portals) all.push(portals.column);
			if (predatorPortal) all.push(predatorPortal);
			return all;
		}
	};

	// requestPalMesh() can settle synchronously while rebuild() is still iterating
	// bosses; the microtask keeps it from re-entering a running rebuild, and
	// rebuildQueued coalesces a burst of settles into one.
	function queueRebuild() {
		if (rebuildQueued || disposed) return;
		// A key that resolved to nothing may resolve now, so the group list itself
		// has to be rebuilt rather than just re-transformed.
		structureDirty = true;
		rebuildQueued = true;
		queueMicrotask(() => {
			rebuildQueued = false;
			rebuild();
		});
	}

	const unsubscribePalMeshLoaded = onPalMeshLoaded(() => queueRebuild());

	return layer;
}
