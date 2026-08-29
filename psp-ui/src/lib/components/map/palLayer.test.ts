import { describe, it, expect, beforeEach, vi } from 'vitest';
import * as THREE from 'three';
import {
	createPalLayer,
	palInstanceMatrix,
	predatorPalBosses,
	type PalBoss,
	type PalDisplay,
	type PalLayer,
	type PalPredator
} from './palLayer';
import { PORTAL_DIM_DEFEATED, PORTAL_RADIUS_CM } from './palPortal';
import { palRingColor } from './mapObjectPortal';
import { PAL_SCALE_DEFAULT } from './palSize';

const DISPLAY: PalDisplay = { scale: 30, heightCm: 0, autoFollow: true, xray: false };

const { models, listeners } = vi.hoisted(() => ({
	models: new Map<string, unknown>(),
	listeners: new Set<() => void>()
}));

vi.mock('./palMeshLibrary', () => ({
	requestPalMesh: (key: string) => models.get(key) ?? null,
	onPalMeshLoaded: (cb: () => void) => {
		listeners.add(cb);
		return () => listeners.delete(cb);
	}
}));

const CM_TO_MERC = 2.5e-9;

function zOf(m: THREE.Matrix4): number {
	return new THREE.Vector3().setFromMatrixPosition(m).z;
}

describe('palInstanceMatrix', () => {
	it('derives altitude from cmToMerc alone', () => {
		const m = palInstanceMatrix(0, 0, 5000, 0, 'MainMap', CM_TO_MERC, 30, 0);
		expect(zOf(m)).toBeCloseTo(5000 * CM_TO_MERC, 15);
	});

	// Discriminator for the double-latitude-correction defect: cmToMerc already
	// carries the camera centre's latitude, so no per-instance term may remain.
	// worldToPixel swaps axes, so latitude follows world X. The pair below is
	// deliberately not symmetric about MainMap's x-midpoint (-375000), since
	// cosine is even and mirrored latitudes would hide the defect. Divergence
	// under the buggy formula: 4.14%.
	it('produces the same altitude regardless of latitude', () => {
		const north = palInstanceMatrix(-1_099_400, 0, 5000, 0, 'MainMap', CM_TO_MERC, 30, 0);
		const south = palInstanceMatrix(340_000, 0, 5000, 0, 'MainMap', CM_TO_MERC, 30, 0);
		const rel = Math.abs(zOf(north) - zOf(south)) / Math.abs(zOf(south));
		expect(rel).toBeLessThan(1e-9);
	});

	it('scales by the requested scale so Pals are legible against a 14 km map', () => {
		const m = palInstanceMatrix(0, 0, 0, 0, 'MainMap', CM_TO_MERC, 30, 0);
		const sx = new THREE.Vector3().setFromMatrixColumn(m, 0).length();
		expect(sx).toBeCloseTo(30 * CM_TO_MERC, 15);
	});

	it('rotates about the up axis so a bearing change only spins the Pal in plan', () => {
		const a = palInstanceMatrix(0, 0, 0, 0, 'MainMap', CM_TO_MERC, 30, 0);
		const b = palInstanceMatrix(0, 0, 0, Math.PI / 2, 'MainMap', CM_TO_MERC, 30, 0);
		const upA = new THREE.Vector3().setFromMatrixColumn(a, 1).normalize();
		const upB = new THREE.Vector3().setFromMatrixColumn(b, 1).normalize();
		expect(upA.dot(upB)).toBeCloseTo(1, 10);
		const xA = new THREE.Vector3().setFromMatrixColumn(a, 0).normalize();
		const xB = new THREE.Vector3().setFromMatrixColumn(b, 0).normalize();
		expect(Math.abs(xA.dot(xB))).toBeLessThan(1e-6);
	});

	it('places two bosses at different world positions at different map positions', () => {
		const a = palInstanceMatrix(0, 0, 0, 0, 'MainMap', CM_TO_MERC, 30, 0);
		const b = palInstanceMatrix(100_000, 100_000, 0, 0, 'MainMap', CM_TO_MERC, 30, 0);
		const pa = new THREE.Vector3().setFromMatrixPosition(a);
		const pb = new THREE.Vector3().setFromMatrixPosition(b);
		expect(pa.x).not.toBeCloseTo(pb.x, 8);
		expect(pa.y).not.toBeCloseTo(pb.y, 8);
	});

	it('faces north when auto-follow is off', () => {
		const following = palInstanceMatrix(0, 0, 0, Math.PI / 2, 'MainMap', 1, 30, 0);
		const north = palInstanceMatrix(0, 0, 0, 0, 'MainMap', 1, 30, 0);
		expect(following.elements).not.toEqual(north.elements);
	});

	it('scales uniformly with the requested scale', () => {
		const small = palInstanceMatrix(0, 0, 0, 0, 'MainMap', 1, 10, 0);
		const large = palInstanceMatrix(0, 0, 0, 0, 'MainMap', 1, 30, 0);
		const column = (m: THREE.Matrix4, i: number) =>
			new THREE.Vector3().setFromMatrixColumn(m, i).length();
		for (const i of [0, 1, 2]) {
			expect(column(large, i) / column(small, i)).toBeCloseTo(3, 6);
		}
	});

	it('lifts the anchor by the height offset', () => {
		const grounded = palInstanceMatrix(0, 0, 0, 0, 'MainMap', 1, 30, 0);
		const raised = palInstanceMatrix(0, 0, 0, 0, 'MainMap', 1, 30, 500);
		expect(raised.elements[14] - grounded.elements[14]).toBeCloseTo(500, 6);
	});
});

