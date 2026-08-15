import { beforeEach, describe, it, expect, vi } from 'vitest';
import * as THREE from 'three';
import type { Map as MLMap } from 'maplibre-gl';
import manifestJson from '../../../../../data/json/map_object_meshes.json';
import {
	createMapObjectLayer,
	bakeMapObjectInstances,
	composeMapObjectMatrices,
	bakeMapObjectPortalInstances,
	composeMapObjectPortalMatrices,
	cullDistanceCmFor,
	viewRadiusCm,
	MAP_OBJECT_DEFAULT_CULL_CM,
	type MapObjectItem
} from './mapObjectLayer';
import { manifestParts, mapObjectInstanceMatrix, type MapObjectManifest } from './mapObjectMesh';
import { mapObjectPortalMatrix, FAST_TRAVEL_RADIUS_CM, RELIC_RADIUS_CM } from './mapObjectPortal';
import { CORE_COLOR } from './palPortal';
import { pixelToLngLat, lngLatToPixel } from './mercator';
import { pixelToWorld, worldToPixel } from './utils';
import { partLocalMatrix, ueEulerToThreeQuaternion } from './meshPlacement';
import { MESH_FLIP } from './structureLayer';

// Only requestMapObjectMesh is replaced: an InstancedMesh over real geometry
// needs no GL context, so the compose loop runs for real -- which is what lets
// the bake/compose split test below actually fail.
const meshes = vi.hoisted(() => ({ requestMapObjectMesh: vi.fn() }));
vi.mock('./mapObjectMeshLibrary', async (importOriginal) => ({
	...(await importOriginal<typeof import('./mapObjectMeshLibrary')>()),
	requestMapObjectMesh: meshes.requestMapObjectMesh
}));

const MANIFEST = manifestJson as unknown as MapObjectManifest;

const STATUE_CLASS = 'BP_LevelObject_TowerFastTravelPoint_C';
const STATUE_MESH = 'SM_FastTravelStatue_61071e';
const RELIC_CLASS = 'BP_LevelObject_Relic_FlameBambi_C';
const WATCHTOWER_CLASS = 'BP_LevelObject_UnlockMapPoint_C';

const ITEM: MapObjectItem = {
	x: 0,
	y: 0,
	z: 0,
	actorClass: STATUE_CLASS,
	scale: 1,
	portalColor: '#4fc3ff',
	ringRadiusCm: FAST_TRAVEL_RADIUS_CM,
	rot: [0, 0, 0]
};

// Baked instances are Float32, and one matrix mixes mercator anchors (~1e-1)
// with centimetre offsets (~1e3), so the tolerance has to be relative.
function expectMatrixNear(got: Float32Array, at: number, want: ArrayLike<number>) {
	for (let e = 0; e < 16; e++) {
		expect(Math.abs(got[at * 16 + e] - want[e])).toBeLessThanOrEqual(
			Math.max(1e-9, Math.abs(want[e]) * 1e-6)
		);
	}
}

// Enough of a map for compose(): it reads the centre, the bounds and asks for a
// repaint, and nothing else.
function stubMap(lng: number, lat: number, spanDeg = 0.05) {
	const state = { lng, lat, spanDeg };
	const map = {
		getCenter: () => ({ lng: state.lng, lat: state.lat }),
		getBounds: () => ({
			getSouthWest: () => ({ lng: state.lng - state.spanDeg, lat: state.lat - state.spanDeg }),
			getNorthEast: () => ({ lng: state.lng + state.spanDeg, lat: state.lat + state.spanDeg })
		}),
		triggerRepaint: () => {}
	};
	return { map: map as unknown as MLMap, state };
}

function lngLatOf(worldX: number, worldY: number): [number, number] {
	return pixelToLngLat(...worldToPixel(worldX, worldY, 'MainMap'));
}

// A layer with a stub map centred on the first item, already updated once.
function mounted(items: typeof ITEM[]) {
	const [lng, lat] = lngLatOf(items[0].x, items[0].y);
	const { map, state } = stubMap(lng, lat);
	const layer = createMapObjectLayer('map-objects-3d');
	layer.attachMapForTest(map);
	layer.update(items, 'MainMap', 1e-9);
	return { layer, state };
}

