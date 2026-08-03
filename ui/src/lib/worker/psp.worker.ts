import { unzipSync, zipSync } from 'fflate';
import { oozCompress, oozDecompress } from './ooz';
import { parseSavHeader, buildSav, getMagic, checkSavFormat, SaveType } from './savframe';
import { openSqlite } from './sqlite';

function toB64(bytes: Uint8Array): string {
	let s = '';
	for (let i = 0; i < bytes.length; i += 0x8000) {
		s += String.fromCharCode(...bytes.subarray(i, i + 0x8000));
	}
	return btoa(s);
}
function fromB64(b64: string): Uint8Array {
	const bin = atob(b64);
	const out = new Uint8Array(bin.length);
	for (let i = 0; i < bin.length; i++) out[i] = bin.charCodeAt(i);
	return out;
}

async function savToGvas(sav: Uint8Array): Promise<Uint8Array> {
	if (checkSavFormat(sav) === null) return sav;
	const h = parseSavHeader(sav);
	if (h.saveType !== SaveType.PLM) throw new Error('Only PLM (Oodle) saves are supported');
	const payload = sav.subarray(h.dataOffset, h.dataOffset + h.compressedLength);
	return oozDecompress(payload, h.uncompressedLength);
}
async function gvasToSav(gvas: Uint8Array): Promise<Uint8Array> {
	const compressed = await oozCompress(gvas);
	return buildSav(compressed, gvas.length, compressed.length, getMagic(SaveType.PLM)!, SaveType.PLM);
}

interface GvasBundle {
	save_id: string;
	level: string;
	level_meta: string | null;
	world_option: string | null;
	players: { uid: string; sav: string; dps: string | null }[];
}

export async function savZipToGvasBundle(zipBytes: Uint8Array): Promise<GvasBundle> {
	const files = unzipSync(zipBytes);
	const names = Object.keys(files);
	const levelName = names.find((n) => n.endsWith('Level.sav'));
	if (!levelName) throw new Error("Zip does not contain 'Level.sav'");
	const metaName = names.find((n) => n.endsWith('LevelMeta.sav'));
	const woName = names.find((n) => n.endsWith('WorldOption.sav'));
	const players: GvasBundle['players'] = [];
	const byUid = new Map<string, { sav?: string; dps?: string }>();
	const order: string[] = [];
	for (const n of names) {
		if (!n.includes('Players') || !n.endsWith('.sav')) continue;
		const stem = n.split('/').pop()!.replace(/\.sav$/, '');
		const isDps = stem.endsWith('_dps');
		const uid = isDps ? stem.slice(0, -4) : stem;
		if (!byUid.has(uid)) {
			byUid.set(uid, {});
			order.push(uid);
		}
		const gvas = toB64(await savToGvas(files[n]));
		if (isDps) byUid.get(uid)!.dps = gvas;
		else byUid.get(uid)!.sav = gvas;
	}
	for (const uid of order) {
		const e = byUid.get(uid)!;
		if (e.sav) players.push({ uid, sav: e.sav, dps: e.dps ?? null });
	}
	const saveId = levelName.includes('/') ? levelName.split('/')[0] : 'save';
	return {
		save_id: saveId,
		level: toB64(await savToGvas(files[levelName])),
		level_meta: metaName ? toB64(await savToGvas(files[metaName])) : null,
		world_option: woName ? toB64(await savToGvas(files[woName])) : null,
		players
	};
}

interface SaveBundleOut {
	world_name: string;
	level: string;
	level_meta: string | null;
	world_option: string | null;
	players: { uid: string; sav: string; dps: string | null }[];
}

export async function gvasBundleToSavZip(
	b: SaveBundleOut
): Promise<{ name: string; zip: Uint8Array }> {
	const entries: Record<string, Uint8Array> = {};
	entries['Level.sav'] = await gvasToSav(fromB64(b.level));
	if (b.world_option) entries['WorldOption.sav'] = await gvasToSav(fromB64(b.world_option));
	for (const p of b.players) {
		const stem = p.uid.replace(/-/g, '');
		entries[`Players/${stem}.sav`] = await gvasToSav(fromB64(p.sav));
		if (p.dps) entries[`Players/${stem}_dps.sav`] = await gvasToSav(fromB64(p.dps));
	}
	const name = `${b.world_name || 'PSP'}.zip`;
	return { name, zip: zipSync(entries) };
}