const MODEL_FORWARD = new THREE.Vector3(0, 0, 1);

describe('palInstanceMatrix facing', () => {
	it('points the model at the camera at every bearing', () => {
		for (const bearing of [0, Math.PI / 2, Math.PI, (3 * Math.PI) / 2]) {
			const m = palInstanceMatrix(-375000, -375000, 0, bearing, 'MainMap', 1e-6, 30, 0);
			const basis = new THREE.Matrix3().setFromMatrix4(m);
			const facing = MODEL_FORWARD.clone().applyMatrix3(basis).normalize();
			expect(facing.x).toBeCloseTo(-Math.sin(bearing), 10);
			expect(facing.y).toBeCloseTo(Math.cos(bearing), 10);
			expect(facing.z).toBeCloseTo(0, 10);
		}
	});
});

describe('predatorPalBosses', () => {
	it('resolves the model key from pal rather than character_id', () => {
		const rows = [
			{ x: 1, y: 2, z: 3, pal: 'SifuDog' },
			{ x: 4, y: 5, z: 6, pal: 'PurpleSpider' }
		];
		expect(predatorPalBosses(rows)).toEqual([
			{ key: 'SifuDog', x: 1, y: 2, z: 3 },
			{ key: 'PurpleSpider', x: 4, y: 5, z: 6 }
		]);
	});

	it('returns an empty list for no predators', () => {
		expect(predatorPalBosses([])).toEqual([]);
	});
});

