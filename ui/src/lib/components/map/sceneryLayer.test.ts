import { MercatorCoordinate } from 'maplibre-gl';
import * as THREE from 'three';
import { describe, expect, it } from 'vitest';
import { ueYawToThreeQuaternion } from './coords3d';
import { pixelToLngLat } from './mercator';
import type { SceneryBucketData, SceneryRun, SceneryStream } from './sceneryFormat';
import {
	bakeInstances,
	composeInstanceMatrices,
	meetsScreenSizeThreshold,
	mergeRunsByMesh,
	projectedPixelDiameter,
	SCENERY_MIN_PIXELS,
	SCENERY_SCALAR_EPSILON,
	sceneryInstanceMatrix,
	selectBuckets,
	stabilizeScalar
} from './sceneryLayer';
import { MESH_FLIP } from './structureLayer';
import { worldToPixel } from './utils';

const bucket = (
	minX: number,
	minY: number,
	maxX: number,
	maxY: number,
	runs: SceneryRun[] = []
): SceneryBucketData => ({
	minX,
	minY,
	maxX,
	maxY,
	runs
});

const run = (meshIndex: number): SceneryRun => ({
	meshIndex,
	count: 0,
	positions: new Float32Array(0),
	quats: new Float32Array(0),
	scales: new Float32Array(0)
});

type Instance = {
	pos: [number, number, number];
	quat: [number, number, number, number];
	scale: [number, number, number];
};

const instanceRun = (meshIndex: number, instances: Instance[]): SceneryRun => ({
	meshIndex,
	count: instances.length,
	positions: new Float32Array(instances.flatMap((i) => i.pos)),
	quats: new Float32Array(instances.flatMap((i) => i.quat)),
	scales: new Float32Array(instances.flatMap((i) => i.scale))
});

// Reads instance k back out of a run exactly as rebuild() does, so the expected
// value is built from the same float32-rounded inputs the baked path saw.
const matrixOf = (r: SceneryRun, k: number, cmToMerc: number): THREE.Matrix4 =>
	sceneryInstanceMatrix(
		r.positions[k * 3],
		r.positions[k * 3 + 1],
		r.positions[k * 3 + 2],
		[r.quats[k * 4], r.quats[k * 4 + 1], r.quats[k * 4 + 2], r.quats[k * 4 + 3]],
		[r.scales[k * 3], r.scales[k * 3 + 1], r.scales[k * 3 + 2]],
		'MainMap',
		cmToMerc
	);

describe('selectBuckets', () => {
	it('returns buckets overlapping the view', () => {
		const buckets = [bucket(0, 0, 10, 10), bucket(100, 100, 110, 110)];
		expect(selectBuckets(buckets, { minX: 5, minY: 5, maxX: 50, maxY: 50 })).toEqual([0]);
	});

	it('includes a bucket that merely touches the view edge', () => {
		expect(
			selectBuckets([bucket(0, 0, 10, 10)], { minX: 10, minY: 10, maxX: 20, maxY: 20 })
		).toEqual([0]);
	});

	it('excludes buckets entirely outside the view', () => {
		expect(
			selectBuckets([bucket(0, 0, 10, 10)], { minX: 11, minY: 11, maxX: 20, maxY: 20 })
		).toEqual([]);
	});

	it('returns every bucket when the view covers the world', () => {
		const buckets = [bucket(0, 0, 10, 10), bucket(20, 20, 30, 30), bucket(-5, -5, 0, 0)];
		expect(selectBuckets(buckets, { minX: -1000, minY: -1000, maxX: 1000, maxY: 1000 })).toEqual([
			0, 1, 2
		]);
	});

	it('handles an empty bucket list', () => {
		expect(selectBuckets([], { minX: 0, minY: 0, maxX: 1, maxY: 1 })).toEqual([]);
	});
});

