import { describe, it, expect } from 'vitest';
import * as THREE from 'three';
import { MercatorCoordinate } from 'maplibre-gl';
import {
	meshInstanceMatrix,
	proxyInstanceMatrix,
	pickPixelCoords,
	createStructureLayer
} from './structureLayer';
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

describe('pickPixelCoords', () => {
	it('converts CSS pixels to device pixels at ratio 1', () => {
		expect(pickPixelCoords(10, 20, 1, 100, 50)).toEqual({ x: 10, y: 29 });
	});

	it('converts CSS pixels to device pixels at ratio 2 (HiDPI)', () => {
		expect(pickPixelCoords(10, 20, 2, 200, 100)).toEqual({ x: 20, y: 59 });
	});

	it('flips Y so CSS y=0 (top) lands on the last device row (bottom-left origin)', () => {
		expect(pickPixelCoords(0, 0, 1, 100, 50)).toEqual({ x: 0, y: 49 });
	});

	it('rejects a point beyond the right edge', () => {
		expect(pickPixelCoords(100, 0, 1, 100, 50)).toBeNull();
	});

	it('rejects a point beyond the bottom edge', () => {
		expect(pickPixelCoords(0, 50, 1, 100, 50)).toBeNull();
	});

	it('rejects negative CSS x', () => {
		expect(pickPixelCoords(-1, 0, 1, 100, 50)).toBeNull();
	});

	it('rejects negative CSS y', () => {
		expect(pickPixelCoords(0, -1, 1, 100, 50)).toBeNull();
	});

	it('accepts the last valid pixel (width-1, height-1) without an off-by-one', () => {
		expect(pickPixelCoords(99, 0, 1, 100, 50)).toEqual({ x: 99, y: 49 });
		expect(pickPixelCoords(0, 49, 1, 100, 50)).toEqual({ x: 0, y: 0 });
	});
});

describe('GPU pick base upload across groups (C1)', () => {
	// scene.overrideMaterial shares one ShaderMaterial instance across every
	// InstancedMesh three draws; three only re-uploads a ShaderMaterial's
	// uniforms when the program or material identity changes. Without
	// uniformsNeedUpdate, only the first drawn group's uPickBase would ever
	// reach the GPU and every later group would rasterize with that base.
	it("uploads each group's own pickBase, not just the first", () => {
		const layer = createStructureLayer({ id: 'test-pick-base' });
		const stubMap = { getCenter: () => ({ lng: 0, lat: 0 }), triggerRepaint: () => {} };
		layer.attachMapForTest(stubMap as unknown as Parameters<typeof layer.attachMapForTest>[0]);

		const footprints: Record<string, Footprint> = {
			PickTestBucketA: { sx: 100, sy: 100, sz: 100, ox: 0, oy: 0, oz: 0, typeA: 'Foundation' },
			PickTestBucketB: { sx: 200, sy: 150, sz: 80, ox: 0, oy: 0, oz: 0, typeA: 'Furniture' }
		};
		const structures = [
			base({ instance_id: 'a0', map_object_id: 'PickTestBucketA' }),
			base({ instance_id: 'a1', map_object_id: 'PickTestBucketA' }),
			base({ instance_id: 'b0', map_object_id: 'PickTestBucketB' }),
			base({ instance_id: 'b1', map_object_id: 'PickTestBucketB' })
		];

		layer.update(structures, footprints, 'MainMap', 1);

		const groups = layer.groupsForTest();
		expect(groups.length).toBe(2);

		for (const group of groups) {
			const fakeMaterial = { uniforms: { uPickBase: { value: -1 } } };
			group.mesh.onBeforeRender(
				null as any,
				null as any,
				null as any,
				null as any,
				fakeMaterial as unknown as THREE.Material,
				null as any
			);

			expect(fakeMaterial.uniforms.uPickBase.value).toBe(group.pickBase);
			expect((fakeMaterial as unknown as THREE.ShaderMaterial).uniformsNeedUpdate).toBe(true);

			for (let i = 0; i < group.keys.length; i++) {
				expect(layer.keyAtForTest(group.pickBase + i)).toBe(group.keys[i]);
			}
		}
	});
});
