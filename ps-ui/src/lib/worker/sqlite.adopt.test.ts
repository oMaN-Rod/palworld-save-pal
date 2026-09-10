import { beforeEach, describe, expect, it, vi } from 'vitest';

class FakeDb {
	exec() {}
	changes() {
		return 0;
	}
}

class FakePool {
	files = new Map<string, Uint8Array>();
	removed = false;
	OpfsSAHPoolDb = FakeDb;
	getFileNames() {
		return [...this.files.keys()];
	}
	exportFile(name: string) {
		return this.files.get(name)!;
	}
	importDb(name: string, bytes: Uint8Array) {
		this.files.set(name, bytes);
		return bytes.length;
	}
	async removeVfs() {
		this.removed = true;
		return true;
	}
}

let pools: Map<string, FakePool>;
let legacyBusy: boolean;

vi.mock('@sqlite.org/sqlite-wasm', () => ({
	default: async () => ({
		installOpfsSAHPoolVfs: async ({ name }: { name: string }) => {
			if (name === 'psp-sahpool' && legacyBusy) throw new Error('Access Handles cannot be created');
			if (!pools.has(name)) pools.set(name, new FakePool());
			return pools.get(name);
		},
		oo1: { DB: FakeDb }
	})
}));

const { openSqlite } = await import('./sqlite');

function withLegacyPool(bytes: Uint8Array) {
	const legacy = new FakePool();
	legacy.files.set('/psp.db', bytes);
	pools.set('psp-sahpool', legacy);
	return legacy;
}

beforeEach(() => {
	pools = new Map();
	legacyBusy = false;
	vi.spyOn(console, 'warn').mockImplementation(() => {});
	vi.spyOn(console, 'error').mockImplementation(() => {});
	vi.stubGlobal('navigator', {
		storage: {
			getDirectory: async () => ({
				getDirectoryHandle: async (name: string) => {
					if (name === '.psp-sahpool' && pools.has('psp-sahpool')) return {};
					throw new DOMException('missing', 'NotFoundError');
				}
			})
		}
	});
});

describe('legacy OPFS pool adoption', () => {
	it('copies the legacy database into the new pool and removes the old pool', async () => {
		const legacy = withLegacyPool(new Uint8Array([1, 2, 3]));

		const db = await openSqlite({ retryDelayMs: 0 });

		expect(db.persistent).toBe(true);
		expect(Array.from(pools.get('ps-sahpool')!.files.get('/ps.db')!)).toEqual([1, 2, 3]);
		expect(legacy.removed).toBe(true);
	});

	it('stays in memory rather than start empty while an old tab holds the legacy pool', async () => {
		withLegacyPool(new Uint8Array([1]));
		legacyBusy = true;

		const db = await openSqlite({ poolAttempts: 1, retryDelayMs: 0 });

		expect(db.persistent).toBe(false);
		expect(pools.get('ps-sahpool')!.files.has('/ps.db')).toBe(false);
	});

	it('leaves the legacy pool alone once the new database exists', async () => {
		const legacy = withLegacyPool(new Uint8Array([1]));
		const current = new FakePool();
		current.files.set('/ps.db', new Uint8Array([9]));
		pools.set('ps-sahpool', current);

		await openSqlite({ retryDelayMs: 0 });

		expect(Array.from(current.files.get('/ps.db')!)).toEqual([9]);
		expect(legacy.removed).toBe(false);
	});

	it('does not create a legacy pool on a fresh origin', async () => {
		const db = await openSqlite({ retryDelayMs: 0 });

		expect(db.persistent).toBe(true);
		expect(pools.has('psp-sahpool')).toBe(false);
	});
});
