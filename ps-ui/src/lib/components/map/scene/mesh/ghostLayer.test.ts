import { describe, it, expect, vi } from 'vitest';
import * as THREE from 'three';
import { MercatorCoordinate } from 'maplibre-gl';

// Mesh loads are network-bound; settle them by hand so the update-discipline
// tests below can drive a layer that has real groups.
const { loadCalls } = vi.hoisted(() => ({
	loadCalls: [] as Array<{ url: string; onLoad: (gltf: { scene: THREE.Object3D }) => void }>
}));

vi.mock('three/examples/jsm/loaders/GLTFLoader.js', () => ({
	GLTFLoader: vi.fn().mockImplementation(() => ({
		setDRACOLoader: vi.fn(),
		setMeshoptDecoder: vi.fn(),
		load: (url: string, onLoad: (gltf: { scene: THREE.Object3D }) => void) => {
			loadCalls.push({ url, onLoad });
		}
	}))
}));

vi.mock('three/examples/jsm/loaders/DRACOLoader.js', () => ({
	DRACOLoader: vi.fn().mockImplementation(() => ({ setDecoderPath: vi.fn() }))
}));

import { createGhostLayer, ghostInstanceMatrix } from './ghostLayer';
import { MESH_FLIP } from '../structures/structureLayer';
import { ueYawToThreeQuaternion } from '../../geo/coords3d';
import { worldToPixel } from '../../geo/utils';
import { pixelToLngLat } from '../../geo/mercator';
import { composeWorld } from './ghostTransform';
import { structureParts } from './meshLibrary';
import type { MeshPart } from './meshPlacement';
import type { BlueprintStructureGeometry, PlacementAnchor, Quat } from '$types';

type QuatTuple = [number, number, number, number];
type Vec3 = [number, number, number];

const identityPart: MeshPart = { loc: [0, 0, 0], rot: [0, 0, 0], scale: [1, 1, 1] };

const world = (translation: { x: number; y: number; z: number }, rotation: Quat, scale = { x: 1, y: 1, z: 1 }) => ({
	translation,
	rotation,
	scale
});

function ueRotateVector([x, y, z, w]: QuatTuple, [vx, vy, vz]: Vec3): Vec3 {
	return [
		(1 - 2 * (y * y + z * z)) * vx + 2 * (x * y - w * z) * vy + 2 * (x * z + w * y) * vz,
		2 * (x * y + w * z) * vx + (1 - 2 * (x * x + z * z)) * vy + 2 * (y * z - w * x) * vz,
		2 * (x * z - w * y) * vx + 2 * (y * z + w * x) * vy + (1 - 2 * (x * x + y * y)) * vz
	];
}

function ueQuatMultiply([x1, y1, z1, w1]: QuatTuple, [x2, y2, z2, w2]: QuatTuple): QuatTuple {
	return [
		w1 * x2 + x1 * w2 + y1 * z2 - z1 * y2,
		w1 * y2 - x1 * z2 + y1 * w2 + z1 * x2,
		w1 * z2 + x1 * y2 - y1 * x2 + z1 * w2,
		w1 * w2 - x1 * x2 - y1 * y2 - z1 * z2
	];
}

function ueToThreeVec([vx, vy, vz]: Vec3): THREE.Vector3 {
	return new THREE.Vector3(vx, vz, vy);
}

