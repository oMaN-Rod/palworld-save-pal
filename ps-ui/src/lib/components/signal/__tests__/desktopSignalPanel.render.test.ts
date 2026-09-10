// @vitest-environment jsdom
import type { SignalStatusJson } from '$types';
import { MessageType } from '$types';
import { render, screen, within } from '@testing-library/svelte';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import '../../pal/__tests__/fixtures/matchMediaPolyfill';

const goto = vi.fn();
vi.mock('$app/navigation', () => ({ goto: (...args: unknown[]) => goto(...args) }));

let status: SignalStatusJson = {
	source: { kind: 'rest', health: 'ok', actorCount: 42 },
	pairing: 'off',
	armed: true
};

let devices: {
	deviceId: string;
	name: string;
	createdAtMs: number;
	lastSeenMs?: number;
	connected: boolean;
}[] = [];

const sendAndWait = vi.fn(async (type: string) => {
	switch (type) {
		case MessageType.SIGNAL_STATUS:
			return status;
		case MessageType.SIGNAL_LIST_DEVICES:
			return { devices };
		default:
			return {};
	}
});

vi.mock('$utils/websocketUtils', () => ({
	sendAndWait: (type: string) => sendAndWait(type),
	send: vi.fn(),
	sendBytes: vi.fn(),
	isReady: () => true,
	pushProgressMessage: vi.fn()
}));

import { getSignalState } from '$states';
import DesktopSignalPanel from '../DesktopSignalPanel.svelte';

function device(
	deviceId: string,
	overrides: Partial<{ name: string; connected: boolean; lastSeenMs: number }> = {}
) {
	return {
		deviceId,
		name: overrides.name ?? deviceId,
		createdAtMs: Date.now() - 86_400_000,
		lastSeenMs: overrides.lastSeenMs ?? Date.now(),
		connected: overrides.connected ?? false
	};
}

beforeEach(() => {
	sendAndWait.mockClear();
	goto.mockClear();
	status = { source: { kind: 'rest', health: 'ok', actorCount: 42 }, pairing: 'off', armed: true };
	devices = [];
	const signalState = getSignalState();
	signalState.status = null;
	signalState.devices = [];
	signalState.url = null;
});

async function chips(): Promise<HTMLElement> {
	return await vi.waitFor(() => {
		const found = document.querySelector('#signal-status-chips');
		expect(found).toBeTruthy();
		return found as HTMLElement;
	});
}

describe('DesktopSignalPanel status chips', () => {
	it('reads source, health, actor count and armed state out of the header', async () => {
		render(DesktopSignalPanel);

		const bar = await chips();
		expect(within(bar).getByText('Managed server')).toBeTruthy();
		expect(within(bar).getByText('Live')).toBeTruthy();
		expect(within(bar).getByText(/42/)).toBeTruthy();
		expect(within(bar).getByText('Armed')).toBeTruthy();
	});

	it('marks a disarmed desktop apart from an accepting one', async () => {
		status = { ...status, armed: false };
		render(DesktopSignalPanel);

		const bar = await chips();
		expect(within(bar).getByText('Not armed')).toBeTruthy();
		expect(within(bar).queryByText('Armed')).toBeNull();
	});

	it('names an unhealthy source rather than only colouring it', async () => {
		status = { ...status, source: { kind: 'rest', health: 'down', actorCount: 0 } };
		render(DesktopSignalPanel);

		expect(within(await chips()).getByText('Unreachable')).toBeTruthy();
	});
});

describe('DesktopSignalPanel explainer', () => {
	it('links to the guide instead of restating it', async () => {
		render(DesktopSignalPanel);
		await chips();

		const link = screen.getByRole('link', { name: /how signal works/i });
		expect(link.getAttribute('href')).toBe('/docs/guides/remote-access');
	});
});

describe('DesktopSignalPanel body by state', () => {
	it('leads with pairing when no device has ever been paired', async () => {
		render(DesktopSignalPanel);
		await chips();

		expect(await screen.findByRole('button', { name: 'Start pairing' })).toBeTruthy();
		expect(document.querySelector('#signal-device-table')).toBeNull();
	});

	it('leads with the device table once devices exist', async () => {
		devices = [
			device('d1', { name: "Omar's Pixel", connected: true }),
			device('d2', { name: 'iPad' })
		];
		render(DesktopSignalPanel);

		const table = await vi.waitFor(() => {
			const found = document.querySelector('#signal-device-table');
			expect(found).toBeTruthy();
			return found as HTMLElement;
		});
		expect(within(table).getByText("Omar's Pixel")).toBeTruthy();
		expect(within(table).getByText('iPad')).toBeTruthy();
	});

	it('shows the pairing code while waiting, devices or not', async () => {
		devices = [device('d1', { name: 'iPad' })];
		status = { ...status, pairing: 'waiting', code: 'K7M-2QP', expiresAtMs: Date.now() + 300_000 };
		render(DesktopSignalPanel);

		expect(await screen.findByText('K7M-2QP')).toBeTruthy();
	});
});
