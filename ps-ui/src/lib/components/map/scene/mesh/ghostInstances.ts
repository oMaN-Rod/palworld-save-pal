// Bake/compose split for ghost instance matrices, the counterpart of
// structureInstances for the blueprint preview. ghostInstanceMatrix reduces to
//
//   T(merc.x, merc.y, altCm * c) . (c . B),   B = MESH_FLIP . R . S(scale) . partLocal
//
// with c = cmToMerc the only camera-derived term, and a uniform c factoring out of
// B's whole 3x4 block. So B and the mercator anchor bake once per anchor change and
// a camera-only change is fifteen multiplies per instance. The tests pin this
// against ghostInstanceMatrix element for element.
import * as THREE from 'three';
import type { Quat, Vec3 } from '$types';
import type { MapArea } from '../../geo/utils';
import { worldToPixel } from '../../geo/utils';
import { pixelToMercator } from '../../geo/mercator';
import { ueQuatToThree } from '../../geo/coords3d';
import { MESH_FLIP } from '../structures/structureLayer';
import { partLocalMatrix, type MeshPart } from './meshPlacement';

// Floats per baked instance: mercator anchorX, anchorY, altitudeCm, then B's 3x3
// (column-major) and B's translation column.
export const GHOST_BAKE_STRIDE = 15;

// Float64, not the Float32 structureInstances bakes into: a mercator coordinate is
// a fraction of the world's width, where a float32 ULP is metres on the ground.
export type GhostBake = Float64Array;

// A manifest part is a stable object shared by every structure of its type, so its
// local matrix is worth keeping rather than rebuilding per instance per frame.
const partCache = new WeakMap<MeshPart, THREE.Matrix4>();

function cachedPartMatrix(part: MeshPart): THREE.Matrix4 {
	let hit = partCache.get(part);
	if (!hit) {
		hit = partLocalMatrix(part);
		partCache.set(part, hit);
	}
	return hit;
}

const scratchBasis = new THREE.Matrix4();
const scratchRotation = new THREE.Matrix4();
const scratchScale = new THREE.Matrix4();

export function bakeGhostInstance(
	world: { translation: Vec3; rotation: Quat; scale: Vec3 },
	part: MeshPart,
	area: MapArea,
	target: GhostBake,
	offset: number
): void {
	const [px, py] = worldToPixel(world.translation.x, world.translation.y, area);
	// pixelToLngLat -> MercatorCoordinate.fromLngLat, minus the two transcendentals
	// that cancel between them; this runs once per instance per rebake.
	const [anchorX, anchorY] = pixelToMercator(px, py);

	scratchRotation.makeRotationFromQuaternion(
		ueQuatToThree(world.rotation.x, world.rotation.y, world.rotation.z, world.rotation.w)
	);
	// Per-structure scale in the same (x, z, y) axis order partLocalMatrix uses.
	const s = world.scale;
	scratchScale.makeScale(s.x, s.z, s.y);
	const e = scratchBasis
		.copy(MESH_FLIP)
		.multiply(scratchRotation)
		.multiply(scratchScale)
		.multiply(cachedPartMatrix(part)).elements;

	target[offset] = anchorX;
	target[offset + 1] = anchorY;
	target[offset + 2] = world.translation.z;
	target[offset + 3] = e[0];
	target[offset + 4] = e[1];
	target[offset + 5] = e[2];
	target[offset + 6] = e[4];
	target[offset + 7] = e[5];
	target[offset + 8] = e[6];
	target[offset + 9] = e[8];
	target[offset + 10] = e[9];
	target[offset + 11] = e[10];
	target[offset + 12] = e[12];
	target[offset + 13] = e[13];
	target[offset + 14] = e[14];
}

export function composeGhostMatrix(
	baked: GhostBake,
	offset: number,
	cmToMerc: number,
	target: THREE.Matrix4
): void {
	const o = offset;
	const c = cmToMerc;
	const e = target.elements;
	e[0] = baked[o + 3] * c;
	e[1] = baked[o + 4] * c;
	e[2] = baked[o + 5] * c;
	e[3] = 0;
	e[4] = baked[o + 6] * c;
	e[5] = baked[o + 7] * c;
	e[6] = baked[o + 8] * c;
	e[7] = 0;
	e[8] = baked[o + 9] * c;
	e[9] = baked[o + 10] * c;
	e[10] = baked[o + 11] * c;
	e[11] = 0;
	e[12] = baked[o] + baked[o + 12] * c;
	e[13] = baked[o + 1] + baked[o + 13] * c;
	e[14] = (baked[o + 2] + baked[o + 14]) * c;
	e[15] = 1;
}
