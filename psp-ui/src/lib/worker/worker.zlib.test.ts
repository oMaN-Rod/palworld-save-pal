import { describe, it, expect } from 'vitest';
import { zlibSync } from 'fflate';
import { savToGvas } from './psp.worker';

function gvasPayload(): Uint8Array {
	const out = new Uint8Array(7006);
	out.set([0x47, 0x56, 0x41, 0x53]);
	for (let i = 4; i < out.length; i++) out[i] = (i * 31 + 7) & 0xff;
	return out;
}

function frame(magic: string, saveType: number, uncompressedLength: number, compressedLength: number, payload: Uint8Array): Uint8Array {
	const out = new Uint8Array(12 + payload.length);
	const view = new DataView(out.buffer);
	view.setUint32(0, uncompressedLength, true);
	view.setUint32(4, compressedLength, true);
	for (let i = 0; i < 3; i++) out[8 + i] = magic.charCodeAt(i);
	out[11] = saveType;
	out.set(payload, 12);
	return out;
}

describe('savToGvas zlib containers', () => {
	// The real-world case: Palworld still ships PlZ WorldOption.sav files whose
	// save-type byte is 0x31 — the same value as SaveType.PLM.
	it('decodes a single-zlib PlZ save (save type 0x31)', async () => {
		const gvas = gvasPayload();
		const deflated = zlibSync(gvas);
		const sav = frame('PlZ', 0x31, gvas.length, deflated.length, deflated);

		expect(Array.from(await savToGvas(sav))).toEqual(Array.from(gvas));
	});

	it('decodes a double-zlib PlZ save (save type 0x32)', async () => {
		const gvas = gvasPayload();
		const first = zlibSync(gvas);
		const second = zlibSync(first);
		const sav = frame('PlZ', 0x32, gvas.length, first.length, second);

		expect(Array.from(await savToGvas(sav))).toEqual(Array.from(gvas));
	});

	it('decodes a CNK save from its nested header', async () => {
		const gvas = gvasPayload();
		const deflated = zlibSync(gvas);
		const inner = frame('PlZ', 0x31, gvas.length, deflated.length, deflated);
		const sav = new Uint8Array(12 + inner.length);
		new DataView(sav.buffer).setUint32(0, 0, true);
		for (let i = 0; i < 3; i++) sav[8 + i] = 'CNK'.charCodeAt(i);
		sav[11] = 0x30;
		sav.set(inner, 12);

		expect(Array.from(await savToGvas(sav))).toEqual(Array.from(gvas));
	});

	it('rejects a PlZ save whose payload does not match its declared length', async () => {
		const gvas = gvasPayload();
		const deflated = zlibSync(gvas);
		const sav = frame('PlZ', 0x31, gvas.length + 1, deflated.length, deflated);

		await expect(savToGvas(sav)).rejects.toThrow(/uncompressed length/i);
	});
});
