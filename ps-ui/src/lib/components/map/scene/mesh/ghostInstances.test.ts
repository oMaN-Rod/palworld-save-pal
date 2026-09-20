// Pinned element-for-element against ghostInstanceMatrix, which is the untouched
// oracle. Equality here proves a layer that bakes on anchor change and composes
// per frame renders identically to one that rebuilt every instance from scratch.
import { describe, it, expect } from 'vitest';
import * as THREE from 'three';
import { bakeGhostInstance, composeGhostMatrix, GHOST_BAKE_STRIDE } from './ghostInstances';
import { ghostInstanceMatrix } from './ghostLayer';
import { composeWorld } from './ghostTransform';
import type { MeshPart } from './meshPlacement';
import type { MapArea } from '../../geo/utils';
import type { PlacementAnchor, Quat, Vec3 } from '$types';

const identityPart: MeshPart = { loc: [0, 0, 0], rot: [0, 0, 0], scale: [1, 1, 1] };
const offsetPart: MeshPart = { loc: [120, -45, 310], rot: [12, -33, 7], scale: [1.5, 0.8, 2] };

const unit = (yaw: number): Quat => ({ x: 0, y: 0, z: Math.sin(yaw / 2), w: Math.cos(yaw / 2) });

const structure = (
	translation: Vec3,
	rotation: Quat,
	scale: Vec3 = { x: 1, y: 1, z: 1 }
) => ({ translation, rotation, scale });

function expectMatches(
	anchor: PlacementAnchor,
	relative: { translation: Vec3; rotation: Quat; scale: Vec3 },
	part: MeshPart,
	area: MapArea,
	cmToMerc: number
) {
	const world = composeWorld(anchor, relative);
	const expected = ghostInstanceMatrix(world, part, area, 1, cmToMerc);

	const baked = new Float64Array(GHOST_BAKE_STRIDE);
	bakeGhostInstance(world, part, area, baked, 0);
	const actual = new THREE.Matrix4();
	composeGhostMatrix(baked, 0, cmToMerc, actual);

	for (let i = 0; i < 16; i++) {
		// Mercator translation components are ~1e-1 with ~1e-9 significance; the
		// basis components are ~1e-7. A relative tolerance keeps both honest.
		const scale = Math.max(Math.abs(expected.elements[i]), 1e-12);
		expect(Math.abs(actual.elements[i] - expected.elements[i]) / scale).toBeLessThan(1e-9);
	}
}

const CM_TO_MERC = 2.4e-9;

