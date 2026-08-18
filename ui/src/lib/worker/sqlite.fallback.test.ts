import { describe, expect, it, vi } from 'vitest';

vi.mock('@sqlite.org/sqlite-wasm', () => ({
	default: async () => {
		throw new Error('module load failed');
	}
}));

import { openSqlite } from './sqlite';

describe('sqlite bridge fallback', () => {
	it('degrades to a no-op bridge when sqlite init fails entirely', async () => {
		const db = await openSqlite();
		expect(db.persistent).toBe(false);
		expect(db.query('SELECT 1', [])).toEqual([]);
		expect(db.exec('CREATE TABLE t (x)', [])).toBe(0);
	});
});