describe('ghostInstanceMatrix', () => {
	it('rotates a pitched/rolled UE quaternion consistently with the UE->three axis mapping', () => {
		const roll: QuatTuple = [Math.sin(0.55), 0, 0, Math.cos(0.55)];
		const pitch: QuatTuple = [0, Math.sin(-0.35), 0, Math.cos(-0.35)];
		const q = ueQuatMultiply(roll, pitch);

		const matrix = ghostInstanceMatrix(
			world({ x: 0, y: 0, z: 0 }, { x: q[0], y: q[1], z: q[2], w: q[3] }),
			identityPart,
			'MainMap',
			1,
			1
		);
		const rotation = new THREE.Matrix4().extractRotation(matrix);

		const basis: Vec3[] = [
			[1, 0, 0],
			[0, 1, 0],
			[0, 0, 1]
		];
		for (const v of basis) {
			const expected = ueToThreeVec(ueRotateVector(q, v)).applyMatrix4(MESH_FLIP);
			const actual = ueToThreeVec(v).applyMatrix4(rotation);
			expect(actual.x).toBeCloseTo(expected.x, 9);
			expect(actual.y).toBeCloseTo(expected.y, 9);
			expect(actual.z).toBeCloseTo(expected.z, 9);
		}
	});

	it('reduces to the trusted MESH_FLIP + ueYawToThreeQuaternion path for a yaw-only quaternion', () => {
		const yaw = 0.9;
		const qz = Math.sin(yaw / 2);
		const qw = Math.cos(yaw / 2);
		const worldX = 12345;
		const worldY = -6789;
		const cmToMerc = 0.7;

		const matrix = ghostInstanceMatrix(
			world({ x: worldX, y: worldY, z: 0 }, { x: 0, y: 0, z: qz, w: qw }),
			identityPart,
			'MainMap',
			1,
			cmToMerc
		);

		const [px, py] = worldToPixel(worldX, worldY, 'MainMap');
		const [lng, lat] = pixelToLngLat(px, py);
		const anchor = MercatorCoordinate.fromLngLat([lng, lat]);
		const rotation = MESH_FLIP.clone().multiply(
			new THREE.Matrix4().makeRotationFromQuaternion(ueYawToThreeQuaternion(yaw))
		);
		const scale = new THREE.Matrix4().makeScale(cmToMerc, cmToMerc, cmToMerc);
		const expected = new THREE.Matrix4()
			.makeTranslation(anchor.x, anchor.y, 0)
			.multiply(rotation)
			.multiply(scale);

		const got = matrix.toArray();
		const want = expected.toArray();
		for (let i = 0; i < 16; i++) {
			expect(got[i]).toBeCloseTo(want[i], 10);
		}
	});

	// Discriminator for the double-latitude-correction defect: cmToMerc already
	// carries the camera centre's latitude, so no per-instance term may remain.
	// Two ghosts sharing a world z but at very different (deliberately
	// non-mirror-image, since cosine is even) latitudes must give the same matrix
	// Z. The buggy version diverges ~4.14% here; the fixed one is bit-identical.
	it('maps two ghosts at the same world z but very different latitudes to the same matrix Z', () => {
		const worldZ = 5000;
		const cmToMerc = 0.6;
		const identityQuat: Quat = { x: 0, y: 0, z: 0, w: 1 };

		const matrixNorth = ghostInstanceMatrix(
			world({ x: -1_099_400, y: 0, z: worldZ }, identityQuat),
			identityPart,
			'MainMap',
			1,
			cmToMerc
		);
		const matrixSouth = ghostInstanceMatrix(
			world({ x: 340_000, y: 0, z: worldZ }, identityQuat),
			identityPart,
			'MainMap',
			1,
			cmToMerc
		);

		const zNorth = new THREE.Vector3().setFromMatrixPosition(matrixNorth).z;
		const zSouth = new THREE.Vector3().setFromMatrixPosition(matrixSouth).z;

		const relativeDiff = Math.abs(zNorth - zSouth) / Math.abs(zSouth);
		expect(relativeDiff).toBeLessThan(1e-9);
		expect(zNorth).toBeCloseTo(worldZ * cmToMerc, 12);
	});
});

// Blueprint ids whose manifest entries are stable single- and double-part cases.
const ONE_PART = 'BlastFurnace';
const TWO_PART = 'PalFoodBox';

const stubMap = () =>
	({ getCenter: () => ({ lng: 0, lat: 0 }), triggerRepaint: () => {} }) as unknown as Parameters<
		ReturnType<typeof createGhostLayer>['attachMapForTest']
	>[0];

