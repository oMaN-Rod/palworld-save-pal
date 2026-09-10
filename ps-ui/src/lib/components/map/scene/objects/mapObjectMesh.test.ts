import type { BaseStructure } from '$types';
import * as THREE from 'three';
import { describe, expect, it } from 'vitest';
import { manifestParts, mapObjectInstanceMatrix, meshNames } from './mapObjectMesh';
import { MESH_FLIP, meshInstanceMatrix } from '../structures/structureLayer';

const DEG = Math.PI / 180;

const MANIFEST = {
	BP_LevelObject_Relic_C: {
		parts: [
			{
				mesh: 'SM_JewelBase_a1',
				loc: [0, 0, 0] as [number, number, number],
				rot: [0, 0, 0] as [number, number, number],
				scale: [1, 1, 1] as [number, number, number]
			},
			{
				mesh: 'SM_Jewel_b2',
				loc: [30, -20, 120] as [number, number, number],
				rot: [0, 45, 0] as [number, number, number],
				scale: [1, 1, 1] as [number, number, number]
			}
		],
		cullDistanceCm: 30000
	},
	BP_LevelObject_Relic_Other_C: {
		parts: [
			{
				mesh: 'SM_JewelBase_a1',
				loc: [0, 0, 0] as [number, number, number],
				rot: [0, 0, 0] as [number, number, number],
				scale: [2, 2, 2] as [number, number, number]
			}
		]
	},
	BP_LevelObject_TowerFastTravelPoint_C: {
		parts: [
			{
				mesh: 'SM_FastTravelStatue_c3',
				loc: [0, 0, 0] as [number, number, number],
				rot: [0, 0, 0] as [number, number, number],
				scale: [1, 1, 1] as [number, number, number]
			}
		]
	}
};

const BASE = MANIFEST.BP_LevelObject_Relic_C.parts[0];
const JEWEL = MANIFEST.BP_LevelObject_Relic_C.parts[1];

function positionOf(m: THREE.Matrix4): THREE.Vector3 {
	return new THREE.Vector3().setFromMatrixPosition(m);
}

function forwardOf(m: THREE.Matrix4): THREE.Vector3 {
	return new THREE.Vector3(1, 0, 0).applyMatrix4(m).sub(positionOf(m)).normalize();
}

describe('manifestParts', () => {
	it('returns every part of a composite actor in order', () => {
		const parts = manifestParts(MANIFEST, 'BP_LevelObject_Relic_C');
		expect(parts.map((p) => p.mesh)).toEqual(['SM_JewelBase_a1', 'SM_Jewel_b2']);
	});

	it('returns an empty list for an unknown class rather than throwing', () => {
		expect(manifestParts(MANIFEST, 'BP_LevelObject_Nope_C')).toEqual([]);
	});
});

describe('meshNames', () => {
	it('lists each distinct mesh once across all classes', () => {
		expect(meshNames(MANIFEST).sort()).toEqual([
			'SM_FastTravelStatue_c3',
			'SM_JewelBase_a1',
			'SM_Jewel_b2'
		]);
	});

	it('counts a mesh shared by two classes once', () => {
		expect(meshNames(MANIFEST).filter((n) => n === 'SM_JewelBase_a1')).toHaveLength(1);
	});
});