describe('createPalLayer', () => {
	beforeEach(() => {
		models.clear();
		listeners.clear();
	});

	function stubModel(name: string): THREE.Object3D {
		const root = new THREE.Object3D();
		root.name = name;
		root.add(new THREE.Mesh(new THREE.BoxGeometry(1, 1, 1), new THREE.MeshStandardMaterial()));
		return root;
	}

	function attach(layer: PalLayer): void {
		const stubMap = {
			getCenter: () => ({ lng: 0, lat: 0 }),
			getBearing: () => 0,
			triggerRepaint: () => {}
		};
		layer.attachMapForTest(stubMap as unknown as Parameters<typeof layer.attachMapForTest>[0]);
	}

	function palGroups(layer: PalLayer): THREE.Object3D[] {
		return layer
			.groupsForTest()
			.filter((o) => (o as THREE.InstancedMesh).isInstancedMesh !== true);
	}

	// The library returns the same cached Object3D however many bosses share a
	// key, and an Object3D holds only one parent, so adding it twice reparents
	// rather than duplicates and one Pal silently vanishes. "grassgolem" really is
	// used by two live spawn points. Identity discriminates, not count: the
	// uncloned implementation still pushes two entries.
	it('gives two bosses sharing one model two distinct parented groups', () => {
		models.set('grassgolem', stubModel('grassgolem'));
		const layer = createPalLayer({ id: 'test-shared-key' });
		attach(layer);

		layer.update(
			[
				{ key: 'grassgolem', x: 0, y: 0, z: 0, defeated: false },
				{ key: 'grassgolem', x: 100_000, y: 100_000, z: 0, defeated: false }
			],
			'MainMap',
			1,
			DISPLAY
		);

		const groups = palGroups(layer);
		expect(groups.length).toBe(2);
		expect(groups[0]).not.toBe(groups[1]);
		expect(groups[0].parent).not.toBeNull();
		expect(groups[1].parent).not.toBeNull();

		layer.dispose();
	});

	it('leaves no groups after dispose, including from a rebuild queued beforehand', async () => {
		models.set('anubis', stubModel('anubis'));
		const layer = createPalLayer({ id: 'test-dispose' });
		attach(layer);

		layer.update([{ key: 'anubis', x: 0, y: 0, z: 0, defeated: false }], 'MainMap', 1, DISPLAY);
		expect(palGroups(layer).length).toBe(1);

		for (const cb of listeners) cb();
		layer.dispose();
		await new Promise((r) => setTimeout(r, 0));

		expect(layer.groupsForTest().length).toBe(0);
	});

	it('builds a group only for bosses whose model resolves', () => {
		models.set('anubis', stubModel('anubis'));
		const layer = createPalLayer({ id: 'test-unresolvable' });
		attach(layer);

		layer.update(
			[
				{ key: 'lilyqueen_dark', x: 0, y: 0, z: 0, defeated: false },
				{ key: 'anubis', x: 100_000, y: 100_000, z: 0, defeated: false }
			],
			'MainMap',
			1,
			DISPLAY
		);

		const groups = palGroups(layer);
		expect(groups.length).toBe(1);
		expect(groups[0].name).toBe('anubis');
		expect(groups[0].children.length).toBe(1);

		layer.dispose();
	});

	it('adds one portal instance per boss regardless of model availability', () => {
		const layer = createPalLayer({ id: 'test-portal-count' });
		attach(layer);

		layer.update(
			[
				{ key: 'anubis', x: -400000, y: -300000, z: 0, defeated: false },
				{ key: 'definitely-not-a-real-key', x: -401000, y: -300000, z: 0, defeated: true }
			],
			'MainMap',
			1,
			DISPLAY
		);

		const instanced = layer
			.groupsForTest()
			.filter((o): o is THREE.InstancedMesh => (o as THREE.InstancedMesh).isInstancedMesh === true);
		expect(instanced.length).toBe(1);
		for (const m of instanced) expect(m.count).toBe(2);

		layer.dispose();
	});

	it('dims the portal of a defeated boss only', () => {
		const layer = createPalLayer({ id: 'test-portal-dim' });
		attach(layer);

		layer.update(
			[
				{ key: 'a', x: -400000, y: -300000, z: 0, defeated: false },
				{ key: 'b', x: -401000, y: -300000, z: 0, defeated: true }
			],
			'MainMap',
			1,
			DISPLAY
		);

		const column = layer
			.groupsForTest()
			.find((o): o is THREE.InstancedMesh => (o as THREE.InstancedMesh).isInstancedMesh === true)!;
		const attr = column.geometry.getAttribute('aIntensity');
		expect(attr.getX(0)).toBeCloseTo(1, 6);
		expect(attr.getX(1)).toBeCloseTo(PORTAL_DIM_DEFEATED, 6);

		layer.dispose();
	});

	it('disposes portals when the layer is disposed', () => {
		const layer = createPalLayer({ id: 'test-portal-dispose' });
		attach(layer);

		layer.update([{ key: 'a', x: -400000, y: -300000, z: 0, defeated: false }], 'MainMap', 1, DISPLAY);
		expect(layer.groupsForTest().length).toBeGreaterThan(0);

		layer.dispose();
		expect(layer.groupsForTest().length).toBe(0);
	});
});

