import { WATCHTOWER_CLASS } from '$lib/components/map/fastTravel';
import { MessageType } from '$types';
import { readFile } from 'node:fs/promises';
import { fileURLToPath } from 'node:url';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

type Deferred = {
	layers: string[];
	resolve: (value: unknown) => void;
	reject: (reason: unknown) => void;
};

const pending: Deferred[] = [];
const sendAndWait = vi.fn((_type: unknown, data?: { layers: string[] }) => {
	return new Promise((resolve, reject) => {
		pending.push({ layers: data?.layers ?? [], resolve, reject });
	});
});

vi.mock('$lib/utils/websocketUtils', () => ({
	sendAndWait: (type: unknown, data?: { layers: string[] }) => sendAndWait(type, data)
}));

import { mapLayers } from './mapLayerStore.svelte';

const flush = () => new Promise((resolve) => setTimeout(resolve, 0));
const keys = (selection: { points: { key: string }[] }) => selection.points.map((p) => p.key);

// Requests are chained, so an unsettled one would stall every later test.
afterEach(async () => {
	for (let i = 0; i < 20 && pending.length > 0; i++) {
		pending.shift()!.resolve({ layers: {} });
		await flush();
	}
});

beforeEach(() => {
	pending.length = 0;
	sendAndWait.mockClear();
	mapLayers.reset();
	vi.spyOn(console, 'error').mockImplementation(() => {});
});

describe('mapLayers.getLayer', () => {
	it('asks for the layer artifact and returns its keyed entries', async () => {
		const got = mapLayers.getLayer('dungeons');
		await flush();

		expect(sendAndWait).toHaveBeenCalledWith(MessageType.GET_MAP_LAYER, { layers: ['dungeons'] });
		pending.shift()!.resolve({ layers: { dungeons: { d1: { x: 1 } } } });

		const selection = await got;
		expect(selection.shape).toBe('keyed');
		expect(keys(selection)).toEqual(['d1']);
	});

	it('never re-fetches an artifact already cached', async () => {
		const first = mapLayers.getLayer('dungeons');
		await flush();
		pending.shift()!.resolve({ layers: { dungeons: { d1: {} } } });
		await first;

		await mapLayers.getLayer('dungeons');
		await flush();
		expect(sendAndWait).toHaveBeenCalledTimes(1);
	});

	it('caches an artifact the response omitted, so a missing layer is not re-asked forever', async () => {
		const got = mapLayers.getLayer('camps');
		await flush();
		pending.shift()!.resolve({ layers: {} });
		expect((await got).points).toEqual([]);

		await mapLayers.getLayer('camps');
		await flush();
		expect(sendAndWait).toHaveBeenCalledTimes(1);
	});
});

// Some layers arrive as top-level arrays, others as keyed objects; both must
// survive the round trip intact.
describe('artifact shapes', () => {
	it('carries an array artifact through and keys it off the entries', async () => {
		const got = mapLayers.getLayer('camps');
		await flush();
		pending.shift()!.resolve({
			layers: {
				camps: [
					{ instance_id: 'IID_A', x: 1 },
					{ name: 'camp_b', x: 2 }
				]
			}
		});

		const selection = await got;
		expect(selection.shape).toBe('list');
		expect(keys(selection)).toEqual(['IID_A', 'camp_b']);
	});

	it('reports the keyed shape for an object artifact', async () => {
		const got = mapLayers.getLayer('tower_boss');
		await flush();
		pending.shift()!.resolve({ layers: { towers: { DesertBoss: { x: 1 } } } });

		const selection = await got;
		expect(selection.shape).toBe('keyed');
		expect(keys(selection)).toEqual(['DesertBoss']);
	});
});

