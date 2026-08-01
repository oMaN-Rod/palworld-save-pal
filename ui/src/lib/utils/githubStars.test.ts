import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { fetchGithubStars, formatStars } from './githubStars';

function installFakeSessionStorage() {
	const store = new Map<string, string>();
	vi.stubGlobal('sessionStorage', {
		getItem: (k: string) => (store.has(k) ? store.get(k)! : null),
		setItem: (k: string, v: string) => {
			store.set(k, v);
		},
		removeItem: (k: string) => {
			store.delete(k);
		},
		clear: () => store.clear(),
		key: () => null,
		length: 0
	} as Storage);
}

describe('formatStars', () => {
	it('formats counts below 1000 as-is', () => {
		expect(formatStars(0)).toBe('0');
		expect(formatStars(999)).toBe('999');
	});
	it('formats thousands with one decimal, trimming .0', () => {
		expect(formatStars(1200)).toBe('1.2k');
		expect(formatStars(2000)).toBe('2k');
	});
	it('formats ten-thousands without a decimal', () => {
		expect(formatStars(15000)).toBe('15k');
	});
});

describe('fetchGithubStars', () => {
	beforeEach(() => {
		installFakeSessionStorage();
	});
	afterEach(() => {
		vi.unstubAllGlobals();
		vi.restoreAllMocks();
	});

	it('returns stargazers_count on a 200 response', async () => {
		vi.stubGlobal(
			'fetch',
			vi.fn().mockResolvedValue({ ok: true, json: async () => ({ stargazers_count: 42 }) })
		);
		expect(await fetchGithubStars('a/b')).toBe(42);
	});

	it('returns null on a non-200 response', async () => {
		vi.stubGlobal('fetch', vi.fn().mockResolvedValue({ ok: false, json: async () => ({}) }));
		expect(await fetchGithubStars('a/b')).toBeNull();
	});

	it('returns null when fetch throws', async () => {
		vi.stubGlobal('fetch', vi.fn().mockRejectedValue(new Error('offline')));
		expect(await fetchGithubStars('a/b')).toBeNull();
	});

	it('caches a successful result and does not refetch', async () => {
		const spy = vi
			.fn()
			.mockResolvedValue({ ok: true, json: async () => ({ stargazers_count: 7 }) });
		vi.stubGlobal('fetch', spy);
		expect(await fetchGithubStars('a/b')).toBe(7);
		expect(await fetchGithubStars('a/b')).toBe(7);
		expect(spy).toHaveBeenCalledTimes(1);
	});
});
