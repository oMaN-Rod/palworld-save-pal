export const SaveType = { CNK: 0x30, PLM: 0x31, PLZ: 0x32 } as const;
export const MAGIC: Record<'PLZ' | 'PLM' | 'CNK', Uint8Array> = {
	PLZ: new Uint8Array([0x50, 0x6c, 0x5a]),
	PLM: new Uint8Array([0x50, 0x6c, 0x4d]),
	CNK: new Uint8Array([0x43, 0x4e, 0x4b])
};

function magicEquals(a: Uint8Array, b: Uint8Array): boolean {
	return a.length === b.length && a.every((byte, i) => byte === b[i]);
}

export interface SavHeader {
	uncompressedLength: number;
	compressedLength: number;
	magicBytes: Uint8Array;
	/** Container identity, from the magic. Dispatch compression on this. */
	format: number;
	/** Byte 11: the compression variant within the container. Shares values
	 * with `format` (0x31 means both PlM and single-zlib), so it can never
	 * stand in for it. */
	saveType: number;
	dataOffset: number;
}

function formatOf(magicBytes: Uint8Array): number | null {
	if (magicEquals(magicBytes, MAGIC.PLZ)) return SaveType.PLZ;
	if (magicEquals(magicBytes, MAGIC.PLM)) return SaveType.PLM;
	if (magicEquals(magicBytes, MAGIC.CNK)) return SaveType.CNK;
	return null;
}

export function parseSavHeader(savData: Uint8Array): SavHeader {
	if (savData.length < 24) throw new Error('File too small to parse header');
	const view = new DataView(savData.buffer, savData.byteOffset, savData.byteLength);
	let uncompressedLength = view.getUint32(0, true);
	let compressedLength = view.getUint32(4, true);
	let magicBytes = savData.subarray(8, 11);
	let saveType = savData[11];
	let dataOffset = 12;
	if (magicEquals(magicBytes, MAGIC.CNK)) {
		uncompressedLength = view.getUint32(12, true);
		compressedLength = view.getUint32(16, true);
		magicBytes = savData.subarray(20, 23);
		saveType = savData[23];
		dataOffset = 24;
	}
	const format = formatOf(magicBytes);
	if (format === null) throw new Error(`Unknown magic bytes: ${magicBytes}`);
	return { uncompressedLength, compressedLength, magicBytes, format, saveType, dataOffset };
}

export function getMagic(saveType: number): Uint8Array | null {
	if (saveType === SaveType.PLZ) return MAGIC.PLZ;
	if (saveType === SaveType.PLM) return MAGIC.PLM;
	if (saveType === SaveType.CNK) return MAGIC.CNK;
	return null;
}

export function checkSavFormat(savData: Uint8Array): number | null {
	if (savData.length < 12) return null;
	return formatOf(savData.subarray(8, 11));
}

export function buildSav(
	compressedData: Uint8Array,
	uncompressedLength: number,
	compressedLength: number,
	magicBytes: Uint8Array,
	saveType: number
): Uint8Array {
	const out = new Uint8Array(12 + compressedData.length);
	const view = new DataView(out.buffer);
	view.setUint32(0, uncompressedLength, true);
	view.setUint32(4, compressedLength, true);
	out.set(magicBytes, 8);
	out[11] = saveType;
	out.set(compressedData, 12);
	return out;
}
