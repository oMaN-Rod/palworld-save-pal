import { describe, expect, it } from 'vitest';
import { composeWorld, composeWorldInto, emptyWorldTransform, yawQuat } from './ghostTransform';
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

describe('composeWorldInto', () => {
	it('matches the allocating form and reuses the caller\'s objects', () => {
		const anchor = { x: -4200, y: 9100, z: 55, yaw: 1.37 };
		const relative = {
			translation: { x: 310, y: -78, z: 24 },
			rotation: { x: 0.1, y: -0.2, z: 0.3, w: Math.sqrt(1 - 0.14) },
			scale: { x: 1.5, y: 0.5, z: 2 }
		};
		const out = emptyWorldTransform();
		const translation = out.translation;
		const rotation = out.rotation;

		const actual = composeWorldInto(anchor, relative, out);
		const expected = composeWorld(anchor, relative);

		expect(actual).toBe(out);
		expect(out.translation).toBe(translation);
		expect(out.rotation).toBe(rotation);
		expect(actual.translation).toEqual(expected.translation);
		expect(actual.rotation).toEqual(expected.rotation);
		expect(actual.scale).toEqual(expected.scale);
	});

	it('overwrites every field left over from a previous instance', () => {
		const out = composeWorldInto(
			{ x: 1, y: 2, z: 3, yaw: 0.9 },
			{
				translation: { x: 7, y: 8, z: 9 },
				rotation: { x: 0.5, y: 0.5, z: 0.5, w: 0.5 },
				scale: { x: 3, y: 4, z: 5 }
			},
			emptyWorldTransform()
		);
		const reused = composeWorldInto(
			{ x: 0, y: 0, z: 0, yaw: 0 },
			{
				translation: { x: 0, y: 0, z: 0 },
				rotation: { x: 0, y: 0, z: 0, w: 1 },
				scale: { x: 1, y: 1, z: 1 }
			},
			out
		);
		expect(reused).toEqual(emptyWorldTransform());
	});
});
