// Pinned element-for-element against meshInstanceMatrix and proxyInstanceMatrix,
// which are the untouched oracle. Equality here proves a layer that bakes once
// and composes per frame renders identically to one that rebuilds every instance.
import { describe, it, expect } from 'vitest';
import * as THREE from 'three';
import { bakeStructureInstance, composeStructureMatrix, STRUCTURE_BAKE_STRIDE } from './structureInstances';
import { meshInstanceMatrix, proxyInstanceMatrix } from './structureLayer';
import { structureAnchor, structurePlacement } from './structurePlacement';
import type { MeshPart } from './meshPlacement';
import type { BaseStructure, Footprint } from '$types';

const identityPart: MeshPart = { loc: [0, 0, 0], rot: [0, 0, 0], scale: [1, 1, 1] };

const base = (over: Partial<BaseStructure>): BaseStructure => ({
	instance_id: 'i',
	map_object_id: 'Wooden_DoorWall',
	x: 0,
	y: 0,
	z: 0,
	yaw: 0,
	scale_x: 1,
	scale_y: 1,
	scale_z: 1,
	hp_current: 1,
	hp_max: 1,
	build_player_uid: 'u',
	...over
});

const fpWithBoxOffset: Footprint = {
	sx: 400,
	sy: 20,
	sz: 325,
	ox: 15,
	oy: -8,
	oz: 158.8,
	typeA: 'Foundation',
	archetype: 'wallDoor'
};

const STRUCTURES: BaseStructure[] = [
	base({ x: 0, y: 0, z: 0, yaw: 0 }),
	base({ x: -1_099_400, y: 0, z: 5000, yaw: Math.PI / 4 }),
	base({ x: 340_000, y: 0, z: 5000, yaw: -Math.PI / 3 }),
	base({ x: 120_000, y: -450_000, z: -2500, yaw: 3.5 * Math.PI }),
	base({ x: -80_000, y: 610_000, z: 99999, yaw: -7.25 }),
	base({ x: 1000, y: -1000, z: -1, yaw: 10 * Math.PI + 0.001 })
];

// A very small and a very large cmToMerc, bracketing the realistic
// verticalScale * meterInMercatorCoordinateUnits() magnitude (~2.5e-9) used
// elsewhere in this codebase's tests.
const CM_TO_MERC_VALUES = [1e-13, 2.5e-9, 1e-3];

// Baked values round-trip through a Float32Array, so raw centimetre altitudes
// lose precision against the oracle's double arithmetic. Hence a relative
// tolerance, floored for near-zero elements.
function expectMatrixClose(got: THREE.Matrix4, want: THREE.Matrix4) {
	const ge = got.elements;
	const we = want.elements;
	for (let i = 0; i < 16; i++) {
		expect(Math.abs(ge[i] - we[i])).toBeLessThanOrEqual(Math.max(1e-9, Math.abs(we[i]) * 1e-6));
	}
}

describe('bakeStructureInstance + composeStructureMatrix (mesh path oracle)', () => {
	for (const s of STRUCTURES) {
		for (const cmToMerc of CM_TO_MERC_VALUES) {
			it(`matches meshInstanceMatrix for x=${s.x} y=${s.y} z=${s.z} yaw=${s.yaw} cmToMerc=${cmToMerc}`, () => {
				const anchor = structureAnchor(s, 'MainMap');
				const baked = bakeStructureInstance(anchor);
				const got = new THREE.Matrix4();
				composeStructureMatrix(baked, 0, cmToMerc, got);

				const want = meshInstanceMatrix(s, identityPart, 'MainMap', 1, cmToMerc);
				expectMatrixClose(got, want);
			});
		}
	}
});

describe('bakeStructureInstance + composeStructureMatrix (proxy path oracle)', () => {
	for (const s of STRUCTURES) {
		for (const cmToMerc of CM_TO_MERC_VALUES) {
			for (const archetype of ['foundation', 'wallDoor']) {
				it(`matches proxyInstanceMatrix (${archetype}) for x=${s.x} y=${s.y} z=${s.z} yaw=${s.yaw} cmToMerc=${cmToMerc}`, () => {
					const fp: Footprint = { ...fpWithBoxOffset, archetype };
					const p = structurePlacement(s, fp, 'MainMap', 1);
					const halfH = p.footprintCm.sz / 2;
					const originCm = p.altitudeCm + (archetype === 'foundation' ? halfH : -halfH);

					const baked = bakeStructureInstance({ lng: p.lng, lat: p.lat, altitudeCm: originCm, yaw: p.yaw });
					const got = new THREE.Matrix4();
					composeStructureMatrix(baked, 0, cmToMerc, got);

					const want = proxyInstanceMatrix(s, fp, archetype, 'MainMap', 1, cmToMerc);
					expectMatrixClose(got, want);
				});
			}
		}
	}
});

describe('composeStructureMatrix packing', () => {
	it('reads only its own instance out of a Float32Array holding many, at any offset', () => {
		const anchors = STRUCTURES.map((s) => structureAnchor(s, 'MainMap'));
		const packed = new Float32Array(anchors.length * STRUCTURE_BAKE_STRIDE);
		anchors.forEach((anchor, i) => {
			packed.set(bakeStructureInstance(anchor), i * STRUCTURE_BAKE_STRIDE);
		});

		const cmToMerc = 2.5e-9;
		anchors.forEach((anchor, i) => {
			const fromPacked = new THREE.Matrix4();
			composeStructureMatrix(packed, i * STRUCTURE_BAKE_STRIDE, cmToMerc, fromPacked);

			const solo = bakeStructureInstance(anchor);
			const fromSolo = new THREE.Matrix4();
			composeStructureMatrix(solo, 0, cmToMerc, fromSolo);

			expectMatrixClose(fromPacked, fromSolo);
		});
	});
});

describe('bakeStructureInstance', () => {
	it('produces exactly STRUCTURE_BAKE_STRIDE floats', () => {
		expect(bakeStructureInstance({ lng: 10, lat: 20, altitudeCm: 500, yaw: 1 }).length).toBe(
			STRUCTURE_BAKE_STRIDE
		);
	});
});
