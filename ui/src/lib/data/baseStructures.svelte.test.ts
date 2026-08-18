import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

type Deferred = {
	baseId: string | undefined;
	resolve: (value: unknown) => void;
	reject: (reason: unknown) => void;
};

// loadFootprints sends no payload, so base_id is read optionally: reading it
// through `!` threw inside the mock and left `pending` empty, which looks exactly
// like a request that was never made.
const pending: Deferred[] = [];
const sendAndWait = vi.fn((_type: unknown, data?: { base_id: string }) => {
	return new Promise((resolve, reject) => {
		pending.push({ baseId: data?.base_id, resolve, reject });
	});
});

vi.mock('$lib/utils/websocketUtils', () => ({
	sendAndWait: (type: unknown, data?: { base_id: string }) => sendAndWait(type, data)
}));

import { baseStructuresData } from './baseStructures.svelte';

const structure = (id: string) => [{ id }] as never;

const flush = () => new Promise((resolve) => setTimeout(resolve, 0));

// The store chains requests, so a test that leaves one unsettled would stall the
// queue for every test after it.
afterEach(async () => {
	for (let i = 0; i < 20 && pending.length > 0; i++) {
		pending.shift()!.resolve({});
		await flush();
	}
});

beforeEach(() => {
	pending.length = 0;
	sendAndWait.mockClear();
	baseStructuresData.reset();
	vi.spyOn(console, 'error').mockImplementation(() => {});
});

describe('baseStructures.load', () => {
	it('serialises overlapping requests so only one is on the wire', async () => {
		const a = baseStructuresData.load('A');
		const b = baseStructuresData.load('B');
		await flush();

		expect(sendAndWait).toHaveBeenCalledTimes(1);
		expect(pending[0].baseId).toBe('A');

		pending[0].resolve({ base_id: 'A', structures: structure('sa') });
		await a;
		await flush();

		expect(sendAndWait).toHaveBeenCalledTimes(2);
		expect(pending[1].baseId).toBe('B');

		pending[1].resolve({ base_id: 'B', structures: structure('sb') });
		await b;

		expect(baseStructuresData.for('A')).toEqual([{ id: 'sa' }]);
		expect(baseStructuresData.for('B')).toEqual([{ id: 'sb' }]);
	});

	it('does not re-request a base that is already inflight', async () => {
		void baseStructuresData.load('A');
		void baseStructuresData.load('A');
		await flush();
		expect(sendAndWait).toHaveBeenCalledTimes(1);
	});

	it('rejects a response whose base_id does not match and allows a retry', async () => {
		const a = baseStructuresData.load('A');
		await flush();
		pending[0].resolve({ base_id: 'B', structures: structure('sb') });
		await a;

		expect(baseStructuresData.for('A')).toEqual([]);

		void baseStructuresData.load('A');
		await flush();
		expect(sendAndWait).toHaveBeenCalledTimes(2);
	});

	it('caches an empty list when the response carries no structures', async () => {
		const a = baseStructuresData.load('A');
		await flush();
		pending[0].resolve({ error: 'No save file loaded' });
		await a;

		expect(baseStructuresData.for('A')).toEqual([]);

		void baseStructuresData.load('A');
		await flush();
		expect(sendAndWait).toHaveBeenCalledTimes(1);
	});

	it('does not deadlock the queue when a request throws', async () => {
		const a = baseStructuresData.load('A');
		const b = baseStructuresData.load('B');
		await flush();

		pending[0].reject(new Error('socket closed'));
		await a;
		await flush();

		expect(sendAndWait).toHaveBeenCalledTimes(2);
		pending[1].resolve({ base_id: 'B', structures: structure('sb') });
		await b;
		expect(baseStructuresData.for('B')).toEqual([{ id: 'sb' }]);
	});

	it('caches empty on failure and stops re-requesting', async () => {
		const a = baseStructuresData.load('A');
		await flush();
		pending[0].reject(new Error('socket closed'));
		await a;

		expect(baseStructuresData.for('A')).toEqual([]);

		void baseStructuresData.load('A');
		await flush();
		expect(sendAndWait).toHaveBeenCalledTimes(1);
	});

	it('discards a response that arrives after reset()', async () => {
		const a = baseStructuresData.load('A');
		await flush();

		baseStructuresData.reset();
		pending[0].resolve({ base_id: 'A', structures: structure('stale') });
		await a;

		expect(baseStructuresData.for('A')).toEqual([]);

		void baseStructuresData.load('A');
		await flush();
		expect(sendAndWait).toHaveBeenCalledTimes(2);
	});

	it('does not cache a stale-epoch failure over a fresh result', async () => {
		const stale = baseStructuresData.load('A');
		await flush();
		baseStructuresData.reset();
		pending[0].reject(new Error('socket closed'));
		await stale;
		await flush();

		const fresh = baseStructuresData.load('A');
		await flush();
		pending[1].resolve({ base_id: 'A', structures: structure('fresh') });
		await fresh;

		expect(baseStructuresData.for('A')).toEqual([{ id: 'fresh' }]);
	});
});

// A base holds thousands of structures whose fields are read on every rebuild,
// and under a deep $state each read goes through Svelte's proxy: profiling one
// base load attributed 83 s to get_proxied_value and 56 s to the proxy's get
// handler, against 495 ms of real work. The store replaces these collections
// wholesale, so they must not be deeply reactive. Reference identity is the
// observable form of that -- a proxied array is never the one handed in.
describe('bulk collections are replaced wholesale', () => {
	it('hands back the exact structure array it was given', async () => {
		const structures = [{ id: 's1' }, { id: 's2' }] as never;
		const load = baseStructuresData.load('RAW');
		await flush();
		pending.shift()!.resolve({ base_id: 'RAW', structures });
		await load;

		expect(baseStructuresData.for('RAW')).toBe(structures);
	});

	it('hands back the exact structure objects it was given', async () => {
		const first = { id: 's1' };
		const structures = [first] as never;
		const load = baseStructuresData.load('RAW_ITEMS');
		await flush();
		pending.shift()!.resolve({ base_id: 'RAW_ITEMS', structures });
		await load;

		expect(baseStructuresData.for('RAW_ITEMS')[0]).toBe(first);
	});

	it('hands back the exact footprints object it was given', async () => {
		const footprints = { Wall: { sx: 1, sy: 1, sz: 1 } } as never;
		const load = baseStructuresData.loadFootprints();
		await flush();
		pending.shift()!.resolve(footprints);
		await load;

		expect(baseStructuresData.footprints).toBe(footprints);
	});

	// Consumers react to the replacement, so a second base must still be visible
	// after the first.
	it('keeps earlier bases readable when a later one lands', async () => {
		const a = [{ id: 'a' }] as never;
		const b = [{ id: 'b' }] as never;

		const loadA = baseStructuresData.load('A2');
		await flush();
		pending.shift()!.resolve({ base_id: 'A2', structures: a });
		await loadA;

		const loadB = baseStructuresData.load('B2');
		await flush();
		pending.shift()!.resolve({ base_id: 'B2', structures: b });
		await loadB;

		expect(baseStructuresData.for('A2')).toBe(a);
		expect(baseStructuresData.for('B2')).toBe(b);
	});
});
