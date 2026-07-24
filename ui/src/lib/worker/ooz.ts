import createOozModule, { type OozModule } from '../../../vendor/ooz/ooz.mjs';

const OODLE_MERMAID = 9;
const OODLE_LEVEL_NORMAL = 4;
const SAFE_SPACE_PADDING = 128;

let oozModulePromise: Promise<OozModule> | null = null;
function ooz(): Promise<OozModule> {
	if (oozModulePromise === null) oozModulePromise = createOozModule();
	return oozModulePromise;
}

export async function oozDecompress(
	compressed: Uint8Array,
	uncompressedLength: number
): Promise<Uint8Array> {
	const m = await ooz();
	const src = m._malloc(compressed.length);
	const dst = m._malloc(uncompressedLength + SAFE_SPACE_PADDING);
	try {
		m.HEAPU8.set(compressed, src);
		const written = m.ccall(
			'Ooz_Decompress',
			'number',
			Array(14).fill('number'),
			[src, compressed.length, dst, uncompressedLength, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]
		);
		if (written !== uncompressedLength) {
			throw new Error(`Ooz_Decompress wrote ${written}, expected ${uncompressedLength}`);
		}
		return new Uint8Array(m.HEAPU8.subarray(dst, dst + uncompressedLength));
	} finally {
		m._free(src);
		m._free(dst);
	}
}

export async function oozCompress(data: Uint8Array): Promise<Uint8Array> {
	const m = await ooz();
	const src = m._malloc(data.length);
	const dstCapacity = data.length + 65536;
	const dst = m._malloc(dstCapacity);
	try {
		m.HEAPU8.set(data, src);
		const written = m.ccall(
			'Ooz_Compress',
			'number',
			Array(6).fill('number'),
			[OODLE_MERMAID, src, data.length, dst, dstCapacity, OODLE_LEVEL_NORMAL]
		);
		if (written <= 0) throw new Error(`Ooz_Compress failed (code ${written})`);
		return new Uint8Array(m.HEAPU8.subarray(dst, dst + written));
	} finally {
		m._free(src);
		m._free(dst);
	}
}
