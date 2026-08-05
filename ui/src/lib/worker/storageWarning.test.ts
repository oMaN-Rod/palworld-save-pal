import { describe, it, expect, vi } from 'vitest';

vi.mock('@sqlite.org/sqlite-wasm', () => ({ default: async () => ({}) }));

import { storageWarning } from './sqlite';

describe('storageWarning', () => {
	it('says nothing when the database persists', () => {
		expect(storageWarning({ persistent: true, opfsSupported: true })).toBeNull();
	});

	// The case that made this function exist: Chrome supports everything, but the
	// OPFS SAH pool is exclusive per origin, so a second tab loses the lock.
	// Blaming the browser there sends users chasing a non-existent problem.
	it('blames the other tab, not the browser, when the browser has sync access handles', () => {
		const msg = storageWarning({ persistent: false, opfsSupported: true });
		expect(msg).toMatch(/another tab/i);
		expect(msg).not.toMatch(/this browser cannot/i);
	});

	it('blames the browser only when sync access handles are missing', () => {
		const msg = storageWarning({ persistent: false, opfsSupported: false });
		expect(msg).toMatch(/this browser cannot store data/i);
		expect(msg).not.toMatch(/another tab/i);
	});
});
