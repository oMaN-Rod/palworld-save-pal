// Bake/compose split for structure instance matrices: everything but the
// camera-derived cmToMerc is baked once, so a camera-only update touches nothing
// but that scale. Both structureLayer matrix builders already reduce to
// T(anchor.x, anchor.y, altitudeCm * cmToMerc) . R . S(cmToMerc) with
// R = MESH_FLIP * yaw, which the tests use as the oracle here.
import * as THREE from 'three';
import { MercatorCoordinate } from 'maplibre-gl';
import { MESH_FLIP } from './structureLayer';
import { ueYawToThreeQuaternion } from './coords3d';

export type StructureAnchorInput = {
	lng: number;
	lat: number;
	altitudeCm: number;
	yaw: number;
};

// Floats per baked instance: anchorX, anchorY, altitudeCm, then R's 3x3
// (MESH_FLIP * yaw), column-major.
export const STRUCTURE_BAKE_STRIDE = 12;

export function bakeStructureInstance(anchor: StructureAnchorInput): Float32Array {
	const mercAnchor = MercatorCoordinate.fromLngLat([anchor.lng, anchor.lat]);
	const yawRotation = new THREE.Matrix4().makeRotationFromQuaternion(
		ueYawToThreeQuaternion(anchor.yaw)
	);
	const rotation = MESH_FLIP.clone().multiply(yawRotation);
	const e = rotation.elements;
	return new Float32Array([
		mercAnchor.x,
		mercAnchor.y,
		anchor.altitudeCm,
		e[0],
		e[1],
		e[2],
		e[4],
		e[5],
		e[6],
		e[8],
		e[9],
		e[10]
	]);
}

// A uniform scale commutes with the rotation, so cmToMerc applies by scaling R's
// 3x3 and the altitude, then translating by the unscaled mercator anchor.
export function composeStructureMatrix(
	baked: Float32Array,
	offset: number,
	cmToMerc: number,
	target: THREE.Matrix4
): void {
	const o = offset;
	const e = target.elements;
	e[0] = baked[o + 3] * cmToMerc;
	e[1] = baked[o + 4] * cmToMerc;
	e[2] = baked[o + 5] * cmToMerc;
	e[3] = 0;
	e[4] = baked[o + 6] * cmToMerc;
	e[5] = baked[o + 7] * cmToMerc;
	e[6] = baked[o + 8] * cmToMerc;
	e[7] = 0;
	e[8] = baked[o + 9] * cmToMerc;
	e[9] = baked[o + 10] * cmToMerc;
	e[10] = baked[o + 11] * cmToMerc;
	e[11] = 0;
	e[12] = baked[o];
	e[13] = baked[o + 1];
	e[14] = baked[o + 2] * cmToMerc;
	e[15] = 1;
}