describe('boss portal instancing', () => {
	function attach(layer: PalLayer): void {
		const stubMap = {
			getCenter: () => ({ lng: 0, lat: 0 }),
			getBearing: () => 0,
			triggerRepaint: () => {}
		};
		layer.attachMapForTest(stubMap as unknown as Parameters<typeof layer.attachMapForTest>[0]);
	}

	function portalCount(layer: PalLayer): number {
		const instanced = layer
			.groupsForTest()
			.find((o): o is THREE.InstancedMesh => (o as THREE.InstancedMesh).isInstancedMesh === true);
		return instanced?.count ?? 0;
	}

	const BOSS_A: PalBoss = { key: 'anubis', x: -400000, y: -300000, z: 0, defeated: false };
	const BOSS_B: PalBoss = { key: 'grassgolem', x: -401000, y: -300000, z: 0, defeated: false };

	it('instances only the bosses it is given', () => {
		const layer = createPalLayer({ id: 'test-instance-count' });
		attach(layer);

		layer.update([BOSS_A], 'MainMap', 1, DISPLAY);
		expect(portalCount(layer)).toBe(1);

		layer.dispose();
	});

	it('drops portals when the boss list shrinks', () => {
		const layer = createPalLayer({ id: 'test-portal-shrink' });
		attach(layer);

		layer.update([BOSS_A, BOSS_B], 'MainMap', 1, DISPLAY);
		expect(portalCount(layer)).toBe(2);

		layer.update([BOSS_A], 'MainMap', 1, DISPLAY);
		expect(portalCount(layer)).toBe(1);

		layer.dispose();
	});
});

