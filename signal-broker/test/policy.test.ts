import { describe, expect, test } from 'vitest';
import { MAX_FRAME_BYTES, takeToken, validateUpgrade, RATE } from '../src/policy';

test('rejects malformed rooms and roles', () => {
	expect(validateUpgrade(new URL('https://x/signal/ws?room=zz&role=host'))).toHaveProperty('error');
	expect(validateUpgrade(new URL('https://x/signal/ws?room=' + 'a'.repeat(32) + '&role=admin'))).toHaveProperty('error');
	expect(validateUpgrade(new URL('https://x/signal/ws?room=' + 'a'.repeat(32) + '&role=guest'))).toEqual({
		room: 'a'.repeat(32),
		role: 'guest',
		kind: 'pair',
	});
});

test('kind defaults to pair, accepts meet, rejects junk', () => {
	const room = 'a'.repeat(32);
	expect(validateUpgrade(new URL(`https://x/signal/ws?room=${room}&role=host`))).toEqual({ room, role: 'host', kind: 'pair' });
	expect(validateUpgrade(new URL(`https://x/signal/ws?room=${room}&role=host&kind=pair`))).toEqual({ room, role: 'host', kind: 'pair' });
	expect(validateUpgrade(new URL(`https://x/signal/ws?room=${room}&role=host&kind=meet`))).toEqual({ room, role: 'host', kind: 'meet' });
	expect(validateUpgrade(new URL(`https://x/signal/ws?room=${room}&role=host&kind=bogus`))).toHaveProperty('error');
});

test('token bucket limits burst and refills', () => {
	const b = { tokens: RATE.capacity, updatedMs: 0 };
	for (let i = 0; i < RATE.capacity; i++) expect(takeToken(b, 0)).toBe(true);
	expect(takeToken(b, 0)).toBe(false);
	expect(takeToken(b, 1000)).toBe(true);
});