beforeEach(() => {
	const cache = new Map<string, { geometry: THREE.BufferGeometry; material: THREE.Material }>();
	meshes.requestMapObjectMesh.mockReset();
	meshes.requestMapObjectMesh.mockImplementation((name: string) => {
		let bundle = cache.get(name);
		if (!bundle) {
			bundle = { geometry: new THREE.BoxGeometry(100, 100, 100), material: new THREE.MeshStandardMaterial() };
			cache.set(name, bundle);
		}
		return bundle;
	});
});

describe('createMapObjectLayer', () => {
	it('starts with nothing baked', () => {
		expect(createMapObjectLayer('map-objects-3d').bakeCount()).toBe(0);
	});

	it('bakes once when the item list changes', () => {
		const layer = createMapObjectLayer('map-objects-3d');
		layer.update([ITEM], 'MainMap', 1e-9);
		expect(layer.bakeCount()).toBe(1);
		layer.update([ITEM], 'MainMap', 1e-9);
		expect(layer.bakeCount()).toBe(1);
	});

	it('re-bakes when the item list actually changes', () => {
		const layer = createMapObjectLayer('map-objects-3d');
		layer.update([ITEM], 'MainMap', 1e-9);
		layer.update([ITEM, { ...ITEM, x: 5000 }], 'MainMap', 1e-9);
		expect(layer.bakeCount()).toBe(2);
	});

	it('re-bakes when the area changes', () => {
		const layer = createMapObjectLayer('map-objects-3d');
		layer.update([ITEM], 'MainMap', 1e-9);
		layer.update([ITEM], 'Tree', 1e-9);
		expect(layer.bakeCount()).toBe(2);
	});

	it('re-bakes when only the scale changes', () => {
		const layer = createMapObjectLayer('map-objects-3d');
		layer.update([{ ...ITEM, scale: 20 }], 'MainMap', 1);
		const baked = layer.bakeCount();
		layer.update([{ ...ITEM, scale: 40 }], 'MainMap', 1);
		expect(layer.bakeCount()).toBe(baked + 1);
	});

	it('re-bakes when only the portal colour changes', () => {
		const layer = createMapObjectLayer('map-objects-3d');
		layer.update([{ ...ITEM, portalColor: '#4fc3ff' }], 'MainMap', 1);
		const baked = layer.bakeCount();
		layer.update([{ ...ITEM, portalColor: '#ffa726' }], 'MainMap', 1);
		expect(layer.bakeCount()).toBe(baked + 1);
	});

	// Fails if sameItems ever stops comparing ringRadiusCm, which would keep
	// composing the old beam bucket into a stale mesh.
	it('re-bakes when only the ring radius changes', () => {
		const layer = createMapObjectLayer('map-objects-3d');
		layer.update([{ ...ITEM, ringRadiusCm: 140 }], 'MainMap', 1);
		const baked = layer.bakeCount();
		layer.update([{ ...ITEM, ringRadiusCm: 220 }], 'MainMap', 1);
		expect(layer.bakeCount()).toBe(baked + 1);
	});

	it('scales the instance transform by the item scale', () => {
		const small = createMapObjectLayer('map-objects-3d');
		const large = createMapObjectLayer('map-objects-3d');
		small.update([{ ...ITEM, scale: 1 }], 'MainMap', 1);
		large.update([{ ...ITEM, scale: 20 }], 'MainMap', 1);
		expect(Math.abs(large.bakedMatrixFor(0).determinant())).toBeGreaterThan(
			Math.abs(small.bakedMatrixFor(0).determinant())
		);
	});

	// Only meaningful with a map attached: without one compose() returns before
	// its body, and moving the bake in there would go unnoticed.
	it('does not re-bake when only the camera moved', () => {
		const { layer, state } = mounted([ITEM]);
		const baked = layer.bakeCount();
		expect(layer.groupsForTest()[0].count).toBe(1);

		state.lng += 0.002;
		layer.compose();
		state.lng += 0.002;
		layer.compose();

		expect(layer.bakeCount()).toBe(baked);
		expect(layer.composeCount()).toBe(3);
		expect(layer.groupsForTest()[0].count).toBe(1);
	});

	it('re-composes the instance matrices when the camera moves', () => {
		const { layer, state } = mounted([ITEM]);
		const before = meshes.requestMapObjectMesh.mock.calls.length;
		state.lng += 0.002;
		layer.compose();
		expect(meshes.requestMapObjectMesh.mock.calls.length).toBeGreaterThan(before);
	});

	// composeCount() counts invocations, not work, so the early-out is observed
	// through the mesh loop it skips.
	it('skips the mesh loop when nothing the scene depends on changed', () => {
		const { layer } = mounted([ITEM]);
		const before = meshes.requestMapObjectMesh.mock.calls.length;
		layer.compose();
		expect(meshes.requestMapObjectMesh.mock.calls.length).toBe(before);
		expect(layer.composeCount()).toBe(2);
	});

	it('drops every instance when given an empty list', () => {
		const { layer } = mounted([ITEM]);
		layer.update([], 'MainMap', 1e-9);
		expect(layer.instanceCount()).toBe(0);
		expect(layer.groupsForTest()).toHaveLength(0);
	});

	// Fast travel and relic items share this path but differ in ring radius, so
	// folding their beams into one InstancedMesh -- one geometry, one base
	// radius -- would break this.
	it('builds a separate beam mesh per distinct ring radius', () => {
		const { layer } = mounted([
			{ ...ITEM, ringRadiusCm: FAST_TRAVEL_RADIUS_CM },
			{
				x: 1000,
				y: 2000,
				z: 300,
				actorClass: RELIC_CLASS,
				scale: 1,
				portalColor: '#66bb6a',
				ringRadiusCm: RELIC_RADIUS_CM,
				rot: [0, 0, 0]
			}
		]);

		const beams = layer
			.groupsForTest()
			.filter((o) => (o as THREE.InstancedMesh).geometry.type === 'CylinderGeometry') as THREE.InstancedMesh[];
		expect(beams.length).toBe(2);

		const radii = beams
			.map((b) => {
				b.geometry.computeBoundingBox();
				const box = b.geometry.boundingBox!;
				return Math.round(Math.max(box.max.x, box.max.y));
			})
			.sort((a, b) => a - b);
		expect(radii).toEqual([RELIC_RADIUS_CM, FAST_TRAVEL_RADIUS_CM].sort((a, b) => a - b));
	});

	it('bakes one instance per part of every item', () => {
		const layer = createMapObjectLayer('map-objects-3d');
		layer.update(
			[
				ITEM,
				{
					x: 1000,
					y: 2000,
					z: 300,
					actorClass: RELIC_CLASS,
					scale: 1,
					portalColor: '#66bb6a',
					ringRadiusCm: RELIC_RADIUS_CM,
					rot: [0, 0, 0]
				}
			],
			'MainMap',
			1e-9
		);
		expect(layer.instanceCount()).toBe(3);
	});

	// The two shipped map object glbs with more than one primitive bundle into a
	// single geometry plus a material *array*, and SM_JewelBase alone is the
	// pedestal for all 12 relic actor classes. Anything that assumed a single
	// Material would render both with one texture instead of two.
	it('constructs an InstancedMesh with a material array for a multi-material bundle', () => {
		const materials = [new THREE.MeshStandardMaterial(), new THREE.MeshStandardMaterial()];
		meshes.requestMapObjectMesh.mockImplementation((name: string) =>
			name === STATUE_MESH
				? { geometry: new THREE.BoxGeometry(100, 100, 100), material: materials }
				: null
		);

		const [lng, lat] = lngLatOf(ITEM.x, ITEM.y);
		const { map } = stubMap(lng, lat);
		const layer = createMapObjectLayer('map-objects-3d');
		layer.attachMapForTest(map);

		expect(() => layer.update([ITEM], 'MainMap', 1e-9)).not.toThrow();

		const inst = layer.groupsForTest()[0] as THREE.InstancedMesh;
		expect(Array.isArray(inst.material)).toBe(true);
		expect(inst.material).toBe(materials);
	});
});

