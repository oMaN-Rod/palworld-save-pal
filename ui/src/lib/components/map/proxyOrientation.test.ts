// Cross-check: the 3D proxy-geometry path (proxyInstanceMatrix, structureLayer.ts)
// must map a UE-local direction to the same mercator direction that flat mode
// (buildStructureFC, features.ts -- shipped, visually verified) puts it.
// Tighter than the mesh cross-check in meshOrientation.test.ts: both paths
// consume the same footprint dimensions (fp.sx/fp.sy), so ring corners can be
// compared directly instead of via a small synthetic offset.
import { describe, it, expect } from 'vitest';
import * as THREE from 'three';
import { MercatorCoordinate } from 'maplibre-gl';
import { proxyInstanceMatrix } from './structureLayer';
import { buildStructureFC } from './features';
import type { BaseStructure, Footprint } from '$types';

const AREA = 'MainMap';
const ORIGIN_X = -50000;
const ORIGIN_Y = 100000;

// Non-square on purpose: sx != sy makes an X/Y axis swap visible, not only a
// single-axis sign flip.
const fp: Footprint = { sx: 400, sy: 300, sz: 200, ox: 0, oy: 0, oz: 0, typeA: 'Other' };

function structure(yaw: number): BaseStructure {
	return {
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
}

function toMerc([lng, lat]: [number, number]): THREE.Vector3 {
	const m = MercatorCoordinate.fromLngLat([lng, lat], 0);
	return new THREE.Vector3(m.x, m.y, m.z);
}

// buildStructureFC's ring, in order, for a footprint centered on the structure
// (fp.ox = fp.oy = 0): [-hx,-hy], [hx,-hy], [hx,hy], [-hx,hy], closing dup.
// corner1-corner0 is a pure +dx (UE local +X) step; corner2-corner1 is a pure
// +dy (UE local +Y) step -- see buildStructureFC's ring comment in features.ts.
function flatModeDirections(yaw: number): { dirX: THREE.Vector3; dirY: THREE.Vector3 } {
	const fc = buildStructureFC([structure(yaw)], { m: fp }, 0, AREA);
	const ring = fc.features[0].geometry.coordinates[0];
	const [c0, c1, c2] = [toMerc(ring[0]), toMerc(ring[1]), toMerc(ring[2])];
	return {
		dirX: c1.clone().sub(c0).normalize(),
		dirY: c2.clone().sub(c1).normalize()
	};
}

// Proxy-geometry local axes (proxyGeometry.ts's BoxGeometry(sx, sz, sy)): local
// +X is the fp.sx dimension (UE local +X), local +Z is the fp.sy dimension (UE
// local +Y) -- the same ue.(x,z,y) swap the mesh path documents. Corners are
// taken at local y=0 so the untouched altitude axis contributes nothing to
// this horizontal comparison.
function proxyPathDirections(yaw: number): { dirX: THREE.Vector3; dirY: THREE.Vector3 } {
	const matrix = proxyInstanceMatrix(structure(yaw), fp, 'box', AREA, 1, 1);
	const hx = fp.sx / 2;
	const hy = fp.sy / 2;
	const p0 = new THREE.Vector3(-hx, 0, -hy).applyMatrix4(matrix);
	const p1 = new THREE.Vector3(hx, 0, -hy).applyMatrix4(matrix);
	const p2 = new THREE.Vector3(hx, 0, hy).applyMatrix4(matrix);
	return {
		dirX: p1.clone().sub(p0).normalize(),
		dirY: p2.clone().sub(p1).normalize()
	};
}

describe('proxy geometry orientation matches flat-mode ground truth', () => {
	const yaws = [0, Math.PI / 2, -Math.PI / 2, Math.PI, (37 * Math.PI) / 180];

	for (const yaw of yaws) {
		it(`UE +X direction agrees with flat mode at yaw=${yaw.toFixed(4)}`, () => {
			const flat = flatModeDirections(yaw);
			const proxy = proxyPathDirections(yaw);
			expect(flat.dirX.dot(proxy.dirX)).toBeCloseTo(1, 3);
		});

		it(`UE +Y direction agrees with flat mode at yaw=${yaw.toFixed(4)}`, () => {
			const flat = flatModeDirections(yaw);
			const proxy = proxyPathDirections(yaw);
			expect(flat.dirY.dot(proxy.dirY)).toBeCloseTo(1, 3);
		});

		it(`handedness (dirX x dirY) matches flat mode at yaw=${yaw.toFixed(4)}`, () => {
			const flat = flatModeDirections(yaw);
			const proxy = proxyPathDirections(yaw);
			const flatCross = flat.dirX.clone().cross(flat.dirY);
			const proxyCross = proxy.dirX.clone().cross(proxy.dirY);
			expect(Math.sign(flatCross.z)).toBe(Math.sign(proxyCross.z));
		});
	}
});
