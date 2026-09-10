import { expect, test } from 'vitest';
import { followVerdict, FOLLOW_GRACE_MS } from './follow';
import type { LiveActorJson } from '$lib/signal/session.svelte';

const AREA = 'MainMap';

function actor(overrides: Partial<LiveActorJson> = {}): LiveActorJson {
	return { id: 'a1', kind: 'otomo', x: -201.1, y: -138238.3, z: 0, ...overrides };
}

test('eases toward a present, in-area, non-palbox actor and refreshes lastSeenMs', () => {
	const result = followVerdict([actor()], 'a1', AREA, 1_000, null);
	expect(result.verdict).toEqual({ kind: 'ease', x: -201.1, y: -138238.3 });
	expect(result.lastSeenMs).toBe(1_000);
});

test('drops a followed actor that is a palbox', () => {
	const result = followVerdict([actor({ kind: 'palbox' })], 'a1', AREA, 1_000, 500);
	expect(result.verdict).toEqual({ kind: 'drop' });
});

test('treats a missing actor as first-miss and starts waiting', () => {
	const result = followVerdict([], 'a1', AREA, 1_000, null);
	expect(result.verdict).toEqual({ kind: 'waiting' });
	expect(result.lastSeenMs).toBe(1_000);
});

test('keeps waiting while still within the grace period', () => {
	const result = followVerdict([], 'a1', AREA, 1_000 + FOLLOW_GRACE_MS - 1, 1_000);
	expect(result.verdict).toEqual({ kind: 'waiting' });
	expect(result.lastSeenMs).toBe(1_000);
});

test('drops once the grace period has elapsed', () => {
	const result = followVerdict([], 'a1', AREA, 1_000 + FOLLOW_GRACE_MS, 1_000);
	expect(result.verdict).toEqual({ kind: 'drop' });
	expect(result.lastSeenMs).toBe(1_000);
});

test('reappearing resets lastSeenMs to now', () => {
	const result = followVerdict([actor()], 'a1', AREA, 5_000, 1_000);
	expect(result.verdict).toEqual({ kind: 'ease', x: -201.1, y: -138238.3 });
	expect(result.lastSeenMs).toBe(5_000);
});

test('an actor that moved to another area is treated as absent (waiting inside grace)', () => {
	const result = followVerdict(
		[actor({ x: 9_999_999, y: 9_999_999 })],
		'a1',
		AREA,
		1_000,
		null
	);
	expect(result.verdict).toEqual({ kind: 'waiting' });
	expect(result.lastSeenMs).toBe(1_000);
});

test('an actor that moved to another area drops after the grace period', () => {
	const result = followVerdict(
		[actor({ x: 9_999_999, y: 9_999_999 })],
		'a1',
		AREA,
		1_000 + FOLLOW_GRACE_MS,
		1_000
	);
	expect(result.verdict).toEqual({ kind: 'drop' });
});

test('non-finite coordinates are treated as absent', () => {
	const result = followVerdict([actor({ x: NaN })], 'a1', AREA, 1_000, null);
	expect(result.verdict).toEqual({ kind: 'waiting' });
});