describe('bakeMapObjectInstances', () => {
	it('groups instances by mesh name', () => {
		const baked = bakeMapObjectInstances(
			[
				{
					x: 1000,
					y: 2000,
					z: 300,
					actorClass: RELIC_CLASS,
					scale: 1,
					portalColor: '#66bb6a',
					ringRadiusCm: RELIC_RADIUS_CM,
					rot: [0, 0, 0]
				},
				{
					x: 4000,
					y: 5000,
					z: 600,
					actorClass: RELIC_CLASS,
					scale: 1,
					portalColor: '#66bb6a',
					ringRadiusCm: RELIC_RADIUS_CM,
					rot: [0, 0, 0]
				}
			],
			'MainMap',
			MANIFEST
		);
		expect([...baked.keys()].sort()).toEqual(['SM_JewelBase_496ae3', 'SM_Relic_FlameBambi_0f22c4']);
	});

	it('ignores an actor class the manifest does not carry', () => {
		expect(
			bakeMapObjectInstances(
				[
					{
						x: 0,
						y: 0,
						z: 0,
						actorClass: 'BP_Nope_C',
						scale: 1,
						portalColor: '#4fc3ff',
						ringRadiusCm: FAST_TRAVEL_RADIUS_CM,
						rot: [0, 0, 0]
					}
				],
				'MainMap',
				MANIFEST
			)
				.size
		).toBe(0);
	});
});

