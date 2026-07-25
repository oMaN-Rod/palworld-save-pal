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
}

// Opens the persistent OPFS database; falls back to in-memory when the browser
// lacks OPFS SyncAccessHandle (e.g. Safari) or when running under Node (tests),
// so the app still works for the session without persisting.
export async function openSqlite(): Promise<SqliteBridge> {
	const sqlite3 = await sqlite3InitModule();
	let db: Database | OpfsSAHPoolDatabase;
	let persistent = false;
	try {
		const pool = await sqlite3.installOpfsSAHPoolVfs({ name: 'psp-sahpool' });
		db = new pool.OpfsSAHPoolDb('/psp.db');
		persistent = true;
	} catch {
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
	return { exec, query, persistent };
}