// Guarded so importing this module for its pure helpers doesn't start the
// runtime. Gate on a real worker global rather than `self`, since test
// environments (e.g. vitest) also define `self`.
declare const WorkerGlobalScope: unknown;
if (typeof WorkerGlobalScope !== 'undefined' && self instanceof (WorkerGlobalScope as never)) {
	let wasmReady: Promise<typeof import('$lib/wasm/psp')> | null = null;
	async function wasm() {
		if (!wasmReady) {
			wasmReady = (async () => {
				const mod = await import('$lib/wasm/psp');
				await mod.default();
				mod.init();
				mod.set_emit_callback((frame: string) => self.postMessage(frame));
				const sqlite = await openSqlite();
				mod.set_sql_bridge(
					(sql: string, params: unknown[]) => sqlite.exec(sql, params),
					(sql: string, params: unknown[]) => sqlite.query(sql, params)
				);
				await mod.run_migrations();
				if (!sqlite.persistent) {
					// Untranslated: paraglide resolves the locale from a cookie or a
					// module-scoped variable set by setLocale, and a dedicated worker
					// has neither `document` nor the main thread's module instance, so
					// importing $i18n/messages here would always render baseLocale.
					self.postMessage(
						JSON.stringify({
							type: 'error',
							data: {
								message:
									'This browser cannot store data; presets, blueprints and stored pals will not be saved between visits.',
								trace: ''
							}
						})
					);
				}
				const manifest: string[] = await (await fetch('/data/json/manifest.json')).json();
				const entries: [string, string][] = await Promise.all(
					// Key is the fetch path minus the .json extension — GameData keys
					// are extension-less (e.g. "items", "l10n/en/pals").
					manifest.map(
						async (f) =>
							[f.replace(/\.json$/, ''), await (await fetch(`/data/json/${f}`)).text()] as [
								string,
								string
							]
					)
				);
				mod.init_game_data(entries);
				return mod;
			})();
		}
		return wasmReady;
	}

	function postError(err: unknown) {
		const e = err instanceof Error ? err : new Error(String(err));
		self.postMessage(
			JSON.stringify({ type: 'error', data: { message: e.message, trace: e.stack ?? '' } })
		);
	}

	self.onmessage = async (ev: MessageEvent<string>) => {
		try {
			const frame = JSON.parse(ev.data) as { type: string; data: unknown };
			const mod = await wasm();
			if (frame.type === 'load_zip_file') {
				const zip = Uint8Array.from(frame.data as number[]);
				const bundle = await savZipToGvasBundle(zip);
				await mod.dispatch_frame(JSON.stringify({ type: 'load_save_gvas', data: bundle }));
				return;
			}
			if (frame.type === 'download_save_file') {
				await mod.dispatch_frame(JSON.stringify({ type: 'download_save_gvas', data: null }));
				return;
			}
			await mod.dispatch_frame(ev.data);
		} catch (err) {
			postError(err);
		}
	};

	// Turns the engine's save_gvas_bundle emission into the download_save_file
	// frame the UI expects, by wrapping postMessage.
	const origPost = self.postMessage.bind(self);
	(self as unknown as { postMessage: (m: string) => void }).postMessage = (message: string) => {
		try {
			const parsed = JSON.parse(message) as { type: string; data: SaveBundleOut };
			if (parsed.type === 'save_gvas_bundle') {
				void gvasBundleToSavZip(parsed.data)
					.then(({ name, zip }) => {
						origPost(
							JSON.stringify({ type: 'download_save_file', data: [{ name, content: toB64(zip) }] })
						);
					})
					.catch(postError);
				return;
			}
		} catch {
			/* not JSON we intercept */
		}
		origPost(message);
	};
}