describe('bakeMapObjectInstances + item rotation', () => {
	it('produces a different baked matrix for a yawed relic than for one at zero yaw', () => {
		const mesh = manifestParts(MANIFEST, RELIC_CLASS)[0].mesh;
		const straight = bakeMapObjectInstances(
			[
				{
					x: 0,
					y: 0,
					z: 0,
					actorClass: RELIC_CLASS,
					scale: 1,
					portalColor: '#4fc3ff',
					ringRadiusCm: RELIC_RADIUS_CM,
					rot: [0, 0, 0]
				}
			],
			'MainMap',
			MANIFEST
		);
		const yawed = bakeMapObjectInstances(
			[
				{
					x: 0,
					y: 0,
					z: 0,
					actorClass: RELIC_CLASS,
					scale: 1,
					portalColor: '#4fc3ff',
					ringRadiusCm: RELIC_RADIUS_CM,
					rot: [0, 90, 0]
				}
			],
			'MainMap',
			MANIFEST
		);
		expect(yawed.get(mesh)).not.toEqual(straight.get(mesh));
	});

	// The rotation must land in the actor's own frame, ahead of the part's
	// loc/rot/scale -- applied in world space afterwards it would spin the jewel's
	// offset about the wrong origin instead of the pedestal it sits on.
	it("applies the item's rotation in the actor frame, matching MESH_FLIP * item rotation * part-local", () => {
		const parts = manifestParts(MANIFEST, RELIC_CLASS);
		const jewel = parts[1];
		const rot: [number, number, number] = [0, 37, 0];
		const layer = createMapObjectLayer('map-objects-3d-test');
		layer.update(
			[
				{
					x: 0,
					y: 0,
					z: 0,
					actorClass: RELIC_CLASS,
					scale: 1,
					portalColor: '#4fc3ff',
					ringRadiusCm: RELIC_RADIUS_CM,
					rot
				}
			],
			'MainMap',
			1
		);

		const got = layer.bakedMatrixFor(1); // parts[0] bakes to index 0, parts[1] (jewel) to index 1
		const expected = MESH_FLIP.clone()
			.multiply(
				new THREE.Matrix4().makeRotationFromQuaternion(ueEulerToThreeQuaternion(rot[0], rot[1], rot[2]))
			)
			.multiply(partLocalMatrix(jewel));

		// bakedMatrixFor round-trips through the baked Float32Array, so precision is
		// float32, not float64.
		got.elements.forEach((v, i) => expect(v).toBeCloseTo(expected.elements[i], 6));
	});

	// A signed comparison would be wrong: MESH_FLIP's determinant is negative, so
	// every baked matrix's is too. Only the magnitude tracks rigidity.
	it('keeps the baked matrix invertible under a non-zero yaw, by determinant magnitude', () => {
		const layer = createMapObjectLayer('map-objects-3d-test');
		layer.update(
			[
				{
					x: 0,
					y: 0,
					z: 0,
					actorClass: RELIC_CLASS,
					scale: 1,
					portalColor: '#4fc3ff',
					ringRadiusCm: RELIC_RADIUS_CM,
					rot: [0, 128, 0]
				}
			],
			'MainMap',
			1
		);
		expect(Math.abs(layer.bakedMatrixFor(0).determinant())).toBeGreaterThan(0);
	});

	it('re-bakes when only the rotation changes', () => {
		const layer = createMapObjectLayer('map-objects-3d-test');
		layer.update([{ ...ITEM, rot: [0, 0, 0] }], 'MainMap', 1);
		const baked = layer.bakeCount();
		layer.update([{ ...ITEM, rot: [0, 45, 0] }], 'MainMap', 1);
		expect(layer.bakeCount()).toBe(baked + 1);
	});
});

