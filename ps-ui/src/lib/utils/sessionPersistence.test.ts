import { beforeEach, describe, expect, it, vi } from 'vitest';
import {
	clearSessionPersistence,
	consumeReattachPending,
	getStoredSelectedPlayerUid,
	getStoredSessionId,
	markReattachPending,
	setStoredSelectedPlayerUid,
	setStoredSessionId
} from './sessionPersistence';

// The vitest env here is plain node (no jsdom), which has no sessionStorage
// global — stub a minimal in-memory Storage for each test.
class MemoryStorage {
	private store = new Map<string, string>();
	getItem(key: string) {
		return this.store.has(key) ? this.store.get(key)! : null;
	}
	setItem(key: string, value: string) {
		this.store.set(key, value);
	}
	removeItem(key: string) {
		this.store.delete(key);
	}
}

beforeEach(() => {
	vi.stubGlobal('sessionStorage', new MemoryStorage());
});

describe('session id storage', () => {
	it('round-trips the stored session id', () => {
		expect(getStoredSessionId()).toBeNull();
		setStoredSessionId('session-123');
		expect(getStoredSessionId()).toBe('session-123');
	});
});

describe('selected player uid storage', () => {
	it('round-trips the stored selected player uid', () => {
		expect(getStoredSelectedPlayerUid()).toBeNull();
		setStoredSelectedPlayerUid('player-abc');
		expect(getStoredSelectedPlayerUid()).toBe('player-abc');
	});
});

describe('clearSessionPersistence', () => {
	it('removes both the session id and selected player uid', () => {
		setStoredSessionId('session-123');
		setStoredSelectedPlayerUid('player-abc');
		clearSessionPersistence();
		expect(getStoredSessionId()).toBeNull();
		expect(getStoredSelectedPlayerUid()).toBeNull();
	});
});

describe('reattach-pending flag', () => {
	it('consumes once: true on first read after marking, false after', () => {
		expect(consumeReattachPending()).toBe(false);
		markReattachPending();
		expect(consumeReattachPending()).toBe(true);
		expect(consumeReattachPending()).toBe(false);
	});
});
