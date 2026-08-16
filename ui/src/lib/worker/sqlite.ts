import sqlite3InitModule, {
	type BindingSpec,
	type Database,
	type OpfsSAHPoolDatabase,
	type SqlValue
} from '@sqlite.org/sqlite-wasm';

export interface SqliteBridge {
	exec(sql: string, params: unknown[]): number;
	query(sql: string, params: unknown[]): Record<string, unknown>[];
	persistent: boolean;
	opfsSupported: boolean;
}

// createSyncAccessHandle, not navigator.storage.getDirectory: Safari below 16.4
// exposes OPFS without sync handles, and the SAH pool needs the handles. This
// module only ever runs in a dedicated worker, where the interface is exposed.
function sahSupported(): boolean {
	const fh = (globalThis as { FileSystemFileHandle?: { prototype?: object } }).FileSystemFileHandle;
	return (
		typeof (fh?.prototype as { createSyncAccessHandle?: unknown })?.createSyncAccessHandle ===
		'function'
	);
}

// The SAH pool takes exclusive access handles on its files for the lifetime of
// the worker, so only one page per origin can hold it. A second tab therefore
// lands here with OPFS fully supported — telling that user their browser can't
// store data sends them chasing a browser problem that does not exist.
export function storageWarning(
	bridge: Pick<SqliteBridge, 'persistent' | 'opfsSupported'>
): string | null {
	if (bridge.persistent) return null;
	return bridge.opfsSupported
		? 'Another tab has Palworld Save Pal open and is using the database; presets, blueprints and stored pals will not be saved from this tab. Close the other tab and reload.'
		: 'This browser cannot store data; presets, blueprints and stored pals will not be saved between visits.';
}

const noopBridge: Omit<SqliteBridge, 'opfsSupported'> = {
	exec: () => 0,
	query: () => [],
	persistent: false
};

export interface OpenSqliteOptions {
	poolAttempts?: number;
	retryDelayMs?: number;
}

// The pool takes exclusive access handles, and a reload's worker starts before
// the departing page's worker has released them. Giving up on the first failure
// would downgrade the whole session to in-memory over a handover that resolves
// in well under a second.
const POOL_ATTEMPTS = 5;
const POOL_RETRY_MS = 250;

async function installPool(
	sqlite3: Awaited<ReturnType<typeof sqlite3InitModule>>,
	{ poolAttempts = POOL_ATTEMPTS, retryDelayMs = POOL_RETRY_MS }: OpenSqliteOptions
) {
	let last: unknown;
	for (let attempt = 1; attempt <= poolAttempts; attempt++) {
		try {
			return await sqlite3.installOpfsSAHPoolVfs({ name: 'psp-sahpool' });
		} catch (e) {
			last = e;
			if (attempt < poolAttempts) {
				console.warn(`[psp] OPFS SAH pool busy (attempt ${attempt}/${poolAttempts}); retrying`);
				await new Promise((resolve) => setTimeout(resolve, retryDelayMs));
			}
		}
	}
	throw last;
}

// Opens the persistent OPFS database; falls back to in-memory when the browser
// lacks OPFS SyncAccessHandle (e.g. Safari) or when running under Node (tests),
// so the app still works for the session without persisting. The whole init is
// wrapped because save load/edit/download must keep working even if sqlite
// can't be loaded at all (CSP, asset-path misconfig, network hiccup) — a
// rejection here would otherwise take down every feature, not just the DB one.
export async function openSqlite(options: OpenSqliteOptions = {}): Promise<SqliteBridge> {
	try {
		const sqlite3 = await sqlite3InitModule();
		let db: Database | OpfsSAHPoolDatabase;
		let persistent = false;
		try {
			const pool = await installPool(sqlite3, options);
			db = new pool.OpfsSAHPoolDb('/psp.db');
			persistent = true;
		} catch (e) {
			console.error('[psp] OPFS SAH pool unavailable; sqlite is in-memory for this session:', e);
			db = new sqlite3.oo1.DB(':memory:');
			persistent = false;
		}

		const exec = (sql: string, params: unknown[]): number => {
			if (!params || params.length === 0) {
				// No params → may be multiple statements (migrations): run them all.
				db.exec(sql);
			} else {
				db.exec(sql, { bind: params as unknown as BindingSpec });
			}
			return db.changes();
		};
		const query = (sql: string, params: unknown[]): Record<string, unknown>[] => {
			const rows: Record<string, SqlValue>[] = [];
			db.exec({
				sql,
				bind: params && params.length ? (params as unknown as BindingSpec) : undefined,
				rowMode: 'object',
				resultRows: rows
			});
			return rows;
		};
		return { exec, query, persistent, opfsSupported: sahSupported() };
	} catch (e) {
		console.error('[psp] sqlite failed to initialise; database features are disabled:', e);
		return { ...noopBridge, opfsSupported: sahSupported() };
	}
}
