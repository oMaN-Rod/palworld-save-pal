import { beforeEach, expect, test } from 'vitest';
import { getLiveActors } from './liveActors.svelte';
import type { LiveFrameJson } from '$lib/signal/session.svelte';

beforeEach(() => {
	getLiveActors().clear();
});

function frame(): LiveFrameJson {
	return {
		seq: 1,
		source: 'test',
		capturedAtMs: 0,
		actors: [{ id: 'a1', kind: 'otomo', x: 1, y: 2, z: 3 }]
	};
}

test('follow/unfollow are reactive', () => {
	const store = getLiveActors();
	expect(store.followedId).toBeNull();

	store.follow('a1');
	expect(store.followedId).toBe('a1');

	store.unfollow();
	expect(store.followedId).toBeNull();
});

test('follow replaces the previously followed id', () => {
	const store = getLiveActors();
	store.follow('a1');
	store.follow('a2');
	expect(store.followedId).toBe('a2');
});

test('every follow() bumps the epoch, unfollow() does not', () => {
	const store = getLiveActors();
	const start = store.followEpoch;

	store.follow('a1');
	expect(store.followEpoch).toBe(start + 1);

	store.follow('a1');
	expect(store.followEpoch).toBe(start + 2);
	expect(store.followedId).toBe('a1');

	store.follow('a2');
	expect(store.followEpoch).toBe(start + 3);

	store.unfollow();
	expect(store.followEpoch).toBe(start + 3);
});

test('clear() also unfollows', () => {
	const store = getLiveActors();
	store.applyFrame(frame());
	store.follow('a1');

	store.clear();

	expect(store.followedId).toBeNull();
	expect(store.actors).toEqual([]);
});
