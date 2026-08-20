import { unzipSync, unzlibSync, zipSync } from 'fflate';
import { initOoz, oozCompress, oozCompressSync, oozDecompress, oozDecompressSync } from './ooz';
import type { SavHeader } from './savframe';
import { buildSav, checkSavFormat, getMagic, parseSavHeader, SaveType } from './savframe';
import { openSqlite, storageWarning } from './sqlite';

const ZLIB_DOUBLE = 0x32;

function zlibToGvas(h: SavHeader, sav: Uint8Array): Uint8Array {
	const first = unzlibSync(sav.subarray(h.dataOffset));
	let gvas = first;
	if (h.saveType === ZLIB_DOUBLE) {
		// compressedLength describes the intermediate stream here, not the disk bytes.
		if (h.compressedLength !== first.length) {
			throw new Error(`incorrect compressed length: ${h.compressedLength} != ${first.length}`);
		}
		gvas = unzlibSync(first);
	}
	if (h.uncompressedLength !== gvas.length) {
		throw new Error(`incorrect uncompressed length: ${h.uncompressedLength} != ${gvas.length}`);
	}
	return gvas;
}

export async function savToGvas(sav: Uint8Array): Promise<Uint8Array> {
	if (checkSavFormat(sav) === null) return sav;
	const h = parseSavHeader(sav);
	if (h.format !== SaveType.PLM) return zlibToGvas(h, sav);
	const payload = sav.subarray(h.dataOffset, h.dataOffset + h.compressedLength);
	return oozDecompress(payload, h.uncompressedLength);
}
async function gvasToSav(gvas: Uint8Array): Promise<Uint8Array> {
	const compressed = await oozCompress(gvas);
	return buildSav(
		compressed,
		gvas.length,
		compressed.length,
		getMagic(SaveType.PLM)!,
		SaveType.PLM
	);
}

export type GvasSlot = 'level' | 'level_meta' | 'world_option' | 'player_sav' | 'player_dps';
export type StageFn = (slot: GvasSlot, uid: string, gvas: Uint8Array) => void;

/** Palworld names player files by 32-hex stem; the engine parses dashed uuids. */
function uidFromStem(stem: string): string {
	const hex = stem.toLowerCase();
	return [
		hex.slice(0, 8),
		hex.slice(8, 12),
		hex.slice(12, 16),
		hex.slice(16, 20),
		hex.slice(20, 32)
	].join('-');
}

/**
 * Decompresses a save zip one file at a time, handing each GVAS buffer to
 * `stage` and dropping it before starting the next. Returns the save id.
 *
 * One file at a time is the whole point: a real save's files total hundreds of
 * megabytes decompressed, and collecting them into a single structure — base64
 * in a JSON frame, above all — exceeds the longest string a browser can hold.
 */
export async function stageSavZip(zipBytes: Uint8Array, stage: StageFn): Promise<string> {
	const files = unzipSync(zipBytes);
	const names = Object.keys(files);
	const levelName = names.find((n) => n.endsWith('Level.sav'));
	if (!levelName) throw new Error("Zip does not contain 'Level.sav'");

	const hand = async (name: string, slot: GvasSlot, uid = '') => {
		const gvas = await savToGvas(files[name]);
		// Release the compressed bytes before the caller takes the GVAS, so the
		// two are never both retained.
		delete files[name];
		stage(slot, uid, gvas);
	};

	await hand(levelName, 'level');
	const metaName = names.find((n) => n.endsWith('LevelMeta.sav'));
	if (metaName) await hand(metaName, 'level_meta');
	const woName = names.find((n) => n.endsWith('WorldOption.sav'));
	if (woName) await hand(woName, 'world_option');

	for (const n of names) {
		if (!n.includes('Players') || !n.endsWith('.sav')) continue;
		const stem = n
			.split('/')
			.pop()!
			.replace(/\.sav$/, '');
		const isDps = stem.endsWith('_dps');
		const uid = uidFromStem(isDps ? stem.slice(0, -4) : stem);
		await hand(n, isDps ? 'player_dps' : 'player_sav', uid);
	}

	return levelName.includes('/') ? levelName.split('/')[0] : 'save';
}

/**
 * Compresses each entry the engine listed back into a `.sav` and zips them.
 * The engine owns the file list, so the naming rules live in one place.
 */
