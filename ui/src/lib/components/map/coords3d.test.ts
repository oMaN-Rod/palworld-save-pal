import { describe, it, expect } from 'vitest';
import * as THREE from 'three';
import { ueYawToThreeQuaternion, ueQuatToThree } from './coords3d';

describe('ueYawToThreeQuaternion', () => {
	it('maps UE yaw (Z-up) to a rotation about Three up-axis Y', () => {
		const q = ueYawToThreeQuaternion(Math.PI / 2);
		// A forward vector along Three +Z rotated by yaw 90deg lands on the X axis (sign per axis-swap).
		const v = new THREE.Vector3(0, 0, 1).applyQuaternion(q);
		// Sign convention (which way yaw turns) is validated visually against the map in Task 8.
		expect(v.x).toBeCloseTo(-1, 6);
		expect(v.y).toBeCloseTo(0, 6);
		expect(v.z).toBeCloseTo(0, 6);
	});

	it('identity yaw is identity rotation', () => {
		const q = ueYawToThreeQuaternion(0);
		expect(q.x).toBeCloseTo(0, 6);
		expect(q.y).toBeCloseTo(0, 6);
		expect(q.z).toBeCloseTo(0, 6);
		expect(Math.abs(q.w)).toBeCloseTo(1, 6);
	});
});

type Quat = [number, number, number, number];
type Vec3 = [number, number, number];

// Ground truth from the standard Hamilton formula, independent of three.js and
// of ueQuatToThree itself, so this cannot be circular.
function ueRotateVector([x, y, z, w]: Quat, [vx, vy, vz]: Vec3): Vec3 {
	return [
		(1 - 2 * (y * y + z * z)) * vx + 2 * (x * y - w * z) * vy + 2 * (x * z + w * y) * vz,
		2 * (x * y + w * z) * vx + (1 - 2 * (x * x + z * z)) * vy + 2 * (y * z - w * x) * vz,
		2 * (x * z - w * y) * vx + 2 * (y * z + w * x) * vy + (1 - 2 * (x * x + y * y)) * vz
	];
}

// Builds a general test quaternion from elemental axis rotations, again
// independent of the code under test.
function ueQuatMultiply([x1, y1, z1, w1]: Quat, [x2, y2, z2, w2]: Quat): Quat {
	return [
		w1 * x2 + x1 * w2 + y1 * z2 - z1 * y2,
		w1 * y2 - x1 * z2 + y1 * w2 + z1 * x2,
		w1 * z2 + x1 * y2 - y1 * x2 + z1 * w2,
		w1 * w2 - x1 * x2 - y1 * y2 - z1 * z2
	];
}

// The ue.(x,y,z) -> three.(x,z,y) axis swap applied to a plain vector.
function ueToThreeVec([vx, vy, vz]: Vec3): THREE.Vector3 {
	return new THREE.Vector3(vx, vz, vy);
}

// The correctness criterion: R' . P == P . R. Checking all three basis vectors
// pins the whole 3x3, not just one column.
function expectAxisMappingHolds(q: Quat) {
	const three = ueQuatToThree(q[0], q[1], q[2], q[3]);
	const basis: Vec3[] = [
		[1, 0, 0],
		[0, 1, 0],
		[0, 0, 1]
	];
	for (const v of basis) {
		const expected = ueToThreeVec(ueRotateVector(q, v));
		const actual = ueToThreeVec(v).applyQuaternion(three);
		expect(actual.x).toBeCloseTo(expected.x, 10);
		expect(actual.y).toBeCloseTo(expected.y, 10);
		expect(actual.z).toBeCloseTo(expected.z, 10);
	}
}

// ueQuatToThree was long unverified beyond the yaw-only case, where its sign
// error cancelled. Each case below is checked against the independent geometric
// ground truth above rather than against any numeric constant.
describe('ueQuatToThree', () => {
	it('matches the UE->three axis mapping for a yaw-only rotation (about UE Z)', () => {
		const yaw = 0.9;
		expectAxisMappingHolds([0, 0, Math.sin(yaw / 2), Math.cos(yaw / 2)]);
	});

	it('matches the UE->three axis mapping for a pitch-only rotation (about UE Y)', () => {
		const pitch = 0.6;
		expectAxisMappingHolds([0, Math.sin(pitch / 2), 0, Math.cos(pitch / 2)]);
	});

	it('matches the UE->three axis mapping for a roll-only rotation (about UE X)', () => {
		const roll = 1.1;
		expectAxisMappingHolds([Math.sin(roll / 2), 0, 0, Math.cos(roll / 2)]);
	});

	it('matches the UE->three axis mapping for a combined pitch+roll+yaw rotation', () => {
		const yaw: Quat = [0, 0, Math.sin(0.4), Math.cos(0.4)];
		const pitch: Quat = [0, Math.sin(-0.7), 0, Math.cos(-0.7)];
		const roll: Quat = [Math.sin(0.25), 0, 0, Math.cos(0.25)];
		const combined = ueQuatMultiply(ueQuatMultiply(roll, pitch), yaw);
		expectAxisMappingHolds(combined);
	});

	it('agrees (up to quaternion negation) with the old yaw-only output for pure yaw', () => {
		// q and -q are the same rotation, so the w sign flip must not matter here.
		const yaw = -1.3;
		const qz = Math.sin(yaw / 2);
		const qw = Math.cos(yaw / 2);
		const fixed = ueQuatToThree(0, 0, qz, qw);
		const old = new THREE.Quaternion(0, -qz, 0, qw); // old formula: (qx, -qz, qy, qw)
		const fixedMat = new THREE.Matrix4().makeRotationFromQuaternion(fixed);
		const oldMat = new THREE.Matrix4().makeRotationFromQuaternion(old);
		for (let i = 0; i < 16; i++) {
			expect(fixedMat.elements[i]).toBeCloseTo(oldMat.elements[i], 10);
		}
	});
});
