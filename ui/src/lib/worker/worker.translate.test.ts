import { unzipSync, zipSync } from 'fflate';
import { readFileSync } from 'node:fs';
import { resolve } from 'node:path';
import { describe, expect, it } from 'vitest';
import { gvasFilesToSavZip, stageSavZip, type GvasSlot } from './psp.worker';

const root = resolve(__dirname, '..', '..', '..', '..');
const fixture = (name: string) =>
	new Uint8Array(readFileSync(resolve(root, 'tests/fixtures/saves/world1', name)));

type Staged = { slot: GvasSlot; uid: string; gvas: Uint8Array };

function collect() {
	const staged: Staged[] = [];
	const stage = (slot: GvasSlot, uid: string, gvas: Uint8Array) => staged.push({ slot, uid, gvas });
	return { staged, stage };
}

describe('worker staging', () => {
	it('hands over one decompressed GVAS buffer per save file', async () => {
		const zip = zipSync({
			'world1/Level.sav': fixture('Level.sav'),
			'world1/LevelMeta.sav': fixture('LevelMeta.sav'),
			'world1/Players/43797F87000000000000000000000000.sav': fixture(
				'Players/43797F87000000000000000000000000.sav'
			)
		});
		const { staged, stage } = collect();

		const saveId = await stageSavZip(zip, stage);

		expect(saveId).toBe('world1');
		expect(staged.map((s) => s.slot).sort()).toEqual(['level', 'level_meta', 'player_sav']);
		for (const { gvas } of staged) {
			// Bytes, never a string: base64ing a real save's files into one JSON
			// frame overruns the longest string the browser can hold.
			expect(gvas).toBeInstanceOf(Uint8Array);
			expect(String.fromCharCode(...gvas.subarray(0, 4))).toBe('GVAS');
		}
	});

	it('pairs a _dps file with its player under the same dashed uid', async () => {
		const player = fixture('Players/43797F87000000000000000000000000.sav');
		const zip = zipSync({
			'world1/Level.sav': fixture('Level.sav'),
			'world1/Players/43797F87000000000000000000000000.sav': player,
			'world1/Players/43797F87000000000000000000000000_dps.sav': player
		});
		const { staged, stage } = collect();

		await stageSavZip(zip, stage);

		const players = staged.filter((s) => s.slot !== 'level');
		expect(players).toHaveLength(2);
		expect(new Set(players.map((s) => s.uid))).toEqual(
			new Set(['43797f87-0000-0000-0000-000000000000'])
		);
		expect(players.map((s) => s.slot).sort()).toEqual(['player_dps', 'player_sav']);
	});

	it('writes every manifest entry into the download zip verbatim', async () => {
		const gvas = new Map<string, Uint8Array>();
		const { staged, stage } = collect();
		await stageSavZip(zipSync({ 'world1/Level.sav': fixture('Level.sav') }), stage);
		gvas.set('Level.sav', staged[0].gvas);
		gvas.set('LevelMeta.sav', staged[0].gvas);
		gvas.set('Players/43797f87000000000000000000000000.sav', staged[0].gvas);

		const names = [...gvas.keys()];
		const out = await gvasFilesToSavZip('MyWorld', names, (name) => gvas.get(name)!);

		expect(out.name).toBe('MyWorld.zip');
		const files = unzipSync(out.zip);
		expect(Object.keys(files).sort()).toEqual(names.sort());
		for (const name of names) {
			expect(String.fromCharCode(...files[name].subarray(8, 11))).toBe('PlM');
		}
	});
});