describe('predator 3D rendering', () => {
	beforeEach(() => {
		models.clear();
		listeners.clear();
	});

	function attach(layer: PalLayer): void {
		const stubMap = {
			getCenter: () => ({ lng: 0, lat: 0 }),
			getBearing: () => 0,
			triggerRepaint: () => {}
		};
		layer.attachMapForTest(stubMap as unknown as Parameters<typeof layer.attachMapForTest>[0]);
	}

	function stubModel(name: string): THREE.Object3D {
		const root = new THREE.Object3D();
		root.name = name;
		root.add(new THREE.Mesh(new THREE.BoxGeometry(1, 1, 1), new THREE.MeshStandardMaterial()));
		return root;
	}

	function instancedMeshes(layer: PalLayer): THREE.InstancedMesh[] {
		return layer
			.groupsForTest()
			.filter((o): o is THREE.InstancedMesh => (o as THREE.InstancedMesh).isInstancedMesh === true);
	}

	function palGroups(layer: PalLayer): THREE.Object3D[] {
		return layer
			.groupsForTest()
			.filter((o) => (o as THREE.InstancedMesh).isInstancedMesh !== true);
	}

	const PREDATOR: PalPredator = { key: 'sifudog', x: -400000, y: -300000, z: 0 };

	it('builds a model group for a predator whose key resolves', () => {
		models.set('sifudog', stubModel('sifudog'));
		const layer = createPalLayer({ id: 'test-predator-model' });
		attach(layer);

		layer.update([], 'MainMap', 1, DISPLAY, [PREDATOR]);

		const groups = palGroups(layer);
		expect(groups.length).toBe(1);
		expect(groups[0].name).toBe('sifudog');

		layer.dispose();
	});

	it('gives a predator its own portal instance, separate from the boss/alpha one', () => {
		const layer = createPalLayer({ id: 'test-predator-portal-count' });
		attach(layer);

		layer.update(
			[{ key: 'anubis', x: -400000, y: -300000, z: 0, defeated: false }],
			'MainMap',
			1,
			DISPLAY,
			[PREDATOR]
		);

		const meshes = instancedMeshes(layer);
		expect(meshes.length).toBe(2);
		expect(meshes.map((m) => m.count).sort()).toEqual([1, 1]);

		layer.dispose();
	});

	it('colors the predator portal red via the aColor attribute, not uCore', () => {
		const layer = createPalLayer({ id: 'test-predator-portal-color' });
		attach(layer);

		layer.update([], 'MainMap', 1, DISPLAY, [PREDATOR]);

		const mesh = instancedMeshes(layer)[0];
		const aColor = mesh.geometry.getAttribute('aColor');
		const expected = palRingColor('predator');
		expect(aColor.getX(0)).toBeCloseTo(expected.r, 6);
		expect(aColor.getY(0)).toBeCloseTo(expected.g, 6);
		expect(aColor.getZ(0)).toBeCloseTo(expected.b, 6);

		layer.dispose();
	});

	it("builds the predator beam from palPortal.ts's PORTAL_RADIUS_CM", () => {
		const layer = createPalLayer({ id: 'test-predator-portal-radius' });
		attach(layer);

		layer.update([], 'MainMap', 1, DISPLAY, [PREDATOR]);

		const mesh = instancedMeshes(layer)[0];
		mesh.geometry.computeBoundingBox();
		const box = mesh.geometry.boundingBox!;
		expect(Math.max(box.max.x, box.max.y)).toBeCloseTo(PORTAL_RADIUS_CM, 4);

		layer.dispose();
	});

	it('drops the predator portal and model when the predator list empties', () => {
		models.set('sifudog', stubModel('sifudog'));
		const layer = createPalLayer({ id: 'test-predator-shrink' });
		attach(layer);

		layer.update([], 'MainMap', 1, DISPLAY, [PREDATOR]);
		expect(instancedMeshes(layer).length).toBe(1);
		expect(palGroups(layer).length).toBe(1);

		layer.update([], 'MainMap', 1, DISPLAY, []);
		expect(instancedMeshes(layer).length).toBe(0);
		expect(palGroups(layer).length).toBe(0);

		layer.dispose();
	});

	it('disposes predator groups and portals when the layer is disposed', () => {
		models.set('sifudog', stubModel('sifudog'));
		const layer = createPalLayer({ id: 'test-predator-dispose' });
		attach(layer);

		layer.update([], 'MainMap', 1, DISPLAY, [PREDATOR]);
		expect(layer.groupsForTest().length).toBeGreaterThan(0);

		layer.dispose();
		expect(layer.groupsForTest().length).toBe(0);
	});
});

describe('pal x-ray depth', () => {
	const BOSSES = [{ key: 'anubis', x: -400000, y: -300000, z: 0, defeated: false }];

	function buildLayerWithBosses({ xray }: { xray: boolean }): PalLayer {
		models.clear();
		listeners.clear();
		const root = new THREE.Object3D();
		root.name = 'anubis';
		root.add(new THREE.Mesh(new THREE.BoxGeometry(1, 1, 1), new THREE.MeshStandardMaterial()));
		models.set('anubis', root);

		const layer = createPalLayer({ id: 'test-xray' });
		const stubMap = {
			getCenter: () => ({ lng: 0, lat: 0 }),
			getBearing: () => 0,
			triggerRepaint: () => {}
		};
		layer.attachMapForTest(stubMap as unknown as Parameters<typeof layer.attachMapForTest>[0]);

		layer.update(BOSSES, 'MainMap', 1, {
			scale: PAL_SCALE_DEFAULT,
			heightCm: 0,
			autoFollow: false,
			xray
		});

		return layer;
	}

	function depthTestFlags(layer: PalLayer): boolean[] {
		const flags: boolean[] = [];
		for (const group of layer.groupsForTest()) {
			group.traverse((obj) => {
				const mesh = obj as THREE.Mesh;
				if (!mesh.material) return;
				for (const mat of Array.isArray(mesh.material) ? mesh.material : [mesh.material]) {
					flags.push(mat.depthTest);
				}
			});
		}
		return flags;
	}

	// Disabling depth testing to draw Pals through terrain would also remove their
	// occlusion against each other: far limbs paint over near ones. Drawing
	// through the world clears the depth buffer instead, leaving these flags alone.
	it('keeps depth testing on every Pal material in both x-ray states', () => {
		for (const xray of [false, true]) {
			const flags = depthTestFlags(buildLayerWithBosses({ xray }));
			expect(flags.length).toBeGreaterThan(0);
			expect(flags.every((f) => f === true)).toBe(true);
		}
	});

	it('leaves depth testing on after toggling x-ray off again', () => {
		const layer = buildLayerWithBosses({ xray: true });
		layer.update(BOSSES, 'MainMap', 1, {
			scale: PAL_SCALE_DEFAULT,
			heightCm: 0,
			autoFollow: false,
			xray: false
		});
		expect(depthTestFlags(layer).every((f) => f === true)).toBe(true);
	});
});