export async function gvasFilesToSavZip(
	worldName: string,
	names: string[],
	read: (name: string) => Uint8Array | Promise<Uint8Array>
): Promise<{ name: string; zip: Uint8Array }> {
	const entries: Record<string, Uint8Array> = {};
	// Sequential on purpose: each GVAS is compressed and released before the
	// next is pulled out of the engine, so only one is ever resident.
	for (const name of names) entries[name] = await gvasToSav(await read(name));
	return { name: `${worldName || 'PSP'}.zip`, zip: zipSync(entries) };
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
				// The engine links no Oodle codec on wasm32; without this every
				// `.sav`/`.psp` it writes for itself — blueprints, above all —
				// fails with "Compression support not enabled".
				await initOoz();
				mod.set_oodle_bridge(oozCompressSync, oozDecompressSync);
				const sqlite = await openSqlite();
				mod.set_sql_bridge(
					(sql: string, params: unknown[]) => sqlite.exec(sql, params),
					(sql: string, params: unknown[]) => sqlite.query(sql, params)
				);
				await mod.run_migrations();
				// Untranslated: paraglide resolves the locale from a cookie or a
				// module-scoped variable set by setLocale, and a dedicated worker
				// has neither `document` nor the main thread's module instance, so
				// importing $i18n/messages here would always render baseLocale.
				// 'warning', not 'error': errorHandler navigates to /error, which
				// would eject the user out of an app that still works fine without
				// persistence.
				const warning = storageWarning(sqlite);
				if (warning) {
					self.postMessage(
						JSON.stringify({ type: 'warning', data: { message: warning, trace: '' } })
					);
				}
				const manifest: string[] = await (await fetch('/data/json/manifest.json')).json();
				// Fetch only the active locale (plus the en fallback) of the
				// l10n/ and ui/ tables: the engine merges a single locale per
				// request, so shipping all 17 would add ~18 MB of download and
				// wasm parse time to every cold start. Language changes reload
				// the app (Sidebar's handleLanguageSelect), re-running this.
				const rows = sqlite.query('SELECT language FROM settings', []);
				const row = rows[0];
				const language = typeof row?.language === 'string' && row.language ? row.language : 'en';
				const wantedLocale = language.toLowerCase();
				const isWanted = (f: string): boolean => {
					if (f.startsWith('ui/')) {
						const stem = f.slice(3, -'.json'.length).toLowerCase();
						return stem === 'en' || stem === wantedLocale;
					}
					if (f.startsWith('l10n/')) {
						const locale = f.slice(5).split('/')[0].toLowerCase();
						return locale === 'en' || locale === wantedLocale;
					}
					return true;
				};
				const selected = manifest.filter(isWanted);
				const entries: [string, string][] = await Promise.all(
					// Key is the fetch path minus the .json extension — GameData keys
					// are extension-less (e.g. "items", "l10n/en/pals").
					selected.map(
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

	// Bulk bytes ride their own binary message rather than a JSON frame, in both
	// directions. Everything else stays a JSON string.
	type BinaryFrame = { type: string; bytes: Uint8Array };
	const postBinary = (message: object, transfer: Transferable[]) =>
		(self as unknown as { postMessage: (m: object, t: Transferable[]) => void }).postMessage(
			message,
			transfer
		);

	// Every engine entry point takes the module state out of its cell for the
	// duration of the call, so two overlapping calls would find it missing and
	// trap. `onmessage` is async, so without this the next message would start
	// at the first await of the one before it.
	let pending: Promise<unknown> = Promise.resolve();
	const serialized = (op: () => Promise<void>) => {
		const next = pending.then(op, op);
		pending = next.catch(() => {});
		return next;
	};

	const handle = async (ev: MessageEvent<string | BinaryFrame>) => {
		try {
			const mod = await wasm();
			if (typeof ev.data !== 'string') {
				if (ev.data.type !== 'load_zip_file') {
					throw new Error(`unsupported binary frame: ${ev.data.type}`);
				}
				const saveId = await stageSavZip(ev.data.bytes, (slot, uid, gvas) =>
					mod.stage_gvas(slot, uid, gvas)
				);
				await mod.load_staged_gvas(saveId);
				return;
			}
			const frame = JSON.parse(ev.data) as { type: string; data: unknown };
			if (frame.type === 'download_save_file') {
				const manifest = await mod.export_gvas_manifest();
				const { name, zip } = await gvasFilesToSavZip(manifest.world_name, manifest.names, (n) =>
					mod.export_gvas_file(n)
				);
				postBinary({ type: 'download_save_file', data: [{ name, bytes: zip }] }, [zip.buffer]);
				return;
			}
			await mod.dispatch_frame(ev.data);
		} catch (err) {
			postError(err);
		}
	};

	self.onmessage = (ev: MessageEvent<string | BinaryFrame>) => {
		void serialized(() => handle(ev));
	};
}
