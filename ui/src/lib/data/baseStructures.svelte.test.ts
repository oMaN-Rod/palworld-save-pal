import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

type Deferred = {
	baseId: string;
	resolve: (value: unknown) => void;
	reject: (reason: unknown) => void;
};

const pending: Deferred[] = [];
const sendAndWait = vi.fn((_type: unknown, data?: { base_id: string }) => {
	return new Promise((resolve, reject) => {
		pending.push({ baseId: data!.base_id, resolve, reject });
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