describe('createPalLayer update cost', () => {
	beforeEach(() => {
		models.clear();
		listeners.clear();
	});

	function stubModel(name: string): THREE.Object3D {
		const root = new THREE.Object3D();
		root.name = name;
		root.add(new THREE.Mesh(new THREE.BoxGeometry(1, 1, 1), new THREE.MeshStandardMaterial()));
		return root;
	}

	function attachMovable(layer: PalLayer): { bearing: number } {
		const camera = { bearing: 0 };
		const stubMap = {
			getCenter: () => ({ lng: 0, lat: 0 }),
			getBearing: () => camera.bearing,
			triggerRepaint: () => {}
		};
		layer.attachMapForTest(stubMap as unknown as Parameters<typeof layer.attachMapForTest>[0]);
		return camera;
	}

	function palGroups(layer: PalLayer): THREE.Object3D[] {
		return layer
			.groupsForTest()
			.filter((o) => (o as THREE.InstancedMesh).isInstancedMesh !== true);
	}

	const TWO = [
		{ key: 'anubis', x: 0, y: 0, z: 0, defeated: false },
		{ key: 'grassgolem', x: 100_000, y: 0, z: 0, defeated: false }
	];

	function seed(): void {
		models.set('anubis', stubModel('anubis'));
		models.set('grassgolem', stubModel('grassgolem'));
	}

	it('reuses the same group objects when the boss list is unchanged', () => {
		seed();
		const layer = createPalLayer({ id: 'test-reuse' });
		attachMovable(layer);

		layer.update(TWO, 'MainMap', 1, DISPLAY);
		const first = palGroups(layer);
		expect(first.length).toBe(2);

		layer.update(TWO, 'MainMap', 1, DISPLAY);
		const second = palGroups(layer);

		expect(second.length).toBe(2);
		expect(second[0]).toBe(first[0]);
		expect(second[1]).toBe(first[1]);
	});

	it('still refreshes group transforms when the camera bearing changes', () => {
		seed();
		const layer = createPalLayer({ id: 'test-refresh' });
		const camera = attachMovable(layer);

		layer.update(TWO, 'MainMap', 1, DISPLAY);
		const before = palGroups(layer)[0].matrix.clone();

		camera.bearing = 90;
		layer.update(TWO, 'MainMap', 1, DISPLAY);
		const after = palGroups(layer)[0].matrix;

		expect(after.elements).not.toEqual(before.elements);
	});

	it('rebuilds groups when the boss list changes', () => {
		seed();
		const layer = createPalLayer({ id: 'test-relist' });
		attachMovable(layer);

		layer.update(TWO, 'MainMap', 1, DISPLAY);
		expect(palGroups(layer).length).toBe(2);

		layer.update([TWO[0]], 'MainMap', 1, DISPLAY);
		expect(palGroups(layer).length).toBe(1);
	});
});
