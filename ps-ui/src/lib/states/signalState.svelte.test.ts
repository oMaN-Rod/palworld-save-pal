import { beforeEach, describe, expect, it, vi } from 'vitest';

const sendAndWait = vi.fn();

vi.mock('$lib/utils/websocketUtils', () => ({
	sendAndWait: (type: unknown, data?: unknown) => sendAndWait(type, data),
	send: vi.fn()
}));

import type { SignalDeviceJson, SignalDeviceListJson, SignalStatusJson } from '$types';
import { SignalState } from './signalState.svelte';

function statusJson(armed: boolean): SignalStatusJson {
	return {
		source: { health: 'idle', actorCount: 0 },
		pairing: 'off',
		armed
	};
}

function device(deviceId: string, name = 'Device'): SignalDeviceJson {
	return { deviceId, name, createdAtMs: 0, lastSeenMs: 0, connected: true };
}

function deferred<T>() {
	let resolve!: (value: T) => void;
	const promise = new Promise<T>((r) => {
		resolve = r;
	});
	return { promise, resolve };
}

beforeEach(() => {
	sendAndWait.mockReset();
});

describe('SignalState device list guard', () => {
	it('keeps a device list that was asked for before a status poll', async () => {
		const signalState = new SignalState();
		const list = deferred<SignalDeviceListJson>();
		const poll = deferred<SignalStatusJson>();

		sendAndWait.mockImplementationOnce(() => list.promise);
		const listPromise = signalState.listDevices();

		sendAndWait.mockImplementationOnce(() => poll.promise);
		const refreshPromise = signalState.refresh();

		poll.resolve(statusJson(true));
		await refreshPromise;

		list.resolve({ devices: [device('dev-1')] });
		await listPromise;

		expect(signalState.devices.map((d) => d.deviceId)).toEqual(['dev-1']);
	});

	it('discards a device list reply superseded by a newer device request', async () => {
		const signalState = new SignalState();
		const first = deferred<SignalDeviceListJson>();
		const second = deferred<SignalDeviceListJson>();

		sendAndWait.mockImplementationOnce(() => first.promise);
		const firstPromise = signalState.listDevices();

		sendAndWait.mockImplementationOnce(() => second.promise);
		const secondPromise = signalState.listDevices();

		second.resolve({ devices: [device('dev-2')] });
		await secondPromise;

		first.resolve({ devices: [device('dev-1')] });
		await firstPromise;

		expect(signalState.devices.map((d) => d.deviceId)).toEqual(['dev-2']);
	});

	it('keeps a rename result that a status poll overtook', async () => {
		const signalState = new SignalState();
		const rename = deferred<SignalDeviceListJson>();
		const poll = deferred<SignalStatusJson>();

		sendAndWait.mockImplementationOnce(() => rename.promise);
		const renamePromise = signalState.renameDevice('dev-1', 'Phone');

		sendAndWait.mockImplementationOnce(() => poll.promise);
		const refreshPromise = signalState.refresh();

		poll.resolve(statusJson(true));
		await refreshPromise;

		rename.resolve({ devices: [device('dev-1', 'Phone')] });
		await renamePromise;

		expect(signalState.devices.map((d) => d.name)).toEqual(['Phone']);
	});
});

describe('SignalState generation guard', () => {
	it('discards a status poll reply that arrives after a newer mutation already landed', async () => {
		const signalState = new SignalState();
		const poll = deferred<SignalStatusJson>();
		const arm = deferred<SignalStatusJson>();

		sendAndWait.mockImplementationOnce(() => poll.promise);
		const refreshPromise = signalState.refresh();

		sendAndWait.mockImplementationOnce(() => arm.promise);
		const armPromise = signalState.setArmed(true);

		arm.resolve(statusJson(true));
		await armPromise;
		expect(signalState.armed).toBe(true);

		poll.resolve(statusJson(false));
		await refreshPromise;

		expect(signalState.armed).toBe(true);
	});

	it('discards a stale mutation reply that arrives after a newer mutation already landed', async () => {
		const signalState = new SignalState();
		const firstArm = deferred<SignalStatusJson>();
		const secondArm = deferred<SignalStatusJson>();

		sendAndWait.mockImplementationOnce(() => firstArm.promise);
		const firstPromise = signalState.setArmed(true);

		sendAndWait.mockImplementationOnce(() => secondArm.promise);
		const secondPromise = signalState.setArmed(false);

		secondArm.resolve(statusJson(false));
		await secondPromise;
		expect(signalState.armed).toBe(false);

		firstArm.resolve(statusJson(true));
		await firstPromise;

		expect(signalState.armed).toBe(false);
	});

	it('still applies a reply when no newer request has been issued since', async () => {
		const signalState = new SignalState();
		sendAndWait.mockResolvedValueOnce(statusJson(true));

		await signalState.refresh();

		expect(signalState.armed).toBe(true);
	});
});