describe('mapObjectInstanceMatrix', () => {
	it('produces a finite, invertible matrix', () => {
		const m = mapObjectInstanceMatrix(BASE, 0, 0, 0, 'MainMap', 1e-9);
		expect(m.elements.every(Number.isFinite)).toBe(true);
		expect(Math.abs(m.determinant())).toBeGreaterThan(0);
	});

	it('places two different world positions at different points', () => {
		const a = positionOf(mapObjectInstanceMatrix(BASE, 0, 0, 0, 'MainMap', 1e-9));
		const b = positionOf(mapObjectInstanceMatrix(BASE, 100000, 0, 0, 'MainMap', 1e-9));
		expect(a.distanceTo(b)).toBeGreaterThan(0);
	});

	it('lifts a part with a positive local Z above one without', () => {
		const zOf = (p: typeof BASE) =>
			positionOf(mapObjectInstanceMatrix(p, 0, 0, 0, 'MainMap', 1e-9)).z;
		expect(zOf(JEWEL)).toBeGreaterThan(zOf(BASE));
	});

	it('scales with cmToMerc', () => {
		const small = mapObjectInstanceMatrix(BASE, 0, 0, 0, 'MainMap', 1e-9);
		const large = mapObjectInstanceMatrix(BASE, 0, 0, 0, 'MainMap', 2e-9);
		expect(Math.abs(large.determinant())).toBeGreaterThan(Math.abs(small.determinant()));
	});

	it('scales by exactly the part scale times cmToMerc', () => {
		const cmToMerc = 3e-9;
		const scale = new THREE.Vector3().setFromMatrixScale(
			mapObjectInstanceMatrix(
				MANIFEST.BP_LevelObject_Relic_Other_C.parts[0],
				0,
				0,
				0,
				'MainMap',
				cmToMerc
			)
		);
		expect(scale.x).toBeCloseTo(2 * cmToMerc, 20);
		expect(scale.y).toBeCloseTo(2 * cmToMerc, 20);
		expect(scale.z).toBeCloseTo(2 * cmToMerc, 20);
	});

	it('offsets a part by its local centimetres through cmToMerc', () => {
		const cmToMerc = 1e-9;
		const base = positionOf(mapObjectInstanceMatrix(BASE, 0, 0, 0, 'MainMap', cmToMerc));
		const jewel = positionOf(mapObjectInstanceMatrix(JEWEL, 0, 0, 0, 'MainMap', cmToMerc));
		expect(jewel.x - base.x).toBeCloseTo(-20 * cmToMerc, 14);
		expect(jewel.y - base.y).toBeCloseTo(-30 * cmToMerc, 14);
		expect(jewel.z - base.z).toBeCloseTo(120 * cmToMerc, 20);
	});

	// Routing a horizontal part offset through worldToPixel treats centimetres as
	// map pixels -- ~177 cm/px on MainMap, a ~700x error that detaches a jewel
	// from its pedestal by tens of metres.
	it('maps a local horizontal offset onto UE +X -> mercator -y, UE +Y -> mercator +x', () => {
		const cmToMerc = 1e-9;
		const at = (loc: [number, number, number]) =>
			positionOf(mapObjectInstanceMatrix({ ...BASE, loc }, 0, 0, 0, 'MainMap', cmToMerc));
		const origin = at([0, 0, 0]);
		const alongUeX = at([100, 0, 0]).sub(origin);
		const alongUeY = at([0, 100, 0]).sub(origin);
		expect(alongUeX.x).toBeCloseTo(0, 14);
		expect(alongUeX.y).toBeCloseTo(-100 * cmToMerc, 14);
		expect(alongUeX.z).toBeCloseTo(0, 20);
		expect(alongUeY.x).toBeCloseTo(100 * cmToMerc, 14);
		expect(alongUeY.y).toBeCloseTo(0, 14);
		expect(alongUeY.z).toBeCloseTo(0, 20);
	});

	it('raises a part by the actor world Z through cmToMerc', () => {
		const cmToMerc = 1e-9;
		const low = positionOf(mapObjectInstanceMatrix(BASE, 0, 0, 0, 'MainMap', cmToMerc));
		const high = positionOf(mapObjectInstanceMatrix(BASE, 0, 0, 5000, 'MainMap', cmToMerc));
		expect(high.z - low.z).toBeCloseTo(5000 * cmToMerc, 20);
	});

	// The manifest stores degrees but ueYawToThreeQuaternion takes radians, so
	// feeding rot[1] raw turns 90 into 90 radians -- a 117 degree turn, which
	// looks like a statue facing an odd but not obviously wrong way.
	it('reads part yaw as degrees, not radians', () => {
		const yawed = {
			...BASE,
			rot: [0, 90, 0] as [number, number, number]
		};
		const straight = forwardOf(mapObjectInstanceMatrix(BASE, 0, 0, 0, 'MainMap', 1e-9));
		const turned = forwardOf(mapObjectInstanceMatrix(yawed, 0, 0, 0, 'MainMap', 1e-9));
		expect(straight.angleTo(turned)).toBeCloseTo(Math.PI / 2, 9);
		expect(
			turned.angleTo(straight.clone().applyAxisAngle(new THREE.Vector3(0, 0, 1), 90 * DEG))
		).toBeCloseTo(0, 9);
	});

	it('keeps yaw in the horizontal plane', () => {
		const yawed = { ...BASE, rot: [0, 90, 0] as [number, number, number] };
		expect(forwardOf(mapObjectInstanceMatrix(yawed, 0, 0, 0, 'MainMap', 1e-9)).z).toBeCloseTo(0, 9);
	});

	it('matches the structure mesh chain for a yaw-zero actor', () => {
		const s = {
			instance_id: 'x',
			map_object_id: 'x',
			x: -300000,
			y: 120000,
			z: 4500,
			yaw: 0,
			scale_x: 1,
			scale_y: 1,
			scale_z: 1,
			hp_current: 1,
			hp_max: 1,
			build_player_uid: 'x'
		} satisfies BaseStructure;
		const expected = meshInstanceMatrix(s, JEWEL, 'MainMap', 1, 1e-9);
		const actual = mapObjectInstanceMatrix(JEWEL, s.x, s.y, s.z, 'MainMap', 1e-9);
		actual.elements.forEach((v, i) => expect(v).toBeCloseTo(expected.elements[i], 20));
	});

	it('orients the mesh with the same flip the other 3D layers use', () => {
		const m = mapObjectInstanceMatrix(BASE, 0, 0, 0, 'MainMap', 1e-9);
		const rotation = new THREE.Matrix4().extractRotation(m);
		rotation.elements.forEach((v, i) => expect(v).toBeCloseTo(MESH_FLIP.elements[i], 12));
	});
});