// sceneryInstanceMatrix generalises the yaw-only rotation handling elsewhere to a
// full actor quaternion, since scenery keeps pitch and roll. Feeding it a pure UE
// Z-yaw quaternion pins that general path against the trusted yaw-only one.
describe('sceneryInstanceMatrix', () => {
	it('reduces to the trusted MESH_FLIP + ueYawToThreeQuaternion path for a pure UE Z-yaw quaternion', () => {
		const yaw = 0.9;
		const qz = Math.sin(yaw / 2);
		const qw = Math.cos(yaw / 2);
		const worldX = 12345;
		const worldY = -6789;
		const worldZ = 4200;
		const cmToMerc = 0.7;

		const matrix = sceneryInstanceMatrix(
			worldX,
			worldY,
			worldZ,
			[0, 0, qz, qw],
			[1, 1, 1],
			'MainMap',
			cmToMerc
		);

		const [px, py] = worldToPixel(worldX, worldY, 'MainMap');
		const [lng, lat] = pixelToLngLat(px, py);
		// Horizontal placement is a per-point projection; vertical placement must
		// not be. It has to use cmToMerc -- one per-frame factor from the camera
		// centre, matching how terrain scales every vertex in a frame -- rather than
		// fromLngLat's altitude argument, which divides by this point's latitude.
		const anchor = MercatorCoordinate.fromLngLat([lng, lat]);
		const rotation = MESH_FLIP.clone().multiply(
			new THREE.Matrix4().makeRotationFromQuaternion(ueYawToThreeQuaternion(yaw))
		);
		const scale = new THREE.Matrix4().makeScale(cmToMerc, cmToMerc, cmToMerc);
		const expected = new THREE.Matrix4()
			.makeTranslation(anchor.x, anchor.y, worldZ * cmToMerc)
			.multiply(rotation)
			.multiply(scale);

		const got = matrix.toArray();
		const want = expected.toArray();
		for (let i = 0; i < 16; i++) {
			expect(got[i]).toBeCloseTo(want[i], 10);
		}
	});

	it('leaves a unit quaternion (no rotation) mapping to identity rotation after MESH_FLIP', () => {
		const matrix = sceneryInstanceMatrix(0, 0, 0, [0, 0, 0, 1], [1, 1, 1], 'MainMap', 1);
		const rotation = new THREE.Matrix4().extractRotation(matrix);
		const want = MESH_FLIP.clone();
		const got = rotation.toArray();
		const wantArr = want.toArray();
		for (let i = 0; i < 16; i++) {
			expect(got[i]).toBeCloseTo(wantArr[i], 10);
		}
	});

	// Regression guard for the per-instance-latitude bug: passing worldZ through
	// fromLngLat's altitude argument divides by each point's own latitude, so two
	// instances sharing a worldZ landed at different mercator heights. Terrain
	// uses one factor per frame, and scenery must match. MainMap's world X extremes
	// span the full -85.05..85.05 degree latitude range, so the pair picked here is
	// as far apart in latitude as this map ever gets.
	it('maps two instances at the same worldZ but very different latitudes to the same mercator height', () => {
		const worldZ = 5000;
		const cmToMerc = 0.6;
		const near = sceneryInstanceMatrix(
			-1099400,
			0,
			worldZ,
			[0, 0, 0, 1],
			[1, 1, 1],
			'MainMap',
			cmToMerc
		);
		const far = sceneryInstanceMatrix(
			349400,
			0,
			worldZ,
			[0, 0, 0, 1],
			[1, 1, 1],
			'MainMap',
			cmToMerc
		);
		expect(near.elements[14]).toBeCloseTo(far.elements[14], 12);
		expect(near.elements[14]).toBeCloseTo(worldZ * cmToMerc, 12);
	});
});

describe('projectedPixelDiameter', () => {
	it('scales linearly with a plain uniform scale', () => {
		const s = 2;
		const r = 3;
		const W = 5;
		const matrix = new THREE.Matrix4().makeScale(s, s, s);
		expect(projectedPixelDiameter(matrix, r, W)).toBeCloseTo(2 * r * s * W, 9);
	});

	it('is driven by the largest axis under anisotropic scale', () => {
		const r = 1;
		const W = 1000;
		// A tall thin instance: the tiny x/y components must not shrink the result,
		// since the large z axis alone has to keep it visible.
		const matrix = new THREE.Matrix4().makeScale(0.001, 0.001, 5);
		expect(projectedPixelDiameter(matrix, r, W)).toBeCloseTo(2 * r * 5 * W, 9);
	});

	it('is unaffected by rotation', () => {
		const s = 2;
		const r = 3;
		const W = 5;
		const rotation = new THREE.Matrix4().makeRotationFromEuler(new THREE.Euler(0.4, 1.1, -0.7));
		const matrix = rotation.multiply(new THREE.Matrix4().makeScale(s, s, s));
		expect(projectedPixelDiameter(matrix, r, W)).toBeCloseTo(2 * r * s * W, 9);
	});

	it('is unaffected by translation', () => {
		const s = 2;
		const r = 3;
		const W = 5;
		const rotation = new THREE.Matrix4().makeRotationFromEuler(new THREE.Euler(0.4, 1.1, -0.7));
		const matrix = new THREE.Matrix4()
			.makeTranslation(123456, -987654, 42)
			.multiply(rotation)
			.multiply(new THREE.Matrix4().makeScale(s, s, s));
		expect(projectedPixelDiameter(matrix, r, W)).toBeCloseTo(2 * r * s * W, 9);
	});
});

