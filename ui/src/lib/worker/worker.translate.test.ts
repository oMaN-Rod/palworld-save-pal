import { describe, it, expect } from 'vitest';
import { readFileSync } from 'node:fs';
import { resolve } from 'node:path';
import { unzipSync, zipSync } from 'fflate';
import { savZipToGvasBundle, gvasBundleToSavZip } from './psp.worker';

const root = resolve(__dirname, '..', '..', '..', '..');

describe('worker translation', () => {
	it('converts a real save zip to a GVAS bundle and back', async () => {
		const level = new Uint8Array(readFileSync(resolve(root, 'tests/fixtures/saves/world1/Level.sav')));
		const playersDir = resolve(root, 'tests/fixtures/saves/world1/Players');
		const zip = zipSync({ 'Level.sav': level });
		const bundle = await savZipToGvasBundle(zip);
		expect(bundle.save_id.length).toBeGreaterThan(0);
		expect(atob(bundle.level).slice(0, 4)).toBe('GVAS');

		const rezip = await gvasBundleToSavZip({ world_name: 'W', level: bundle.level, level_meta: null, world_option: null, players: [] });
		const files = unzipSync(rezip.zip);
		expect(files['Level.sav']).toBeDefined();
		expect(String.fromCharCode(...files['Level.sav'].subarray(8, 11))).toBe('PlM');
		void playersDir;
	});
});
