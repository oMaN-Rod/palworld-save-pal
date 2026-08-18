import { existsSync, readFileSync } from 'node:fs';
import { describe, expect, it } from 'vitest';
import { decodeSceneryStream } from './sceneryFormat';
import { MAP_AREAS, type MapArea } from './utils';

// Builds the exact byte layout SceneryBucketer.Write emits.
function buildFixture(): ArrayBuffer {
	const names = ['SM_Cliff_Formation_6'];
	const nameBytes = names.map((n) => new TextEncoder().encode(n));
	const size =
		4 + 4 + 4 + nameBytes.reduce((a, b) => a + 2 + b.length, 0) + 4 + (16 + 4) + (2 + 4) + 2 * 32;

	const buf = new ArrayBuffer(size);
	const view = new DataView(buf);
	const bytes = new Uint8Array(buf);
	let o = 0;

	bytes.set([0x50, 0x53, 0x50, 0x53], o);
	o += 4; // "PSPS"
	view.setUint32(o, 1, true);
	o += 4; // version
	view.setUint32(o, names.length, true);
	o += 4;
	for (const nb of nameBytes) {
		view.setUint16(o, nb.length, true);
		o += 2;
		bytes.set(nb, o);
		o += nb.length;
	}
	view.setUint32(o, 1, true);
	o += 4; // bucketCount
	view.setFloat32(o, -100, true);
	o += 4;
	view.setFloat32(o, -200, true);
	o += 4;
	view.setFloat32(o, 300, true);
	o += 4;
	view.setFloat32(o, 400, true);
	o += 4;
	view.setUint32(o, 1, true);
	o += 4; // runCount
	view.setUint16(o, 0, true);
	o += 2; // meshIndex
	view.setUint32(o, 2, true);
	o += 4; // instanceCount

	for (const [x, y, z] of [
		[1, 2, 3],
		[4, 5, 6]
	]) {
		view.setFloat32(o, x, true);
		o += 4;
		view.setFloat32(o, y, true);
		o += 4;
		view.setFloat32(o, z, true);
		o += 4;
		view.setInt16(o, 0, true);
		o += 2; // qx
		view.setInt16(o, 0, true);
		o += 2; // qy
		view.setInt16(o, 0, true);
		o += 2; // qz
		view.setInt16(o, 32767, true);
		o += 2; // qw = 1
		view.setFloat32(o, 1, true);
		o += 4;
		view.setFloat32(o, 1, true);
		o += 4;
		view.setFloat32(o, 1, true);
		o += 4;
	}
	return buf;
}

describe('decodeSceneryStream', () => {
	it('reads the mesh table', () => {
		expect(decodeSceneryStream(buildFixture()).meshes).toEqual(['SM_Cliff_Formation_6']);
	});

	it('reads bucket bounds', () => {
		const b = decodeSceneryStream(buildFixture()).buckets[0];
		expect([b.minX, b.minY, b.maxX, b.maxY]).toEqual([-100, -200, 300, 400]);
	});

	it('reads every instance in a run', () => {
		const run = decodeSceneryStream(buildFixture()).buckets[0].runs[0];
		expect(run.meshIndex).toBe(0);
		expect(run.count).toBe(2);
		expect(Array.from(run.positions)).toEqual([1, 2, 3, 4, 5, 6]);
	});

	it('dequantises quaternions back to unit range', () => {
		const run = decodeSceneryStream(buildFixture()).buckets[0].runs[0];
		expect(run.quats[3]).toBeCloseTo(1, 4);
		expect(run.quats[0]).toBeCloseTo(0, 4);
	});

	it('rejects a buffer with the wrong magic', () => {
		const bad = new ArrayBuffer(16);
		expect(() => decodeSceneryStream(bad)).toThrow(/magic/i);
	});

	it('rejects an unknown version', () => {
		const buf = buildFixture();
		new DataView(buf).setUint32(4, 999, true);
		expect(() => decodeSceneryStream(buf)).toThrow(/version/i);
	});
});

// The parser's real per-area output, which this repo does not own -- skipped
// cleanly when absent so CI and other machines are unaffected. One file per area,
// not the single scenery_instances.bin that shipped before per-area clipping.
const REAL_STREAM: Record<MapArea, string> = {
	MainMap: 'O:/psp/palworld_parser/processed/scenery_instances_mainmap.bin',
	Tree: 'O:/psp/palworld_parser/processed/scenery_instances_tree.bin'
};

function readReal(area: MapArea) {
	const file = readFileSync(REAL_STREAM[area]);
	const buffer = file.buffer.slice(file.byteOffset, file.byteOffset + file.byteLength);
	return decodeSceneryStream(buffer);
}

describe('decodeSceneryStream (real artifacts)', () => {
	for (const area of ['MainMap', 'Tree'] as const) {
		it.skipIf(!existsSync(REAL_STREAM[area]))(
			`decodes the ${area} stream and every instance falls inside ${area}'s own extent`,
			() => {
				const stream = readReal(area);
				const { min, max } = MAP_AREAS[area];

				expect(stream.meshes.length).toBeGreaterThan(0);
				expect(stream.buckets.length).toBeGreaterThan(0);
				for (const mesh of stream.meshes) {
					expect(typeof mesh).toBe('string');
					expect(mesh.length).toBeGreaterThan(0);
				}

				// Guards World Tree content shipping inside MainMap's stream and vice
				// versa. Bucket bounds are derived from members, so checking the bounds
				// transitively covers every instance.
				let totalInstances = 0;
				for (const bucket of stream.buckets) {
					expect(bucket.minX).toBeGreaterThanOrEqual(min.x);
					expect(bucket.maxX).toBeLessThanOrEqual(max.x);
					expect(bucket.minY).toBeGreaterThanOrEqual(min.y);
					expect(bucket.maxY).toBeLessThanOrEqual(max.y);
					for (const run of bucket.runs) totalInstances += run.count;
				}
				expect(totalInstances).toBeGreaterThan(0);
			}
		);
	}
});
