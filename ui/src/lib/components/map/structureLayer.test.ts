import { describe, it, expect } from 'vitest';
import * as THREE from 'three';
import { MercatorCoordinate } from 'maplibre-gl';
import { meshInstanceMatrix, proxyInstanceMatrix } from './structureLayer';
import { structureAnchor } from './structurePlacement';
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

// Wooden_DoorWall's real footprint offset from the manifest -- carried by 791 of
// 815 mesh-backed structure ids. A regression that lets it leak into the mesh
// path again would shift every one of them off the ground.
const fpWithBoxOffset: Footprint = {
	sx: 400,
	sy: 20,
	sz: 325,
	ox: 0,
	oy: 0,
	oz: 158.8,
	typeA: 'Foundation',
	archetype: 'wallDoor'
};

function altitudeCmFromMatrix(matrix: THREE.Matrix4, lng: number, lat: number, verticalScale: number): number {
	const position = new THREE.Vector3().setFromMatrixPosition(matrix);
	const mPerUnit = MercatorCoordinate.fromLngLat([lng, lat], 0).meterInMercatorCoordinateUnits();
	return position.z / mPerUnit / verticalScale;
}

describe('meshInstanceMatrix', () => {
	it('anchors mesh altitude at the raw actor z, not the collision-box footprint offset', () => {
		const s = base({ z: 5000 });
		const anchor = structureAnchor(s, 'MainMap');
		const matrix = meshInstanceMatrix(s, identityPart, 'MainMap', 1, 1);
		const altitude = altitudeCmFromMatrix(matrix, anchor.lng, anchor.lat, 1);

		expect(altitude).toBeCloseTo(s.z, 5);
		expect(altitude).not.toBeCloseTo(s.z + fpWithBoxOffset.oz, 5);
	});
});

describe('proxyInstanceMatrix (unchanged proxy behavior)', () => {
	it('still bakes the footprint origin/half-height offset in for the proxy box path', () => {
		const s = base({ z: 5000 });
		const anchor = structureAnchor(s, 'MainMap');
		const matrix = proxyInstanceMatrix(s, fpWithBoxOffset, 'wallDoor', 'MainMap', 1, 1);
		const altitude = altitudeCmFromMatrix(matrix, anchor.lng, anchor.lat, 1);

		const halfH = fpWithBoxOffset.sz / 2;
		expect(altitude).toBeCloseTo(s.z + fpWithBoxOffset.oz - halfH, 5);
	});
});
