import { describe, it, expect } from 'vitest';
import * as THREE from 'three';
import { MercatorCoordinate } from 'maplibre-gl';
import { ghostInstanceMatrix } from './ghostLayer';
import { MESH_FLIP } from './structureLayer';
import { ueYawToThreeQuaternion } from './coords3d';
import { worldToPixel } from './utils';
import { pixelToLngLat } from './mercator';
import type { MeshPart } from './meshPlacement';
import type { Quat } from '$types';

type QuatTuple = [number, number, number, number];
type Vec3 = [number, number, number];

const identityPart: MeshPart = { loc: [0, 0, 0], rot: [0, 0, 0], scale: [1, 1, 1] };

const world = (translation: { x: number; y: number; z: number }, rotation: Quat, scale = { x: 1, y: 1, z: 1 }) => ({
	translation,
	rotation,
	scale
});

// Ground truth from the standard Hamilton formula, independent of three.js and
// of the conversion under test, so this cannot be circular.
function ueRotateVector([x, y, z, w]: QuatTuple, [vx, vy, vz]: Vec3): Vec3 {
	return [
		(1 - 2 * (y * y + z * z)) * vx + 2 * (x * y - w * z) * vy + 2 * (x * z + w * y) * vz,
		2 * (x * y + w * z) * vx + (1 - 2 * (x * x + z * z)) * vy + 2 * (y * z - w * x) * vz,
		2 * (x * z - w * y) * vx + 2 * (y * z + w * x) * vy + (1 - 2 * (x * x + y * y)) * vz
	];
}

// Builds a combined pitch+roll test quaternion from elemental axis rotations,
// again independent of the code under test.
function ueQuatMultiply([x1, y1, z1, w1]: QuatTuple, [x2, y2, z2, w2]: QuatTuple): QuatTuple {
	return [
		w1 * x2 + x1 * w2 + y1 * z2 - z1 * y2,
		w1 * y2 - x1 * z2 + y1 * w2 + z1 * x2,
		w1 * z2 + x1 * y2 - y1 * x2 + z1 * w2,
		w1 * w2 - x1 * x2 - y1 * y2 - z1 * z2
	];
}

// The ue.(x,y,z) -> three.(x,z,y) axis swap, applied to a plain vector (same
// map the position path uses).
function ueToThreeVec([vx, vy, vz]: Vec3): THREE.Vector3 {
	return new THREE.Vector3(vx, vz, vy);
}

describe('ghostInstanceMatrix', () => {
	// Rotating a UE vector then converting to three-space must equal converting
	// first and then applying the matrix's rotation. MESH_FLIP is common to both
	// sides, so a wrong quaternion conversion cannot be masked by it.
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
		// extractRotation strips translation and normalizes out uniform scale,
		// leaving exactly MESH_FLIP * R(ueQuatToThree(q)).
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

	// A yaw-only quaternion must still reduce to the MESH_FLIP +
	// ueYawToThreeQuaternion path structureLayer trusts. worldZ is 0 so the
	// expected matrix is immune to the altitude formula, isolating the rotation.
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
