import createOozModule, { type OozModule } from '../../../vendor/ooz/ooz.mjs';

const OODLE_MERMAID = 9;
const OODLE_LEVEL_NORMAL = 4;
const SAFE_SPACE_PADDING = 128;

let oozModulePromise: Promise<OozModule> | null = null;
let loaded: OozModule | null = null;
function ooz(): Promise<OozModule> {
	if (oozModulePromise === null) {
		oozModulePromise = createOozModule().then((m) => {
			loaded = m;
			return m;
		});
	}
	return oozModulePromise;
}

/** Brings the module up so the `*Sync` pair can be called. The wasm engine
 * reaches for its Oodle codec from inside a synchronous encode, so it can be
 * lent one only after this resolves. */
export async function initOoz(): Promise<void> {
	await ooz();
}

function ready(): OozModule {
	if (loaded === null) throw new Error('ooz module is not initialized; await initOoz() first');
	return loaded;
}

function decompress(m: OozModule, compressed: Uint8Array, uncompressedLength: number): Uint8Array {
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

function compress(m: OozModule, data: Uint8Array): Uint8Array {
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

export async function oozDecompress(
	compressed: Uint8Array,
	uncompressedLength: number
): Promise<Uint8Array> {
	return decompress(await ooz(), compressed, uncompressedLength);
}

export async function oozCompress(data: Uint8Array): Promise<Uint8Array> {
	return compress(await ooz(), data);
}

export function oozDecompressSync(compressed: Uint8Array, uncompressedLength: number): Uint8Array {
	return decompress(ready(), compressed, uncompressedLength);
}

export function oozCompressSync(data: Uint8Array): Uint8Array {
	return compress(ready(), data);
}
