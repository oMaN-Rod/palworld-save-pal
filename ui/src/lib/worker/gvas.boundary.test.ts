import { describe, it, expect, beforeAll } from 'vitest';
import { readFileSync, existsSync } from 'node:fs';
import { resolve } from 'node:path';
import { oozCompress, oozDecompress } from './ooz';
import { parseSavHeader, buildSav, getMagic, SaveType } from './savframe';

const root = resolve(__dirname, '..', '..', '..', '..');
const savPath = resolve(root, 'tests/fixtures/saves/world1/Level.sav');
const goldenPath = resolve(root, 'target/gvas-goldens/world1-level.gvas');
const hasGolden = existsSync(goldenPath);

// skipIf → the describe callback still runs at collection, so file reads live in
// beforeAll (which does NOT run for a skipped suite) to avoid throwing when the
// Rust golden-gen has not been run yet.
describe.skipIf(!hasGolden)('GVAS boundary: ooz.wasm ↔ psp_core', () => {
	let sav: Uint8Array;
	let golden: Uint8Array;
	beforeAll(() => {
		sav = new Uint8Array(readFileSync(savPath));
		golden = new Uint8Array(readFileSync(goldenPath)); // run `cargo test -p psp-core --test gvas_boundary` first
	});

	it('ooz-decompressed real Level.sav equals psp_core write_gvas_bytes output', async () => {
		const header = parseSavHeader(sav);
		expect(header.saveType).toBe(SaveType.PLM);
		const payload = sav.subarray(header.dataOffset, header.dataOffset + header.compressedLength);
		const gvas = await oozDecompress(payload, header.uncompressedLength);
		expect(String.fromCharCode(...gvas.subarray(0, 4))).toBe('GVAS');
		expect(gvas.length).toBe(golden.length);
		expect(Array.from(gvas)).toEqual(Array.from(golden));
	});

	it('compressing the golden GVAS re-frames to a .sav that decompresses back', async () => {
		const compressed = await oozCompress(golden);
		const sav2 = buildSav(compressed, golden.length, compressed.length, getMagic(SaveType.PLM)!, SaveType.PLM);
		const header = parseSavHeader(sav2);
		const payload = sav2.subarray(header.dataOffset, header.dataOffset + header.compressedLength);
		const restored = await oozDecompress(payload, header.uncompressedLength);
		expect(Array.from(restored)).toEqual(Array.from(golden));
	});
});
