import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import {
	clearStoredDesktop,
	deviceLabel,
	loadStoredDesktop,
	saveStoredDesktop,
	type StoredDesktop
} from './credentialStore';

const SAMPLE: StoredDesktop = {
	meetRoom: 'a'.repeat(32),
	deviceId: 'b'.repeat(32),
	deviceSecret: 'c'.repeat(64),
	desktopName: 'Study PC',
	pairedAtMs: 1_700_000_000_000
};

function createFakeStorage() {
	const map = new Map<string, string>();
	return {
		getItem: vi.fn((key: string) => (map.has(key) ? map.get(key)! : null)),
		setItem: vi.fn((key: string, value: string) => {
			map.set(key, value);
		}),
		removeItem: vi.fn((key: string) => {
			map.delete(key);
		})
	};
}

describe('credentialStore', () => {
	beforeEach(() => {
		Object.defineProperty(globalThis, 'localStorage', {
			value: createFakeStorage(),
			configurable: true,
			writable: true
		});
	});

	afterEach(() => {
		delete (globalThis as { localStorage?: unknown }).localStorage;
	});

	it('round-trips a stored desktop through save and load', () => {
		saveStoredDesktop(SAMPLE);
		expect(loadStoredDesktop()).toEqual(SAMPLE);
	});

	it('returns null when nothing is stored', () => {
		expect(loadStoredDesktop()).toBeNull();
	});

	it('clear removes the stored desktop', () => {
		saveStoredDesktop(SAMPLE);
		clearStoredDesktop();
		expect(loadStoredDesktop()).toBeNull();
	});

	it('returns null and removes a corrupt (non-JSON) entry', () => {
		localStorage.setItem('psp-signal-device', 'not json{{{');

		expect(loadStoredDesktop()).toBeNull();
		expect(localStorage.getItem('psp-signal-device')).toBeNull();
	});

	it('returns null and removes an entry missing a required field', () => {
		localStorage.setItem('psp-signal-device', JSON.stringify({ ...SAMPLE, deviceId: undefined }));

		expect(loadStoredDesktop()).toBeNull();
		expect(localStorage.getItem('psp-signal-device')).toBeNull();
	});

	it('stores under the psp-signal-device key', () => {
		saveStoredDesktop(SAMPLE);
		expect(localStorage.getItem('psp-signal-device')).toBe(JSON.stringify(SAMPLE));
	});

	describe('with a throwing localStorage', () => {
		beforeEach(() => {
			Object.defineProperty(globalThis, 'localStorage', {
				value: {
					getItem: vi.fn(() => {
						throw new DOMException('blocked', 'SecurityError');
					}),
					setItem: vi.fn(() => {
						throw new DOMException('quota exceeded', 'QuotaExceededError');
					}),
					removeItem: vi.fn(() => {
						throw new DOMException('blocked', 'SecurityError');
					})
				},
				configurable: true,
				writable: true
			});
		});

		it('loadStoredDesktop never throws', () => {
			expect(() => loadStoredDesktop()).not.toThrow();
			expect(loadStoredDesktop()).toBeNull();
		});

		it('saveStoredDesktop never throws', () => {
			expect(() => saveStoredDesktop(SAMPLE)).not.toThrow();
		});

		it('clearStoredDesktop never throws', () => {
			expect(() => clearStoredDesktop()).not.toThrow();
		});
	});

	it('never throws when localStorage is entirely absent', () => {
		delete (globalThis as { localStorage?: unknown }).localStorage;

		expect(() => loadStoredDesktop()).not.toThrow();
		expect(loadStoredDesktop()).toBeNull();
		expect(() => saveStoredDesktop(SAMPLE)).not.toThrow();
		expect(() => clearStoredDesktop()).not.toThrow();
	});
});

describe('deviceLabel', () => {
	afterEach(() => {
		vi.unstubAllGlobals();
	});

	function stubUserAgent(userAgent: string) {
		vi.stubGlobal('navigator', { userAgent });
	}

	it('returns a non-empty "<Browser> on <OS>" label for Chrome on Windows', () => {
		stubUserAgent(
			'Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/128.0.0.0 Safari/537.36'
		);

		expect(deviceLabel()).toBe('Chrome on Windows');
	});

	it('returns a non-empty label for Safari on iPhone', () => {
		stubUserAgent(
			'Mozilla/5.0 (iPhone; CPU iPhone OS 17_5 like Mac OS X) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/17.5 Mobile/15E148 Safari/604.1'
		);

		expect(deviceLabel()).toBe('Safari on iPhone');
	});

	it('returns a non-empty label for Firefox on Linux', () => {
		stubUserAgent('Mozilla/5.0 (X11; Linux x86_64; rv:128.0) Gecko/20100101 Firefox/128.0');

		expect(deviceLabel()).toBe('Firefox on Linux');
	});

	it('falls back to "This browser" for an unrecognized user agent', () => {
		stubUserAgent('SomeUnknownBot/1.0');

		expect(deviceLabel()).toBe('This browser');
	});

	it('falls back to "This browser" when navigator is unavailable', () => {
		vi.stubGlobal('navigator', undefined);

		expect(deviceLabel()).toBe('This browser');
	});

	it('never returns an empty string', () => {
		stubUserAgent('');

		expect(deviceLabel().length).toBeGreaterThan(0);
	});
});

