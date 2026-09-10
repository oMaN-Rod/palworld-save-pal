// Decoder for the scenery instance stream emitted by SceneryBucketer.Write.
// Any layout change must bump VERSION on both sides.
const MAGIC = 0x53505350; // "PSPS" little-endian
const VERSION = 1;
const BYTES_PER_INSTANCE = 32;

export type SceneryRun = {
	meshIndex: number;
	count: number;
	positions: Float32Array;
	quats: Float32Array;
	scales: Float32Array;
};

export type SceneryBucketData = {
	minX: number;
	minY: number;
	maxX: number;
	maxY: number;
	runs: SceneryRun[];
};

export type SceneryStream = { meshes: string[]; buckets: SceneryBucketData[] };

export function decodeSceneryStream(buffer: ArrayBuffer): SceneryStream {
	const view = new DataView(buffer);
	if (buffer.byteLength < 12 || view.getUint32(0, true) !== MAGIC) {
		throw new Error('scenery stream: bad magic, expected "PSPS"');
	}
	const version = view.getUint32(4, true);
	if (version !== VERSION) {
		throw new Error(`scenery stream: unsupported version ${version}, expected ${VERSION}`);
	}

	let o = 8;
	const meshCount = view.getUint32(o, true);
	o += 4;

	const decoder = new TextDecoder();
	const meshes: string[] = [];
	for (let i = 0; i < meshCount; i++) {
		const len = view.getUint16(o, true);
		o += 2;
		meshes.push(decoder.decode(new Uint8Array(buffer, o, len)));
		o += len;
	}

	const bucketCount = view.getUint32(o, true);
	o += 4;

	const buckets: SceneryBucketData[] = [];
	for (let b = 0; b < bucketCount; b++) {
		const minX = view.getFloat32(o, true);
		const minY = view.getFloat32(o + 4, true);
		const maxX = view.getFloat32(o + 8, true);
		const maxY = view.getFloat32(o + 12, true);
		o += 16;
		const runCount = view.getUint32(o, true);
		o += 4;

		const runs: SceneryRun[] = [];
		for (let r = 0; r < runCount; r++) {
			const meshIndex = view.getUint16(o, true);
			o += 2;
			const count = view.getUint32(o, true);
			o += 4;

			const positions = new Float32Array(count * 3);
			const quats = new Float32Array(count * 4);
			const scales = new Float32Array(count * 3);

			for (let i = 0; i < count; i++) {
				positions[i * 3] = view.getFloat32(o, true);
				positions[i * 3 + 1] = view.getFloat32(o + 4, true);
				positions[i * 3 + 2] = view.getFloat32(o + 8, true);
				quats[i * 4] = view.getInt16(o + 12, true) / 32767;
				quats[i * 4 + 1] = view.getInt16(o + 14, true) / 32767;
				quats[i * 4 + 2] = view.getInt16(o + 16, true) / 32767;
				quats[i * 4 + 3] = view.getInt16(o + 18, true) / 32767;
				scales[i * 3] = view.getFloat32(o + 20, true);
				scales[i * 3 + 1] = view.getFloat32(o + 24, true);
				scales[i * 3 + 2] = view.getFloat32(o + 28, true);
				o += BYTES_PER_INSTANCE;
			}

			runs.push({ meshIndex, count, positions, quats, scales });
		}

		buckets.push({ minX, minY, maxX, maxY, runs });
	}

	return { meshes, buckets };
}
