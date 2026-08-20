import { describe, expect, it } from 'vitest';
import { decodeSceneryStream } from './sceneryFormat';
import { decodeSceneryStreamAsync } from './sceneryStreamLoader';

// The test environment has no Worker global, which exercises the wrapper's
// inline fallback -- the exact path SSR, older webviews, and jsdom take.
function sampleBuffer(): ArrayBuffer {
	const view = new DataView(new ArrayBuffer(200));
	let o = 0;
	view.setUint32(o, 0x53505350, true); // "PSPS"
	o += 4;
	view.setUint32(o, 1, true); // VERSION
	o += 4;
	view.setUint32(o, 2, true); // mesh count
	o += 4;
	for (const name of ['pine', 'rock']) {
		view.setUint16(o, name.length, true);
		o += 2;
		for (let i = 0; i < name.length; i++) view.setUint8(o++, name.charCodeAt(i));
	}
	view.setUint32(o, 1, true); // bucket count
	o += 4;
	view.setFloat32(o, 0, true); // minX
	view.setFloat32(o + 4, 0, true); // minY
	view.setFloat32(o + 8, 100, true); // maxX
	view.setFloat32(o + 12, 100, true); // maxY
	o += 16;
	view.setUint32(o, 1, true); // run count
	o += 4;
	view.setUint16(o, 1, true); // meshIndex
	o += 2;
	view.setUint32(o, 1, true); // instance count
	o += 4;
	view.setFloat32(o, 10, true); // x
	view.setFloat32(o + 4, 20, true); // y
	view.setFloat32(o + 8, 30, true); // z
	view.setInt16(o + 12, 0, true); // quat x
	view.setInt16(o + 14, 32767, true); // quat y
	view.setInt16(o + 16, 0, true); // quat z
	view.setInt16(o + 18, 0, true); // quat w
	view.setFloat32(o + 20, 1, true); // sx
	view.setFloat32(o + 24, 2, true); // sy
	view.setFloat32(o + 28, 3, true); // sz
	o += 32;
	return view.buffer.slice(0, o);
}

describe('decodeSceneryStreamAsync', () => {
	it('falls back to the inline decode when Worker is unavailable', async () => {
		const buffer = sampleBuffer();
		const stream = await decodeSceneryStreamAsync(buffer);
		expect(stream.meshes).toEqual(['pine', 'rock']);
		expect(stream.buckets).toHaveLength(1);
		const run = stream.buckets[0].runs[0];
		expect(run.count).toBe(1);
		expect(run.positions).toEqual(new Float32Array([10, 20, 30]));
		expect(run.quats[1]).toBeCloseTo(1, 3);
		expect(run.scales).toEqual(new Float32Array([1, 2, 3]));
	});

	it('matches the sync decoder on the same buffer', async () => {
		const buffer = sampleBuffer();
		expect(await decodeSceneryStreamAsync(buffer)).toEqual(decodeSceneryStream(buffer));
	});
});