// sendAndWait keys pending resolvers by message type alone, so a second
// get_map_layer on the wire would overwrite the first resolver and the first
// promise would never settle -- a silent hang, not an error.
describe('request coalescing', () => {
	it('batches two concurrent layer requests into one message and settles both', async () => {
		const dungeons = mapLayers.getLayer('dungeons');
		const camps = mapLayers.getLayer('camps');
		await flush();

		expect(sendAndWait).toHaveBeenCalledTimes(1);
		expect(pending).toHaveLength(1);
		expect(pending[0].layers).toEqual(['dungeons', 'camps']);

		pending.shift()!.resolve({ layers: { dungeons: { d1: {} }, camps: [{ instance_id: 'c1' }] } });

		expect(keys(await dungeons)).toEqual(['d1']);
		expect(keys(await camps)).toEqual(['c1']);
		expect(sendAndWait).toHaveBeenCalledTimes(1);
	});

	it('collapses layers that share one artifact into a single artifact id', async () => {
		const ft = mapLayers.getLayer('fast_travel');
		const wt = mapLayers.getLayer('watchtower');
		await flush();

		expect(sendAndWait).toHaveBeenCalledTimes(1);
		expect(pending[0].layers).toEqual(['fast_travel_points']);

		pending.shift()!.resolve({
			layers: {
				fast_travel_points: {
					a: { class: 'BP_LevelObject_TowerFastTravelPoint_C' },
					b: { class: WATCHTOWER_CLASS }
				}
			}
		});

		expect(keys(await ft)).toEqual(['a']);
		expect(keys(await wt)).toEqual(['b']);
	});

	it('serialises a request made while one is already on the wire', async () => {
		const dungeons = mapLayers.getLayer('dungeons');
		await flush();
		expect(sendAndWait).toHaveBeenCalledTimes(1);

		const camps = mapLayers.getLayer('camps');
		await flush();
		expect(sendAndWait).toHaveBeenCalledTimes(1);

		pending.shift()!.resolve({ layers: { dungeons: { d1: {} } } });
		await dungeons;
		await flush();

		expect(sendAndWait).toHaveBeenCalledTimes(2);
		expect(pending[0].layers).toEqual(['camps']);
		pending.shift()!.resolve({ layers: { camps: [{ instance_id: 'c1' }] } });
		expect(keys(await camps)).toEqual(['c1']);
	});

	// A refusal answers under get_map_layer carrying `error` rather than as an
	// error frame, so sendAndWait resolves and the store would otherwise cache
	// silence.
	it('reports a refusal that came back under the layer message type', async () => {
		const got = mapLayers.getLayer('camps');
		await flush();
		pending.shift()!.resolve({ error: 'Unknown map layer: camps' });

		expect((await got).points).toEqual([]);
		expect(console.error).toHaveBeenCalledWith(
			expect.stringContaining('map layers'),
			'Unknown map layer: camps'
		);
	});

	it('does not deadlock the queue when a request fails', async () => {
		const dungeons = mapLayers.getLayer('dungeons');
		await flush();
		pending.shift()!.reject(new Error('unknown layer'));
		expect((await dungeons).points).toEqual([]);
		await flush();

		const camps = mapLayers.getLayer('camps');
		await flush();
		expect(sendAndWait).toHaveBeenCalledTimes(2);
		pending.shift()!.resolve({ layers: { camps: [] } });
		expect((await camps).points).toEqual([]);
	});
});

describe('mapLayers.getLayers', () => {
	it('resolves several layers from a single message keyed by layer id', async () => {
		const got = mapLayers.getLayers(['alpha_pals', 'predator_pals', 'dungeons']);
		await flush();

		expect(sendAndWait).toHaveBeenCalledTimes(1);
		expect(pending[0].layers).toEqual(['bosses', 'dungeons']);

		pending.shift()!.resolve({
			layers: {
				bosses: { a: { spawn_type: 'alpha' }, p: { spawn_type: 'predator' } },
				dungeons: { d1: {} }
			}
		});

		const result = await got;
		expect(keys(result.alpha_pals)).toEqual(['a']);
		expect(keys(result.predator_pals)).toEqual(['p']);
		expect(keys(result.dungeons)).toEqual(['d1']);
	});
});

