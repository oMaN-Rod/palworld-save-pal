import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { createTurnProvider } from './turnCredential';

function jsonResponse(body: unknown, ok = true): Response {
	return {
		ok,
		json: async () => body
	} as Response;
}

const TURN_BODY = {
	urls: ['turn:relay.example:3478?transport=udp'],
	username: '1700000000:ps',
	credential: 'abc=',
	ttl_seconds: 3600
};

describe('createTurnProvider', () => {
	beforeEach(() => {
		vi.useFakeTimers();
	});

	afterEach(() => {
		vi.useRealTimers();
		vi.unstubAllEnvs();
	});

	it('mints a credential on the first call', async () => {
		const fetchFn = vi.fn().mockResolvedValue(jsonResponse(TURN_BODY));
		const provider = createTurnProvider(fetchFn, () => 0);

		const servers = await provider();

		expect(fetchFn).toHaveBeenCalledTimes(1);
		expect(servers).toEqual([
			{
				urls: TURN_BODY.urls,
				username: TURN_BODY.username,
				credential: TURN_BODY.credential
			}
		]);
	});

	it('caches the minted entry within its TTL, fetching once for two calls', async () => {
		const fetchFn = vi.fn().mockResolvedValue(jsonResponse(TURN_BODY));
		let nowMs = 0;
		const provider = createTurnProvider(fetchFn, () => nowMs);

		await provider();
		nowMs += 60_000;
		const second = await provider();

		expect(fetchFn).toHaveBeenCalledTimes(1);
		expect(second).toEqual([
			{
				urls: TURN_BODY.urls,
				username: TURN_BODY.username,
				credential: TURN_BODY.credential
			}
		]);
	});

	it('refreshes once less than 5 minutes of TTL remain', async () => {
		const fetchFn = vi.fn().mockResolvedValue(jsonResponse(TURN_BODY));
		let nowMs = 0;
		const provider = createTurnProvider(fetchFn, () => nowMs);

		await provider();
		nowMs += (3600 - 60) * 1000;
		await provider();

		expect(fetchFn).toHaveBeenCalledTimes(2);
	});

	it('returns [] on a non-200 response', async () => {
		const fetchFn = vi.fn().mockResolvedValue(jsonResponse({}, false));
		const provider = createTurnProvider(fetchFn, () => 0);

		expect(await provider()).toEqual([]);
	});

	it('returns [] when the fetch throws', async () => {
		const fetchFn = vi.fn().mockRejectedValue(new Error('network down'));
		const provider = createTurnProvider(fetchFn, () => 0);

		expect(await provider()).toEqual([]);
	});

	it('returns [] and aborts a fetch slower than 3s', async () => {
		let capturedSignal: AbortSignal | undefined;
		const fetchFn = vi.fn().mockImplementation(
			(_url: string, init?: RequestInit) =>
				new Promise((_resolve, reject) => {
					capturedSignal = init?.signal ?? undefined;
					init?.signal?.addEventListener('abort', () => reject(new Error('aborted')));
				})
		);
		const provider = createTurnProvider(fetchFn, () => 0);

		const pending = provider();
		await vi.advanceTimersByTimeAsync(3_000);

		expect(await pending).toEqual([]);
		expect(capturedSignal?.aborted).toBe(true);
	});

	it('derives the URL from VITE_SIGNAL_BROKER_URL, swapping ws for http', async () => {
		vi.stubEnv('VITE_SIGNAL_BROKER_URL', 'ws://localhost:8787');
		const fetchFn = vi.fn().mockResolvedValue(jsonResponse(TURN_BODY));
		const provider = createTurnProvider(fetchFn, () => 0);

		await provider();

		expect(fetchFn).toHaveBeenCalledWith(
			'http://localhost:8787/signal/turn',
			expect.anything()
		);
	});

	it('derives the URL from VITE_SIGNAL_BROKER_URL, swapping wss for https', async () => {
		vi.stubEnv('VITE_SIGNAL_BROKER_URL', 'wss://palstudio.app');
		const fetchFn = vi.fn().mockResolvedValue(jsonResponse(TURN_BODY));
		const provider = createTurnProvider(fetchFn, () => 0);

		await provider();

		expect(fetchFn).toHaveBeenCalledWith(
			'https://palstudio.app/signal/turn',
			expect.anything()
		);
	});

	it('returns [] when the response body has no urls', async () => {
		const fetchFn = vi
			.fn()
			.mockResolvedValue(jsonResponse({ username: 'u', credential: 'c', ttl_seconds: 3600 }));
		const provider = createTurnProvider(fetchFn, () => 0);

		expect(await provider()).toEqual([]);
	});

	it('returns [] when the response urls array is empty', async () => {
		const fetchFn = vi.fn().mockResolvedValue(jsonResponse({ ...TURN_BODY, urls: [] }));
		const provider = createTurnProvider(fetchFn, () => 0);

		expect(await provider()).toEqual([]);
	});
});
