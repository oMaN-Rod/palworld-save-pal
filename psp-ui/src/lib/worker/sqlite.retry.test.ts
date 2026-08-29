import { beforeEach, describe, expect, it, vi } from 'vitest';

let attempts = 0;
let failuresBeforeSuccess = 0;

class FakeDb {
	exec() {}
	changes() {
		return 0;
	}
}

vi.mock('@sqlite.org/sqlite-wasm', () => ({
	default: async () => ({
		installOpfsSAHPoolVfs: async () => {
			attempts += 1;
			if (attempts <= failuresBeforeSuccess) {
				throw new Error(
					'NoModificationAllowedError: Access Handles cannot be created if there is another open Access Handle'
				);
			}
			return { OpfsSAHPoolDb: FakeDb };
		},
		oo1: { DB: FakeDb }
	})
}));

const { openSqlite } = await import('./sqlite');

beforeEach(() => {
	attempts = 0;
	failuresBeforeSuccess = 0;
	vi.spyOn(console, 'warn').mockImplementation(() => {});
	vi.spyOn(console, 'error').mockImplementation(() => {});
});

describe('OPFS pool acquisition', () => {
	// A reload's new worker starts before the departing page's worker has
	// released its access handles. Giving up on the first failure downgrades the
	// whole session to in-memory, which silently stops persisting settings,
	// presets and blueprints.
	it('acquires the pool on a retry when a departing page still holds it', async () => {
		failuresBeforeSuccess = 2;

		const db = await openSqlite({ retryDelayMs: 0 });

		expect(db.persistent).toBe(true);
		expect(attempts).toBe(3);
	});

	it('falls back to in-memory once the retries are exhausted', async () => {
		failuresBeforeSuccess = Number.MAX_SAFE_INTEGER;

		const db = await openSqlite({ poolAttempts: 3, retryDelayMs: 0 });

		expect(db.persistent).toBe(false);
		expect(attempts).toBe(3);
	});

	it('does not retry when the pool is available immediately', async () => {
		const db = await openSqlite({ retryDelayMs: 0 });

		expect(db.persistent).toBe(true);
		expect(attempts).toBe(1);
	});
});
