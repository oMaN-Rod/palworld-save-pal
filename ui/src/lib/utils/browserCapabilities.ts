/** Injectable so detection unit-tests against plain objects, with no browser. */
export interface CapabilityScope {
	WebAssembly?: unknown;
	Worker?: unknown;
	showDirectoryPicker?: unknown;
	navigator?: { storage?: { getDirectory?: unknown } };
	FileSystemFileHandle?: { prototype?: { createSyncAccessHandle?: unknown } };
	indexedDB?: unknown;
}

export interface Capabilities {
	wasm: boolean;
	workers: boolean;
	fsa: boolean;
	opfs: boolean;
	opfsSyncAccess: boolean;
	indexedDb: boolean;
}

export type LimitationKey = 'fsa' | 'opfs' | 'opfsSyncAccess' | 'indexedDb';

// `workers` checks that the Worker constructor exists, NOT that module workers
// are supported — workerTransport builds one with { type: 'module' }, and module
// support cannot be probed synchronously. Module workers shipped in Chrome 80,
// Safari 15 and Firefox 114, so any browser reaching the rest of this list has
// them; the flag is named for what it actually checks.
export function detectCapabilities(
	scope: CapabilityScope = globalThis as CapabilityScope
): Capabilities {
	return {
		wasm: typeof scope.WebAssembly === 'object' && scope.WebAssembly !== null,
		workers: typeof scope.Worker === 'function',
		fsa: typeof scope.showDirectoryPicker === 'function',
		opfs: typeof scope.navigator?.storage?.getDirectory === 'function',
		opfsSyncAccess:
			typeof scope.FileSystemFileHandle?.prototype?.createSyncAccessHandle === 'function',
		indexedDb: scope.indexedDB !== undefined && scope.indexedDB !== null
	};
}

export function hardBlocked(c: Capabilities): boolean {
	return !c.wasm || !c.workers;
}

export function limitations(c: Capabilities): LimitationKey[] {
	if (hardBlocked(c)) return [];
	const out: LimitationKey[] = [];
	if (!c.fsa) out.push('fsa');
	if (!c.opfs) out.push('opfs');
	if (!c.opfsSyncAccess) out.push('opfsSyncAccess');
	if (!c.indexedDb) out.push('indexedDb');
	return out;
}
