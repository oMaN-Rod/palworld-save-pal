import { describe, expect, it } from 'vitest';
import { composeWorld, yawQuat } from './ghostTransform';
import type { Quat, Vec3 } from '$types';

const IDENT: Quat = { x: 0, y: 0, z: 0, w: 1 };
const rel = (t: Vec3, r: Quat = IDENT, s: Vec3 = { x: 1, y: 1, z: 1 }) => ({
	translation: t, rotation: r, scale: s
});
const near = (a: number, b: number) => expect(Math.abs(a - b)).toBeLessThan(1e-9);

describe('composeWorld', () => {
	it('identity anchor returns the relative transform unchanged', () => {
		const w = composeWorld({ x: 0, y: 0, z: 0, yaw: 0 }, rel({ x: 10, y: 20, z: 30 }));
		expect(w.translation).toEqual({ x: 10, y: 20, z: 30 });
		expect(w.rotation).toEqual(IDENT);
	});

	it('a zero-yaw anchor just translates', () => {
		const w = composeWorld({ x: 100, y: 200, z: 5, yaw: 0 }, rel({ x: 10, y: 20, z: 30 }));
		expect(w.translation).toEqual({ x: 110, y: 220, z: 35 });
	});

	it('a 90-degree yaw rotates the relative offset about +Z (right-handed)', () => {
		// +X offset, rotated +90deg about Z -> +Y
		const w = composeWorld({ x: 0, y: 0, z: 0, yaw: Math.PI / 2 }, rel({ x: 100, y: 0, z: 0 }));
		near(w.translation.x, 0);
		near(w.translation.y, 100);
		near(w.translation.z, 0);
	});

	it('composes the anchor yaw with the relative rotation (quat multiply)', () => {
		// relative already yawed 90; anchor yaws another 90 -> combined 180 about Z
		const w = composeWorld({ x: 0, y: 0, z: 0, yaw: Math.PI / 2 }, rel({ x: 0, y: 0, z: 0 }, yawQuat(Math.PI / 2)));
		const expected = yawQuat(Math.PI); // z=sin(90deg)=1, w=cos(90deg)=0
		near(w.rotation.z, expected.z);
		near(w.rotation.w, expected.w);
	});

	it('carries scale through unchanged', () => {
		const w = composeWorld({ x: 0, y: 0, z: 0, yaw: 1.2 }, rel({ x: 0, y: 0, z: 0 }, IDENT, { x: 2, y: 3, z: 4 }));
		expect(w.scale).toEqual({ x: 2, y: 3, z: 4 });
	});
});