describe('mapLayers.isLoading', () => {
	it('is false for a layer nobody has asked for', () => {
		expect(mapLayers.isLoading('dungeons')).toBe(false);
	});

	it('is true from the moment a layer is requested until its response lands', async () => {
		const got = mapLayers.getLayer('dungeons');
		expect(mapLayers.isLoading('dungeons')).toBe(true);

		await flush();
		expect(mapLayers.isLoading('dungeons')).toBe(true);

		pending.shift()!.resolve({ layers: { dungeons: { d1: {} } } });
		await got;
		expect(mapLayers.isLoading('dungeons')).toBe(false);
	});

	it('is true for a layer still buffered behind an in-flight request', async () => {
		const dungeons = mapLayers.getLayer('dungeons');
		await flush();
		void mapLayers.getLayer('camps');

		expect(mapLayers.isLoading('camps')).toBe(true);

		pending.shift()!.resolve({ layers: { dungeons: {} } });
		await dungeons;
		await flush();
		expect(mapLayers.isLoading('camps')).toBe(true);

		pending.shift()!.resolve({ layers: { camps: [] } });
		await flush();
		expect(mapLayers.isLoading('camps')).toBe(false);
	});

	it('clears the flag when the batch it joined turns out to be empty', async () => {
		const first = mapLayers.getLayer('dungeons');
		await flush();
		pending.shift()!.resolve({ layers: { dungeons: { d1: {} } } });
		await first;

		void mapLayers.getLayer('dungeons');
		await flush();
		expect(mapLayers.isLoading('dungeons')).toBe(false);
	});

	it('is false once cached, and false again after a failure so the row is not stuck', async () => {
		const got = mapLayers.getLayer('dungeons');
		await flush();
		pending.shift()!.reject(new Error('socket closed'));
		await got;
		expect(mapLayers.isLoading('dungeons')).toBe(false);
	});

	it('clears on reset', async () => {
		void mapLayers.getLayer('dungeons');
		await flush();
		expect(mapLayers.isLoading('dungeons')).toBe(true);
		mapLayers.reset();
		expect(mapLayers.isLoading('dungeons')).toBe(false);
	});
});

describe('mapLayers.peek', () => {
	it('returns undefined before the artifact lands and the entries after', async () => {
		expect(mapLayers.peek('dungeons')).toBeUndefined();

		const got = mapLayers.getLayer('dungeons');
		await flush();
		pending.shift()!.resolve({ layers: { dungeons: { d1: {} } } });
		await got;

		expect(keys(mapLayers.peek('dungeons')!)).toEqual(['d1']);
	});
});

describe('mapLayers.reset', () => {
	it('clears the cache so the next request refetches', async () => {
		const got = mapLayers.getLayer('dungeons');
		await flush();
		pending.shift()!.resolve({ layers: { dungeons: { d1: {} } } });
		await got;

		mapLayers.reset();
		expect(mapLayers.peek('dungeons')).toBeUndefined();

		void mapLayers.getLayer('dungeons');
		await flush();
		expect(sendAndWait).toHaveBeenCalledTimes(2);
	});

	it('discards a response that arrives after reset()', async () => {
		const got = mapLayers.getLayer('dungeons');
		await flush();

		mapLayers.reset();
		pending.shift()!.resolve({ layers: { dungeons: { stale: {} } } });
		await got;

		expect(mapLayers.peek('dungeons')).toBeUndefined();
	});
});

// Identity alone cannot prove this: vitest compiles .svelte.ts for the server,
// where deep $state does not proxy and `toBe` holds either way. So the
// declaration is checked at the source instead.
describe('marker tables are not deeply proxied', () => {
	it('declares the artifact cache with $state.raw', async () => {
		const source = await readFile(
			fileURLToPath(new URL('./mapLayerStore.svelte.ts', import.meta.url)),
			'utf8'
		);
		expect(source).toMatch(/#artifacts[^\n]*=\s*\$state\.raw\(/);
		expect(source).not.toMatch(/#artifacts[^\n]*=\s*\$state\(/);
	});

	it('hands back the exact entry objects it was given', async () => {
		const entry = { x: 1 };
		const got = mapLayers.getLayer('dungeons');
		await flush();
		pending.shift()!.resolve({ layers: { dungeons: { d1: entry } } });

		expect((await got).points[0].entry).toBe(entry);
	});

	it('hands back the exact entry objects for a subset layer', async () => {
		const alpha = { spawn_type: 'alpha' };
		const got = mapLayers.getLayer('alpha_pals');
		await flush();
		pending.shift()!.resolve({ layers: { bosses: { a: alpha, b: { spawn_type: 'boss' } } } });

		expect((await got).points[0].entry).toBe(alpha);
	});
});
