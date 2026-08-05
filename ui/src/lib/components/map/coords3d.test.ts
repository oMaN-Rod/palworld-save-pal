import { describe, it, expect } from 'vitest';
import * as THREE from 'three';
import { ueYawToThreeQuaternion } from './coords3d';

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