describe('mergeRunsByMesh', () => {
	it('merges runs for the same mesh index across different selected buckets', () => {
		const r0 = run(3);
		const r1 = run(3);
		const r2 = run(3);
		const buckets = [
			bucket(0, 0, 10, 10, [r0]),
			bucket(20, 20, 30, 30, [r1]),
			bucket(40, 40, 50, 50, [r2])
		];
		const stream: SceneryStream = { meshes: ['rock'], buckets };

		const result = mergeRunsByMesh(stream, [0, 1, 2]);

		expect(result.size).toBe(1);
		expect(result.get(3)).toEqual([r0, r1, r2]);
		const totalRuns = buckets.reduce((n, b) => n + b.runs.length, 0);
		expect(result.size).toBeLessThan(totalRuns);
	});

	it('ignores runs in buckets that are not selected', () => {
		const included = run(1);
		const excluded = run(2);
		const buckets = [bucket(0, 0, 10, 10, [included]), bucket(20, 20, 30, 30, [excluded])];
		const stream: SceneryStream = { meshes: ['a', 'b'], buckets };

		const result = mergeRunsByMesh(stream, [0]);

		expect(result.size).toBe(1);
		expect(result.has(2)).toBe(false);
		expect(result.get(1)).toEqual([included]);
	});

	it('returns an empty map when no buckets are selected', () => {
		const stream: SceneryStream = {
			meshes: ['a'],
			buckets: [bucket(0, 0, 10, 10, [run(0)])]
		};

		expect(mergeRunsByMesh(stream, []).size).toBe(0);
	});
});

// The bake/compose split separates what depends only on the stream from what
// changes with the camera. It only pays off if the composed result is
// indistinguishable from computing the whole matrix from scratch every frame.
describe('bakeInstances + composeInstanceMatrices', () => {
	const instances: Instance[] = [
		{ pos: [12345, -6789, 4200], quat: [0.1, -0.2, 0.3, Math.sqrt(0.86)], scale: [1, 2, 3] },
		{ pos: [-50000, 20000, 0], quat: [0, 0, 0, 1], scale: [1, 1, 1] }
	];

	it('reproduces sceneryInstanceMatrix for every instance that survives the cull', () => {
		const cmToMerc = 0.6;
		const r = instanceRun(0, instances);
		const baked = bakeInstances([r], 'MainMap');
		const target = new Float32Array(32);

		// A large pixelsPerMercatorUnit keeps every instance above the threshold,
		// isolating this test to the transform.
		const count = composeInstanceMatrices(baked, 2, cmToMerc, 1000, target, 0);

		expect(count).toBe(2);
		for (let k = 0; k < 2; k++) {
			const want = matrixOf(r, k, cmToMerc).elements;
			for (let e = 0; e < 16; e++) {
				expect(target[k * 16 + e]).toBeCloseTo(want[e], 6);
			}
		}
	});

	it('concatenates the runs it is given, in order', () => {
		const cmToMerc = 0.6;
		const first = instanceRun(0, [instances[0]]);
		const second = instanceRun(0, [instances[1]]);
		const baked = bakeInstances([first, second], 'MainMap');
		const target = new Float32Array(32);

		expect(composeInstanceMatrices(baked, 2, cmToMerc, 1000, target, 0)).toBe(2);

		const wantFirst = matrixOf(first, 0, cmToMerc).elements;
		const wantSecond = matrixOf(second, 0, cmToMerc).elements;
		for (let e = 0; e < 16; e++) {
			expect(target[e]).toBeCloseTo(wantFirst[e], 6);
			expect(target[16 + e]).toBeCloseTo(wantSecond[e], 6);
		}
	});

	// The same instances must drop out, and survivors must pack down with no gap
	// where a culled one was.
	it('drops instances below the screen-size threshold and packs the survivors contiguously', () => {
		const cmToMerc = 1;
		const geometryRadius = 1;
		const pixelsPerMercatorUnit = 1;
		const small: Instance = { pos: [0, 0, 0], quat: [0, 0, 0, 1], scale: [1, 1, 1] };
		const large: Instance = { pos: [1000, 1000, 0], quat: [0, 0, 0, 1], scale: [6, 6, 6] };
		const r = instanceRun(0, [small, large]);

		const smallMatrix = matrixOf(r, 0, cmToMerc);
		const largeMatrix = matrixOf(r, 1, cmToMerc);
		expect(
			meetsScreenSizeThreshold(
				projectedPixelDiameter(smallMatrix, geometryRadius, pixelsPerMercatorUnit)
			)
		).toBe(false);
		expect(
			meetsScreenSizeThreshold(
				projectedPixelDiameter(largeMatrix, geometryRadius, pixelsPerMercatorUnit)
			)
		).toBe(true);

		const target = new Float32Array(32);
		const count = composeInstanceMatrices(
			bakeInstances([r], 'MainMap'),
			geometryRadius,
			cmToMerc,
			pixelsPerMercatorUnit,
			target,
			0
		);

		expect(count).toBe(1);
		const want = largeMatrix.elements;
		for (let e = 0; e < 16; e++) {
			expect(target[e]).toBeCloseTo(want[e], 6);
		}
	});

	// One InstancedMesh is filled from several buckets' baked chunks in sequence,
	// so a chunk must be able to append without disturbing what is already there.
	it('appends at the given instance offset without touching earlier instances', () => {
		const cmToMerc = 0.6;
		const r = instanceRun(0, [instances[1]]);
		const target = new Float32Array(32).fill(7);

		const count = composeInstanceMatrices(
			bakeInstances([r], 'MainMap'),
			2,
			cmToMerc,
			1000,
			target,
			1
		);

		expect(count).toBe(1);
		for (let e = 0; e < 16; e++) {
			expect(target[e]).toBe(7);
		}
		const want = matrixOf(r, 0, cmToMerc).elements;
		for (let e = 0; e < 16; e++) {
			expect(target[16 + e]).toBeCloseTo(want[e], 6);
		}
	});

	it('bakes an empty run list to nothing and composes no instances from it', () => {
		const baked = bakeInstances([], 'MainMap');
		expect(baked.length).toBe(0);
		expect(composeInstanceMatrices(baked, 2, 0.6, 1000, new Float32Array(16), 0)).toBe(0);
	});
});