function settlePendingLoads(): void {
	while (loadCalls.length > 0) {
		const call = loadCalls.shift()!;
		const root = new THREE.Object3D();
		root.add(new THREE.Mesh(new THREE.BoxGeometry(100, 100, 100)));
		call.onLoad({ scene: root });
	}
}

const ghostStructure = (
	mapObjectId: string,
	translation = { x: 0, y: 0, z: 0 }
): BlueprintStructureGeometry => ({
	map_object_id: mapObjectId,
	translation,
	rotation: { x: 0, y: 0, z: 0, w: 1 },
	scale: { x: 1, y: 1, z: 1 }
});

// A layer with its meshes resolved and one frame already applied.
function mountedLayer(geometry: BlueprintStructureGeometry[], anchor: PlacementAnchor) {
	const layer = createGhostLayer({ id: `ghost-${Math.random()}` });
	layer.attachMapForTest(stubMap());
	layer.update(geometry, anchor, 'MainMap', 1);
	layer.flushForTest();
	// The first pass requests meshes and finds none cached; settling them requeues.
	settlePendingLoads();
	layer.flushForTest();
	return layer;
}

function matrixAt(layer: ReturnType<typeof createGhostLayer>, group: number, index: number) {
	const out = new THREE.Matrix4();
	layer.groupsForTest()[group].mesh.getMatrixAt(index, out);
	return out;
}