// The split only pays off if its result is indistinguishable from computing the
// whole matrix from scratch every frame.
describe('bakeMapObjectInstances + composeMapObjectMatrices', () => {
	it('reproduces mapObjectInstanceMatrix for every instance it writes', () => {
		const cmToMerc = 0.6;
		const item: MapObjectItem = {
			x: 12345,
			y: -6789,
			z: 4200,
			actorClass: RELIC_CLASS,
			scale: 1,
			portalColor: '#4fc3ff',
			ringRadiusCm: RELIC_RADIUS_CM,
			rot: [0, 0, 0]
		};
		const baked = bakeMapObjectInstances([item], 'MainMap', MANIFEST);
		const parts = manifestParts(MANIFEST, RELIC_CLASS);

		for (const part of parts) {
			const target = new Float32Array(16);
			const written = composeMapObjectMatrices(
				baked.get(part.mesh)!,
				cmToMerc,
				item.x,
				item.y,
				0,
				target,
				0
			);
			expect(written).toBe(1);
			expectMatrixNear(
				target,
				0,
				mapObjectInstanceMatrix(part, item.x, item.y, item.z, 'MainMap', cmToMerc).elements
			);
		}
	});

	it('writes at the offset it is given, packing survivors contiguously', () => {
		const cmToMerc = 0.6;
		const near: MapObjectItem = {
			x: 0,
			y: 0,
			z: 0,
			actorClass: STATUE_CLASS,
			scale: 1,
			portalColor: '#4fc3ff',
			ringRadiusCm: FAST_TRAVEL_RADIUS_CM,
			rot: [0, 0, 0]
		};
		const far: MapObjectItem = {
			x: 0,
			y: 200000,
			z: 0,
			actorClass: STATUE_CLASS,
			scale: 1,
			portalColor: '#4fc3ff',
			ringRadiusCm: FAST_TRAVEL_RADIUS_CM,
			rot: [0, 0, 0]
		};
		const baked = bakeMapObjectInstances([far, near], 'MainMap', MANIFEST);
		const target = new Float32Array(48);

		const written = composeMapObjectMatrices(baked.get(STATUE_MESH)!, cmToMerc, 0, 0, 0, target, 1);

		expect(written).toBe(1);
		expectMatrixNear(
			target,
			1,
			mapObjectInstanceMatrix(
				manifestParts(MANIFEST, STATUE_CLASS)[0],
				near.x,
				near.y,
				near.z,
				'MainMap',
				cmToMerc
			).elements
		);
	});

	it("culls an instance past its class's own cull distance", () => {
		const baked = bakeMapObjectInstances(
			[
				{
					x: 0,
					y: 40000,
					z: 0,
					actorClass: STATUE_CLASS,
					scale: 1,
					portalColor: '#4fc3ff',
					ringRadiusCm: FAST_TRAVEL_RADIUS_CM,
					rot: [0, 0, 0]
				}
			],
			'MainMap',
			MANIFEST
		);
		expect(
			composeMapObjectMatrices(baked.get(STATUE_MESH)!, 0.6, 0, 0, 0, new Float32Array(16), 0)
		).toBe(0);
	});

	it('keeps an instance inside its cull distance', () => {
		const baked = bakeMapObjectInstances(
			[
				{
					x: 0,
					y: 20000,
					z: 0,
					actorClass: STATUE_CLASS,
					scale: 1,
					portalColor: '#4fc3ff',
					ringRadiusCm: FAST_TRAVEL_RADIUS_CM,
					rot: [0, 0, 0]
				}
			],
			'MainMap',
			MANIFEST
		);
		expect(
			composeMapObjectMatrices(baked.get(STATUE_MESH)!, 0.6, 0, 0, 0, new Float32Array(16), 0)
		).toBe(1);
	});

	// The game's distances are metres from a first-person camera; on a map camera
	// hundreds of metres up they cut a circle out of the frame. Raising the limit
	// to what the camera can see is what stops an on-screen instance being culled.
	it('keeps an instance past its cull distance but inside the view', () => {
		const baked = bakeMapObjectInstances(
			[
				{
					x: 0,
					y: 40000,
					z: 0,
					actorClass: STATUE_CLASS,
					scale: 1,
					portalColor: '#4fc3ff',
					ringRadiusCm: FAST_TRAVEL_RADIUS_CM,
					rot: [0, 0, 0]
				}
			],
			'MainMap',
			MANIFEST
		);
		expect(
			composeMapObjectMatrices(baked.get(STATUE_MESH)!, 0.6, 0, 0, 50000, new Float32Array(16), 0)
		).toBe(1);
	});

	// The watchtower is the one baked class with no cullDistanceCm at all, so it
	// is culled against the default rather than against `undefined`.
	it('culls a class with no baked cull distance against the default', () => {
		const mesh = manifestParts(MANIFEST, WATCHTOWER_CLASS)[0].mesh;
		const inside = bakeMapObjectInstances(
			[
				{
					x: 0,
					y: MAP_OBJECT_DEFAULT_CULL_CM / 2,
					z: 0,
					actorClass: WATCHTOWER_CLASS,
					scale: 1,
					portalColor: '#4fc3ff',
					ringRadiusCm: FAST_TRAVEL_RADIUS_CM,
					rot: [0, 0, 0]
				}
			],
			'MainMap',
			MANIFEST
		);
		const outside = bakeMapObjectInstances(
			[
				{
					x: 0,
					y: MAP_OBJECT_DEFAULT_CULL_CM * 2,
					z: 0,
					actorClass: WATCHTOWER_CLASS,
					scale: 1,
					portalColor: '#4fc3ff',
					ringRadiusCm: FAST_TRAVEL_RADIUS_CM,
					rot: [0, 0, 0]
				}
			],
			'MainMap',
			MANIFEST
		);
		expect(
			composeMapObjectMatrices(inside.get(mesh)!, 0.6, 0, 0, 0, new Float32Array(16), 0)
		).toBe(1);
		expect(
			composeMapObjectMatrices(outside.get(mesh)!, 0.6, 0, 0, 0, new Float32Array(16), 0)
		).toBe(0);
	});
});