// cmToMerc shifts by a fraction of a percent on most frames of a pan, so a new
// value is adopted only once it is far enough from the one in use to matter.
describe('stabilizeScalar', () => {
	it('keeps the value already in use when the new one is within epsilon of it', () => {
		expect(stabilizeScalar(1.0005, 1, 1e-3)).toBe(1);
	});

	it('adopts the new value once it differs by more than epsilon', () => {
		expect(stabilizeScalar(1.002, 1, 1e-3)).toBe(1.002);
	});

	it('measures epsilon relative to the current value, not absolutely', () => {
		expect(stabilizeScalar(1000.5, 1000, 1e-3)).toBe(1000);
		expect(stabilizeScalar(0.0015, 0.001, 1e-3)).toBe(0.0015);
	});

	it('adopts any value when there is no usable current value', () => {
		expect(stabilizeScalar(5, 0, 1e-3)).toBe(5);
		expect(stabilizeScalar(5, Number.NaN, 1e-3)).toBe(5);
	});

	// Drift is measured against the value in use, never against the last raw
	// reading, so a slow pan cannot creep arbitrarily far in sub-epsilon steps.
	it('eventually adopts a value reached by repeated sub-epsilon steps', () => {
		let current = 1;
		for (let i = 1; i <= 20; i++) current = stabilizeScalar(1 + i * 0.0002, current, 1e-3);
		expect(current).toBeGreaterThan(1);
	});

	it('defaults to a small positive epsilon', () => {
		expect(SCENERY_SCALAR_EPSILON).toBeGreaterThan(0);
		expect(SCENERY_SCALAR_EPSILON).toBeLessThan(0.01);
		expect(stabilizeScalar(1 + SCENERY_SCALAR_EPSILON / 2, 1)).toBe(1);
	});
});

// Binds to the real cull rule rather than an inline duplicate of it.
describe('meetsScreenSizeThreshold', () => {
	it('is a positive threshold, culling just below it and keeping just at/above it', () => {
		expect(SCENERY_MIN_PIXELS).toBeGreaterThan(0);
		expect(meetsScreenSizeThreshold(SCENERY_MIN_PIXELS - 0.001)).toBe(false);
		expect(meetsScreenSizeThreshold(SCENERY_MIN_PIXELS + 0.001)).toBe(true);
	});
});