describe('ghost layer update discipline', () => {
	it('keeps its groups and materials across an anchor move, rebaking only', () => {
		const geometry = [ghostStructure(ONE_PART), ghostStructure(ONE_PART, { x: 400, y: 0, z: 0 })];
		const layer = mountedLayer(geometry, { x: 0, y: 0, z: 0, yaw: 0 });

		const before = layer.groupsForTest();
		expect(before).toHaveLength(1);
		expect(before[0].count).toBe(2);
		const rebuildsBefore = layer.statsForTest().rebuilds;
		const firstMatrix = matrixAt(layer, 0, 0);

		layer.update(geometry, { x: 25_000, y: -8_000, z: 120, yaw: 0.6 }, 'MainMap', 1);
		layer.flushForTest();

		const after = layer.groupsForTest();
		expect(after[0].mesh).toBe(before[0].mesh);
		expect(after[0].material).toBe(before[0].material);
		expect(layer.statsForTest().rebuilds).toBe(rebuildsBefore);
		expect(layer.statsForTest().bakes).toBeGreaterThan(0);
		expect(matrixAt(layer, 0, 0).equals(firstMatrix)).toBe(false);

		layer.dispose();
	});

	it('recomposes without rebaking when only the camera moved', () => {
		const geometry = [ghostStructure(ONE_PART)];
		const layer = mountedLayer(geometry, { x: 1000, y: 2000, z: 30, yaw: 0.2 });

		const before = layer.groupsForTest()[0].mesh;
		const bakesBefore = layer.statsForTest().bakes;
		const composesBefore = layer.statsForTest().composes;

		layer.update(geometry, { x: 1000, y: 2000, z: 30, yaw: 0.2 }, 'MainMap', 1.75);
		layer.flushForTest();

		expect(layer.groupsForTest()[0].mesh).toBe(before);
		expect(layer.statsForTest().bakes).toBe(bakesBefore);
		expect(layer.statsForTest().composes).toBe(composesBefore + 1);

		layer.dispose();
	});

	it('rebuilds when the blueprint itself is replaced', () => {
		const first = [ghostStructure(ONE_PART)];
		const layer = mountedLayer(first, { x: 0, y: 0, z: 0, yaw: 0 });
		const rebuildsBefore = layer.statsForTest().rebuilds;

		layer.update([ghostStructure(TWO_PART)], { x: 0, y: 0, z: 0, yaw: 0 }, 'MainMap', 1);
		layer.flushForTest();
		settlePendingLoads();
		layer.flushForTest();

		expect(layer.statsForTest().rebuilds).toBeGreaterThan(rebuildsBefore);
		// PalFoodBox carries two parts, so it buckets into two single-instance groups.
		expect(layer.groupsForTest()).toHaveLength(2);
		expect(layer.groupsForTest().map((g) => g.count)).toEqual([1, 1]);

		layer.dispose();
	});

	it('collapses a burst of drag updates into one applied frame', async () => {
		const geometry = [ghostStructure(ONE_PART)];
		const layer = mountedLayer(geometry, { x: 0, y: 0, z: 0, yaw: 0 });
		const appliesBefore = layer.statsForTest().applies;

		for (let i = 1; i <= 20; i++) {
			layer.update(geometry, { x: i * 10, y: i * 10, z: 0, yaw: i * 0.01 }, 'MainMap', 1);
		}
		expect(layer.statsForTest().applies).toBe(appliesBefore);

		await new Promise((resolve) => setTimeout(resolve, 20));
		expect(layer.statsForTest().applies).toBe(appliesBefore + 1);
		// The frame that ran must reflect the last request, not the first.
		expect(layer.groupsForTest()).toHaveLength(1);

		layer.dispose();
	});

	it('drops a scheduled frame on dispose', async () => {
		const geometry = [ghostStructure(ONE_PART)];
		const layer = mountedLayer(geometry, { x: 0, y: 0, z: 0, yaw: 0 });
		const appliesBefore = layer.statsForTest().applies;

		layer.update(geometry, { x: 900, y: 900, z: 0, yaw: 1 }, 'MainMap', 1);
		layer.dispose();

		await new Promise((resolve) => setTimeout(resolve, 20));
		expect(layer.statsForTest().applies).toBe(appliesBefore);
		expect(layer.groupsForTest()).toHaveLength(0);
	});

	it('writes the same instance matrices the per-instance oracle would', () => {
		const anchor: PlacementAnchor = { x: -31_000, y: 12_500, z: 240, yaw: 0.85 };
		const geometry = [
			ghostStructure(ONE_PART, { x: 0, y: 0, z: 0 }),
			ghostStructure(ONE_PART, { x: 1200, y: -450, z: 80 })
		];
		const layer = mountedLayer(geometry, anchor);
		const part = structureParts(ONE_PART)![0];

		// The stub map sits at lng/lat 0, where meterInMercatorCoordinateUnits is the
		// equatorial value; verticalScale 1 leaves cmToMerc at exactly that.
		const cmToMerc = MercatorCoordinate.fromLngLat([0, 0], 0).meterInMercatorCoordinateUnits();

		for (let i = 0; i < geometry.length; i++) {
			const expected = ghostInstanceMatrix(
				composeWorld(anchor, geometry[i]),
				part,
				'MainMap',
				1,
				cmToMerc
			);
			const actual = matrixAt(layer, 0, i);
			for (let e = 0; e < 16; e++) {
				const scale = Math.max(Math.abs(expected.elements[e]), 1e-12);
				expect(Math.abs(actual.elements[e] - expected.elements[e]) / scale).toBeLessThan(1e-6);
			}
		}

		layer.dispose();
	});
});

describe('ghost layer no-op updates', () => {
	it('does no per-instance work when nothing about the request changed', () => {
		const geometry = [ghostStructure(ONE_PART)];
		const anchor: PlacementAnchor = { x: 500, y: 600, z: 10, yaw: 0.3 };
		const layer = mountedLayer(geometry, anchor);
		const before = layer.statsForTest();

		for (let i = 0; i < 5; i++) {
			layer.update(geometry, { ...anchor }, 'MainMap', 1);
			layer.flushForTest();
		}

		const after = layer.statsForTest();
		expect(after.rebuilds).toBe(before.rebuilds);
		expect(after.bakes).toBe(before.bakes);
		expect(after.composes).toBe(before.composes);

		layer.dispose();
	});
});
