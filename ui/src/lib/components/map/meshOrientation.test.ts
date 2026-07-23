// Cross-check: the 3D mesh path (meshInstanceMatrix, structureLayer.ts) must
// map a direction expressed in UE space to the same mercator direction that
// flat mode (buildStructureFC, features.ts -- shipped, visually verified)
// would put it. Ground truth is derived independently of meshInstanceMatrix's
// own axis-swap/UP_FLIP internals: it rotates a UE-space offset by yaw exactly
// as buildStructureFC's ring does, then runs it through the same
// worldToPixel/pixelToLngLat/MercatorCoordinate chain flat mode uses.
import { describe, it, expect } from 'vitest';
import * as THREE from 'three';
import { MercatorCoordinate } from 'maplibre-gl';
import { meshInstanceMatrix } from './structureLayer';
import { worldToPixel } from './utils';
import { pixelToLngLat } from './mercator';
import type { MeshPart } from './meshPlacement';
import type { BaseStructure } from '$types';

const identityPart: MeshPart = { loc: [0, 0, 0], rot: [0, 0, 0], scale: [1, 1, 1] };
const AREA = 'MainMap';
const ORIGIN_X = -50000;
const ORIGIN_Y = 100000;
const EPS = 1000; // 10m in UE cm -- small relative to MainMap's ~2.2M cm span

function toMerc(worldX: number, worldY: number): THREE.Vector3 {
	const [px, py] = worldToPixel(worldX, worldY, AREA);
	const [lng, lat] = pixelToLngLat(px, py);
	const m = MercatorCoordinate.fromLngLat([lng, lat], 0);
	return new THREE.Vector3(m.x, m.y, m.z);
}

// buildStructureFC's ring: wx = cx + dx*cos - dy*sin, wy = cy + dx*sin + dy*cos.
// Edge (-hx,-hy)->(hx,-hy) is pure +dx (UE local +X); edge (hx,-hy)->(hx,hy) is
// pure +dy (UE local +Y). Differentiating gives the rotated unit directions.
function flatModeDirections(yaw: number): { dirX: THREE.Vector3; dirY: THREE.Vector3 } {
	const cos = Math.cos(yaw);
	const sin = Math.sin(yaw);
	const center = toMerc(ORIGIN_X, ORIGIN_Y);

	const ueLocalX = { dx: cos, dy: sin };
	const ueLocalY = { dx: -sin, dy: cos };

	function dirFor(d: { dx: number; dy: number }): THREE.Vector3 {
		const p = toMerc(ORIGIN_X + d.dx * EPS, ORIGIN_Y + d.dy * EPS);
		return p.clone().sub(center).normalize();
	}

	return { dirX: dirFor(ueLocalX), dirY: dirFor(ueLocalY) };
}

// mesh-local axes per the documented ue.(x,z,y) swap: local +X = UE +X (a
// footprint's local x-axis), local +Z = UE +Y (a footprint's local y-axis).
function meshPathDirections(yaw: number): { dirX: THREE.Vector3; dirY: THREE.Vector3 } {
	const s: BaseStructure = {
		instance_id: 'i',
		map_object_id: 'm',
		x: ORIGIN_X,
		y: ORIGIN_Y,
		z: 0,
		yaw,
		scale_x: 1,
		scale_y: 1,
		scale_z: 1,
		hp_current: 1,
		hp_max: 1,
		build_player_uid: 'u'
	};
	const matrix = meshInstanceMatrix(s, identityPart, AREA, 1, 1);
	const dirX = new THREE.Vector3(1, 0, 0).transformDirection(matrix);
	const dirY = new THREE.Vector3(0, 0, 1).transformDirection(matrix);
	return { dirX, dirY };
}

describe('mesh orientation matches flat-mode ground truth', () => {
	const yaws = [0, Math.PI / 2, -Math.PI / 2, Math.PI, (37 * Math.PI) / 180];

	for (const yaw of yaws) {
		it(`UE +X direction agrees with flat mode at yaw=${yaw.toFixed(4)}`, () => {
			const flat = flatModeDirections(yaw);
			const mesh = meshPathDirections(yaw);
			expect(flat.dirX.dot(mesh.dirX)).toBeCloseTo(1, 3);
		});

		it(`UE +Y direction agrees with flat mode at yaw=${yaw.toFixed(4)}`, () => {
			const flat = flatModeDirections(yaw);
			const mesh = meshPathDirections(yaw);
			expect(flat.dirY.dot(mesh.dirY)).toBeCloseTo(1, 3);
		});

		it(`handedness (dirX x dirY) matches flat mode at yaw=${yaw.toFixed(4)}`, () => {
			const flat = flatModeDirections(yaw);
			const mesh = meshPathDirections(yaw);
			const flatCross = flat.dirX.clone().cross(flat.dirY);
			const meshCross = mesh.dirX.clone().cross(mesh.dirY);
			// Both crosses should be (near) parallel to the shared up axis (z); compare sign.
			expect(Math.sign(flatCross.z)).toBe(Math.sign(meshCross.z));
		});
	}
});