const sessionMocks = vi.hoisted(() => ({ createSignalSession: vi.fn() }));
vi.mock('./session.svelte', () => ({ createSignalSession: sessionMocks.createSignalSession }));
vi.mock('$lib/data/liveActors.svelte', () => ({
	getLiveActors: () => ({ applyFrame: vi.fn() })
}));

describe('webSession credential bridge', () => {
	type CtlListener = (msg: { type: string; data: unknown }) => void;

	function createFakeSession() {
		const subscribers = new Set<CtlListener>();
		return {
			onFrame: vi.fn(() => () => {}),
			send: vi.fn(),
			message: {
				subscribe: (fn: CtlListener) => {
					subscribers.add(fn);
					return () => subscribers.delete(fn);
				}
			},
			deliver(msg: { type: string; data: unknown }) {
				for (const fn of subscribers) fn(msg);
			}
		};
	}

	let fakeSession: ReturnType<typeof createFakeSession>;

	beforeEach(() => {
		vi.resetModules();
		fakeSession = createFakeSession();
		sessionMocks.createSignalSession.mockReturnValue(fakeSession);
		Object.defineProperty(globalThis, 'localStorage', {
			value: createFakeStorage(),
			configurable: true,
			writable: true
		});
		vi.stubGlobal('navigator', {
			userAgent: 'Mozilla/5.0 (Windows NT 10.0; Win64; x64) Chrome/128.0.0.0 Safari/537.36'
		});
	});

	afterEach(() => {
		delete (globalThis as { localStorage?: unknown }).localStorage;
		vi.unstubAllGlobals();
	});

	it('saves and acks a valid device_credential message', async () => {
		const { getWebSignalSession } = await import('./webSession');
		getWebSignalSession();

		fakeSession.deliver({
			type: 'device_credential',
			data: {
				deviceId: 'device-1',
				deviceSecret: 'secret-1',
				meetRoom: 'room-1',
				desktopName: 'Living Room PC'
			}
		});

		expect(loadStoredDesktop()).toMatchObject({
			deviceId: 'device-1',
			deviceSecret: 'secret-1',
			meetRoom: 'room-1',
			desktopName: 'Living Room PC'
		});
		expect(fakeSession.send).toHaveBeenCalledWith('device_credential', {
			name: 'Chrome on Windows'
		});
	});

	it('ignores a device_credential message missing a required field', async () => {
		const { getWebSignalSession } = await import('./webSession');
		getWebSignalSession();

		fakeSession.deliver({
			type: 'device_credential',
			data: { deviceId: 'device-1', deviceSecret: 'secret-1', meetRoom: 'room-1' }
		});

		expect(loadStoredDesktop()).toBeNull();
		expect(fakeSession.send).not.toHaveBeenCalled();
	});

	it('ignores unrelated ctl messages', async () => {
		const { getWebSignalSession } = await import('./webSession');
		getWebSignalSession();

		fakeSession.deliver({ type: 'source-status', data: { fps: 30 } });

		expect(loadStoredDesktop()).toBeNull();
		expect(fakeSession.send).not.toHaveBeenCalled();
	});

	it('saves the credential and swallows a thrown ack when the ctl channel is closed', async () => {
		fakeSession.send.mockImplementation(() => {
			throw new Error('SignalSession: cannot send before the ctl channel is open');
		});
		const { getWebSignalSession } = await import('./webSession');
		getWebSignalSession();

		expect(() =>
			fakeSession.deliver({
				type: 'device_credential',
				data: {
					deviceId: 'device-1',
					deviceSecret: 'secret-1',
					meetRoom: 'room-1',
					desktopName: 'Living Room PC'
				}
			})
		).not.toThrow();

		expect(loadStoredDesktop()).toMatchObject({ deviceId: 'device-1' });
		expect(fakeSession.send).toHaveBeenCalledWith('device_credential', {
			name: 'Chrome on Windows'
		});
	});
});
