import { describe, expect, it } from 'vitest';
import {
	detectCapabilities,
	hardBlocked,
	limitations,
	type CapabilityScope
} from './browserCapabilities';

const fullSupport: CapabilityScope = {
	WebAssembly: {},
	Worker: function () {},
	showDirectoryPicker: function () {},
	navigator: { storage: { getDirectory: function () {} } },
	indexedDB: {}
};

function without(keys: (keyof CapabilityScope)[]): CapabilityScope {
	const s: CapabilityScope = { ...fullSupport };
	for (const k of keys) delete s[k];
	return s;
}

describe('detectCapabilities', () => {
	it('reports everything present on a fully capable scope', () => {
		expect(detectCapabilities(fullSupport)).toEqual({
			wasm: true,
			workers: true,
			fsa: true,
			opfs: true,
			indexedDb: true
		});
	});

	it('detects a Safari-like scope: no FSA, storage intact', () => {
		const c = detectCapabilities(without(['showDirectoryPicker']));
		expect(c.fsa).toBe(false);
		expect(c.opfs).toBe(true);
		expect(c.indexedDb).toBe(true);
		expect(hardBlocked(c)).toBe(false);
		expect(limitations(c)).toEqual(['fsa']);
	});

	it('ignores a FileSystemFileHandle without a sync access handle', () => {
		// createSyncAccessHandle is [Exposed=DedicatedWorker], so it is absent on
		// the main thread in EVERY browser. Detection must not read anything off
		// FileSystemFileHandle, or every web user is told storage is broken.
		const scope = { ...fullSupport, FileSystemFileHandle: { prototype: {} } };
		expect(limitations(detectCapabilities(scope))).toEqual([]);
	});

	it('detects a private-mode-like scope with no storage APIs', () => {
		const priv = without(['indexedDB']);
		priv.navigator = {};
		const c = detectCapabilities(priv);
		expect(c.opfs).toBe(false);
		expect(c.indexedDb).toBe(false);
		expect(hardBlocked(c)).toBe(false);
	});

	it('treats a null indexedDB as unavailable', () => {
		const c = detectCapabilities({ ...fullSupport, indexedDB: null });
		expect(c.indexedDb).toBe(false);
	});

	it('does not throw when navigator is absent entirely', () => {
		expect(() => detectCapabilities({})).not.toThrow();
		expect(detectCapabilities({}).opfs).toBe(false);
	});
});

describe('hardBlocked', () => {
	it('blocks when WebAssembly is missing', () => {
		expect(hardBlocked(detectCapabilities(without(['WebAssembly'])))).toBe(true);
	});

	it('blocks when Worker is missing', () => {
		expect(hardBlocked(detectCapabilities(without(['Worker'])))).toBe(true);
	});

	it('does not block on degradable losses alone', () => {
		expect(hardBlocked(detectCapabilities(without(['showDirectoryPicker', 'indexedDB'])))).toBe(
			false
		);
	});
});

describe('limitations', () => {
	it('returns nothing when everything is supported', () => {
		expect(limitations(detectCapabilities(fullSupport))).toEqual([]);
	});

	it('lists each degradable loss in a stable order', () => {
		const priv = without(['showDirectoryPicker', 'indexedDB']);
		priv.navigator = {};
		expect(limitations(detectCapabilities(priv))).toEqual(['fsa', 'opfs', 'indexedDb']);
	});

	it('never lists hard requirements', () => {
		// Missing WebAssembly AND FSA: without the hard-block short-circuit
		// this would return ['fsa'].
		expect(
			limitations(detectCapabilities(without(['WebAssembly', 'showDirectoryPicker'])))
		).toEqual([]);
	});
});
