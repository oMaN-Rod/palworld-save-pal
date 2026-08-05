import { describe, it, expect } from 'vitest';
import { openSqlite } from './sqlite';

describe('sqlite bridge', () => {
	it('exec creates/inserts and query reads back typed rows', async () => {
		const db = await openSqlite();
		db.exec('CREATE TABLE t (id INTEGER PRIMARY KEY, name TEXT, blob BLOB);', []);
		const affected = db.exec('INSERT INTO t (id, name, blob) VALUES (?, ?, ?)', [
			1,
			'hello',
			new Uint8Array([1, 2, 3])
		]);
		expect(affected).toBe(1);
		const rows = db.query('SELECT id, name, blob FROM t WHERE id = ?', [1]);
		expect(rows.length).toBe(1);
		expect(rows[0].id).toBe(1);
		expect(rows[0].name).toBe('hello');
		expect(Array.from(rows[0].blob as Uint8Array)).toEqual([1, 2, 3]);
	});

	it('runs a multi-statement script when no params are given', async () => {
		const db = await openSqlite();
		db.exec('CREATE TABLE a (x); CREATE TABLE b (y);', []);
		expect(db.query("SELECT name FROM sqlite_master WHERE type='table' ORDER BY name", []).map((r) => r.name)).toEqual(['a', 'b']);
	});
});