describe('bakeGhostInstance / composeGhostMatrix', () => {
	it('reproduces the oracle for an identity part at the origin', () => {
		expectMatches(
			{ x: 0, y: 0, z: 0, yaw: 0 },
			structure({ x: 0, y: 0, z: 0 }, unit(0)),
			identityPart,
			'MainMap',
			CM_TO_MERC
		);
	});

	it('reproduces the oracle for an offset anchor and a rotated structure', () => {
		expectMatches(
			{ x: -142000, y: 87000, z: 4300, yaw: 1.1 },
			structure({ x: 2400, y: -1800, z: 620 }, unit(-0.7)),
			identityPart,
			'MainMap',
			CM_TO_MERC
		);
	});

	it('reproduces the oracle for a part with its own transform', () => {
		expectMatches(
			{ x: 33000, y: -21000, z: -150, yaw: -2.4 },
			structure({ x: -900, y: 4100, z: 75 }, unit(2.0)),
			offsetPart,
			'MainMap',
			CM_TO_MERC
		);
	});

	it('reproduces the oracle for a non-uniform per-structure scale', () => {
		expectMatches(
			{ x: 1200, y: 9900, z: 10, yaw: 0.35 },
			structure({ x: 500, y: 500, z: 200 }, unit(0.9), { x: 2, y: 0.5, z: 1.25 }),
			offsetPart,
			'MainMap',
			CM_TO_MERC
		);
	});

	it('reproduces the oracle for a pitched and rolled structure quaternion', () => {
		const roll = Math.sin(0.4);
		const q: Quat = { x: roll, y: Math.sin(-0.25), z: Math.sin(0.6), w: Math.cos(0.5) };
		const n = Math.hypot(q.x, q.y, q.z, q.w);
		expectMatches(
			{ x: -5000, y: -5000, z: 800, yaw: 2.9 },
			structure({ x: 310, y: -220, z: 44 }, { x: q.x / n, y: q.y / n, z: q.z / n, w: q.w / n }),
			offsetPart,
			'MainMap',
			CM_TO_MERC
		);
	});

	it('reproduces the oracle on the Tree area', () => {
		expectMatches(
			{ x: 4000, y: -7000, z: 220, yaw: 0.6 },
			structure({ x: 800, y: 150, z: 30 }, unit(1.4)),
			offsetPart,
			'Tree',
			CM_TO_MERC
		);
	});

	it('recomposes an unchanged bake across vertical scales', () => {
		const anchor: PlacementAnchor = { x: 6100, y: -2200, z: 95, yaw: 0.8 };
		const relative = structure({ x: 40, y: -90, z: 12 }, unit(0.3), { x: 1.4, y: 1, z: 0.6 });
		const world = composeWorld(anchor, relative);
		const baked = new Float64Array(GHOST_BAKE_STRIDE);
		bakeGhostInstance(world, offsetPart, 'MainMap', baked, 0);

		for (const cmToMerc of [1e-9, 2.4e-9, 9e-9]) {
			const expected = ghostInstanceMatrix(world, offsetPart, 'MainMap', 1, cmToMerc);
			const actual = new THREE.Matrix4();
			composeGhostMatrix(baked, 0, cmToMerc, actual);
			for (let i = 0; i < 16; i++) {
				const scale = Math.max(Math.abs(expected.elements[i]), 1e-12);
				expect(Math.abs(actual.elements[i] - expected.elements[i]) / scale).toBeLessThan(1e-9);
			}
		}
	});

	it('bakes at an offset without disturbing neighbouring instances', () => {
		const anchor: PlacementAnchor = { x: 100, y: 200, z: 5, yaw: 0.2 };
		const baked = new Float64Array(GHOST_BAKE_STRIDE * 3);
		baked.fill(Number.NaN);
		const world = composeWorld(anchor, structure({ x: 10, y: 20, z: 3 }, unit(0.1)));
		bakeGhostInstance(world, identityPart, 'MainMap', baked, GHOST_BAKE_STRIDE);

		for (let i = 0; i < GHOST_BAKE_STRIDE; i++) {
			expect(Number.isNaN(baked[i])).toBe(true);
			expect(Number.isNaN(baked[GHOST_BAKE_STRIDE * 2 + i])).toBe(true);
			expect(Number.isNaN(baked[GHOST_BAKE_STRIDE + i])).toBe(false);
		}

		const expected = ghostInstanceMatrix(world, identityPart, 'MainMap', 1, CM_TO_MERC);
		const actual = new THREE.Matrix4();
		composeGhostMatrix(baked, GHOST_BAKE_STRIDE, CM_TO_MERC, actual);
		for (let i = 0; i < 16; i++) {
			const scale = Math.max(Math.abs(expected.elements[i]), 1e-12);
			expect(Math.abs(actual.elements[i] - expected.elements[i]) / scale).toBeLessThan(1e-9);
		}
	});

	it('writes only into the buffers the caller owns', () => {
		// A guard on the hot path: bake and compose run per instance per frame during
		// a drag, so both must write into buffers the caller already owns.
		const anchor: PlacementAnchor = { x: 0, y: 0, z: 0, yaw: 0.5 };
		const world = composeWorld(anchor, structure({ x: 1, y: 2, z: 3 }, unit(0.1)));
		const baked = new Float64Array(GHOST_BAKE_STRIDE);
		const target = new THREE.Matrix4();
		const elements = target.elements;
		for (let i = 0; i < 100; i++) {
			bakeGhostInstance(world, offsetPart, 'MainMap', baked, 0);
			composeGhostMatrix(baked, 0, CM_TO_MERC, target);
		}
		expect(target.elements).toBe(elements);
	});
});