describe('bakeMapObjectPortalInstances + composeMapObjectPortalMatrices', () => {
	function targetAndColor(): [Float32Array, THREE.InstancedBufferAttribute] {
		return [new Float32Array(16), new THREE.InstancedBufferAttribute(new Float32Array(3), 3)];
	}

	it('reproduces mapObjectPortalMatrix for every instance it writes', () => {
		const cmToMerc = 0.6;
		const item: MapObjectItem = {
			x: 12345,
			y: -6789,
			z: 4200,
			actorClass: RELIC_CLASS,
			scale: 3,
			portalColor: '#4fc3ff',
			ringRadiusCm: RELIC_RADIUS_CM,
			rot: [0, 0, 0]
		};
		const baked = bakeMapObjectPortalInstances([item], 'MainMap', MANIFEST);
		const [target, color] = targetAndColor();

		const written = composeMapObjectPortalMatrices(baked, cmToMerc, item.x, item.y, 0, target, color, 0);

		expect(written).toBe(1);
		expectMatrixNear(
			target,
			0,
			mapObjectPortalMatrix(item.x, item.y, item.z, 'MainMap', cmToMerc, item.scale).elements
		);
	});

	it("carries the item's own colour into the instanced colour buffer", () => {
		const item: MapObjectItem = {
			x: 0,
			y: 0,
			z: 0,
			actorClass: RELIC_CLASS,
			scale: 1,
			portalColor: '#ffa726',
			ringRadiusCm: RELIC_RADIUS_CM,
			rot: [0, 0, 0]
		};
		const baked = bakeMapObjectPortalInstances([item], 'MainMap', MANIFEST);
		const [target, color] = targetAndColor();

		composeMapObjectPortalMatrices(baked, 1, 0, 0, 0, target, color, 0);

		const expected = new THREE.Color('#ffa726');
		expect(color.getX(0)).toBeCloseTo(expected.r, 5);
		expect(color.getY(0)).toBeCloseTo(expected.g, 5);
		expect(color.getZ(0)).toBeCloseTo(expected.b, 5);
	});

	it('does not mutate the shared boss portal colour while baking beams', () => {
		const before = CORE_COLOR.getHexString();
		bakeMapObjectPortalInstances([ITEM], 'MainMap', MANIFEST);
		expect(CORE_COLOR.getHexString()).toBe(before);
	});

	it("culls a beam past its item's own cull distance, same as its mesh", () => {
		const baked = bakeMapObjectPortalInstances(
			[
				{
					x: 0,
					y: 40000,
					z: 0,
					actorClass: STATUE_CLASS,
					scale: 1,
					portalColor: '#4fc3ff',
					ringRadiusCm: FAST_TRAVEL_RADIUS_CM,
					rot: [0, 0, 0]
				}
			],
			'MainMap',
			MANIFEST
		);
		const [target, color] = targetAndColor();
		expect(composeMapObjectPortalMatrices(baked, 0.6, 0, 0, 0, target, color, 0)).toBe(0);
	});

	it('keeps a beam past its cull distance but inside the view, same as its mesh', () => {
		const baked = bakeMapObjectPortalInstances(
			[
				{
					x: 0,
					y: 40000,
					z: 0,
					actorClass: STATUE_CLASS,
					scale: 1,
					portalColor: '#4fc3ff',
					ringRadiusCm: FAST_TRAVEL_RADIUS_CM,
					rot: [0, 0, 0]
				}
			],
			'MainMap',
			MANIFEST
		);
		const [target, color] = targetAndColor();
		expect(composeMapObjectPortalMatrices(baked, 0.6, 0, 0, 50000, target, color, 0)).toBe(1);
	});

	it('ignores an actor class the manifest does not carry', () => {
		expect(
			bakeMapObjectPortalInstances(
				[
					{
						x: 0,
						y: 0,
						z: 0,
						actorClass: 'BP_Nope_C',
						scale: 1,
						portalColor: '#4fc3ff',
						ringRadiusCm: FAST_TRAVEL_RADIUS_CM,
						rot: [0, 0, 0]
					}
				],
				'MainMap',
				MANIFEST
			).length
		).toBe(0);
	});
});

