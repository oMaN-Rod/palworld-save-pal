import { describe, expect, it } from 'vitest';
import { mintTurnCredential, handleTurnRequest } from '../src/turn';

describe('mintTurnCredential', () => {
	it('matches the reference HMAC-SHA1 vector', async () => {
		// printf '%s' "1700000000:ps" | openssl dgst -sha1 -hmac "test-secret" -binary | base64
		const { username, credential } = await mintTurnCredential('test-secret', 1_699_996_400_000, 3600);
		expect(username).toBe('1700000000:ps');
		expect(credential).toBe('gcTJTYkt1EYSGfIec80pYtcc2II=');
	});
});

describe('handleTurnRequest', () => {
	const env = (over = {}) =>
		({
			TURN_SECRET: 'test-secret',
			TURN_URLS: 'turn:turn.example.com:3478?transport=udp,turns:turn.example.com:443?transport=tcp',
			TURN_TTL_SECONDS: '3600',
			...over
		}) as any;

	it('answers with urls, credential and no-store', async () => {
		const res = await handleTurnRequest(new Request('https://x/signal/turn'), env());
		expect(res.status).toBe(200);
		expect(res.headers.get('cache-control')).toBe('no-store');
		const body = (await res.json()) as any;
		expect(body.urls).toHaveLength(2);
		expect(body.username.endsWith(':ps')).toBe(true);
		expect(body.ttl_seconds).toBe(3600);
	});

	it('is 503 when the secret is not configured', async () => {
		const res = await handleTurnRequest(new Request('https://x/signal/turn'), env({ TURN_SECRET: undefined }));
		expect(res.status).toBe(503);
		expect(res.headers.get('cache-control')).toBe('no-store');
	});

	it('is 429 when the rate limiter says no', async () => {
		const limited = env({ TURN_RATE: { limit: async () => ({ success: false }) } });
		const res = await handleTurnRequest(new Request('https://x/signal/turn'), limited);
		expect(res.status).toBe(429);
		expect(res.headers.get('retry-after')).toBe('60');
		expect(res.headers.get('cache-control')).toBe('no-store');
	});

	it('falls back to a 3600s ttl when TURN_TTL_SECONDS is an empty string', async () => {
		const res = await handleTurnRequest(new Request('https://x/signal/turn'), env({ TURN_TTL_SECONDS: '' }));
		const body = (await res.json()) as any;
		expect(body.ttl_seconds).toBe(3600);
	});

	it('passes when the rate limiter allows', async () => {
		const open = env({ TURN_RATE: { limit: async () => ({ success: true }) } });
		expect((await handleTurnRequest(new Request('https://x/signal/turn'), open)).status).toBe(200);
	});
});
