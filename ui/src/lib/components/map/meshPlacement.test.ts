import { describe, it, expect } from 'vitest';
import * as THREE from 'three';
import { partLocalMatrix, ueEulerToThreeQuaternion, type MeshPart } from './meshPlacement';
import { ueYawToThreeQuaternion } from './coords3d';

const DEG = Math.PI / 180;

const identity: MeshPart = { loc: [0, 0, 0], rot: [0, 0, 0], scale: [1, 1, 1] };

describe('partLocalMatrix', () => {
	it('is identity for an identity part', () => {
		expect(partLocalMatrix(identity).elements).toEqual(new THREE.Matrix4().identity().elements);
	});

	it('applies the UE->three axis swap to translation', () => {
		const p = new THREE.Vector3().setFromMatrixPosition(
			partLocalMatrix({ ...identity, loc: [10, 20, 30] })
		);
		expect(p.x).toBeCloseTo(10, 6);
		expect(p.y).toBeCloseTo(30, 6);
		expect(p.z).toBeCloseTo(20, 6);
	});

	it('applies the axis swap to scale', () => {
		const s = new THREE.Vector3().setFromMatrixScale(
			partLocalMatrix({ ...identity, scale: [2, 3, 4] })
		);
		expect(s.x).toBeCloseTo(2, 6);
		expect(s.y).toBeCloseTo(4, 6);
		expect(s.z).toBeCloseTo(3, 6);
	});
});

describe('ueEulerToThreeQuaternion', () => {
	it('is identity for zero rotation', () => {
		expect(Math.abs(ueEulerToThreeQuaternion(0, 0, 0).w)).toBeCloseTo(1, 6);
	});

	it('yaw rotates about the three up axis', () => {
		const v = new THREE.Vector3(0, 0, 1).applyQuaternion(ueEulerToThreeQuaternion(0, 90, 0));
		expect(v.y).toBeCloseTo(0, 6);
		// Signed, not abs: -1 is the mathematically correct value (see consistency
		// check below), matching ueYawToThreeQuaternion's already-proven sign convention.
		expect(v.x).toBeCloseTo(-1, 6);
		expect(v.z).toBeCloseTo(0, 6);
	});

	it('pitch alone rotates about the three Z axis', () => {
		const deg = 40;
		const v = new THREE.Vector3(1, 0, 0).applyQuaternion(ueEulerToThreeQuaternion(deg, 0, 0));
		expect(v.x).toBeCloseTo(Math.cos(deg * DEG), 6);
		expect(v.y).toBeCloseTo(Math.sin(deg * DEG), 6);
	});

	it('roll alone rotates about the three X axis', () => {
		const deg = 25;
		const v = new THREE.Vector3(0, 1, 0).applyQuaternion(ueEulerToThreeQuaternion(0, 0, deg));
		expect(v.y).toBeCloseTo(Math.cos(deg * DEG), 6);
		expect(v.z).toBeCloseTo(Math.sin(deg * DEG), 6);
	});

	it('agrees with ueYawToThreeQuaternion for a pitch/roll-free part, across several yaw angles', () => {
		for (const yawDeg of [0, 30, 90, -45, 178]) {
			const fromPart = ueEulerToThreeQuaternion(0, yawDeg, 0);
			const fromYawOnly = ueYawToThreeQuaternion(yawDeg * DEG);
			expect(fromPart.x).toBeCloseTo(fromYawOnly.x, 6);
			expect(fromPart.y).toBeCloseTo(fromYawOnly.y, 6);
			expect(fromPart.z).toBeCloseTo(fromYawOnly.z, 6);
			expect(fromPart.w).toBeCloseTo(fromYawOnly.w, 6);
		}
	});
});