describe('viewRadiusCm', () => {
	const corners = (sw: [number, number], ne: [number, number]) =>
		[sw, ne].map(([lng, lat]) => pixelToWorld(...lngLatToPixel(lng, lat), 'MainMap'));

	it('reaches the furthest corner of the view from the camera', () => {
		const sw: [number, number] = [-0.4, -0.3];
		const ne: [number, number] = [0.2, 0.5];
		const [a, b] = corners(sw, ne);
		const camera = { x: a.worldX, y: a.worldY };

		expect(viewRadiusCm(sw, ne, 'MainMap', camera.x, camera.y)).toBeCloseTo(
			Math.hypot(b.worldX - a.worldX, b.worldY - a.worldY),
			3
		);
	});

	// Pitching puts the camera off the rectangle's centre, so the further side is
	// the one that counts.
	it('measures from the camera, not from the middle of the view', () => {
		const sw: [number, number] = [-0.4, -0.4];
		const ne: [number, number] = [0.4, 0.4];
		const [a, b] = corners(sw, ne);
		const outside = viewRadiusCm(sw, ne, 'MainMap', a.worldX - 100000, a.worldY);
		const inside = viewRadiusCm(sw, ne, 'MainMap', (a.worldX + b.worldX) / 2, a.worldY);
		expect(outside).toBeGreaterThan(inside);
	});
});

describe('cullDistanceCmFor', () => {
	it('uses the baked distance when the class carries one', () => {
		expect(cullDistanceCmFor(MANIFEST[STATUE_CLASS])).toBe(30000);
		expect(cullDistanceCmFor(MANIFEST[RELIC_CLASS])).toBe(100000);
	});

	it('falls back to the default for the class that carries none', () => {
		expect(cullDistanceCmFor(MANIFEST[WATCHTOWER_CLASS])).toBe(MAP_OBJECT_DEFAULT_CULL_CM);
		expect(cullDistanceCmFor(undefined)).toBe(MAP_OBJECT_DEFAULT_CULL_CM);
	});
});
