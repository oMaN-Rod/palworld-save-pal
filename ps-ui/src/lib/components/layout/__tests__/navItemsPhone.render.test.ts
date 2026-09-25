// @vitest-environment jsdom
import { beforeEach, describe, expect, it, vi } from 'vitest';

import { installViewportStub } from '$utils/__tests__/fixtures/viewportStub';

vi.mock('$states', () => ({ getSignalState: () => ({ armed: false }) }));
vi.mock('$lib/signal/remoteMode.svelte', () => ({ getRemoteMode: () => ({ active: false }) }));
vi.mock('$lib/signal/webSession', () => ({ getWebSignalSession: () => ({ connected: false }) }));

const { setViewport } = installViewportStub();
const { navItems } = await import('../navItems');
const { flushSync } = await import('svelte');

const ctx = {
	appState: { saveFile: undefined, settings: {}, hasGpsAvailable: false },
	desktop: true,
	expanded: true,
	titleBar: false
} as never;

function visible(id: string): boolean {
	const item = navItems.find((candidate) => candidate.id === id);
	return item?.visible?.(ctx) ?? true;
}

beforeEach(() => {
	setViewport(1440);
	flushSync();
});

describe('phone-only nav entries', () => {
	it.each(['editor', 'tools'])('shows %s on a desktop and a tablet', (id) => {
		expect(visible(id)).toBe(true);
		setViewport(1024);
		flushSync();
		expect(visible(id)).toBe(true);
	});

	it.each(['editor', 'tools'])('hides %s on a phone', (id) => {
		setViewport(390);
		flushSync();
		expect(visible(id)).toBe(false);
	});
});
