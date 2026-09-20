// @vitest-environment jsdom
import { render, screen } from '@testing-library/svelte';
import { tick } from 'svelte';
import { beforeEach, describe, expect, it, vi } from 'vitest';

import { installViewportStub } from '$utils/__tests__/fixtures/viewportStub';

const { appState, chrome } = vi.hoisted(() => ({
	appState: {
		saveFile: { name: 'Level.sav' } as { name: string } | undefined,
		settings: { debug_mode: false },
		hasGpsAvailable: false,
		saveState: vi.fn().mockResolvedValue(undefined)
	},
	chrome: { on: true }
}));

vi.mock('$lib/utils/platform', async (original) => ({
	...(await original<Record<string, unknown>>()),
	desktopChrome: () => chrome.on,
	isWebBuild: false
}));
vi.mock('$states', () => ({
	getAppState: () => appState,
	getModalState: () => ({}),
	getSignalState: () => ({ armed: false })
}));
vi.mock('$lib/signal/remoteMode.svelte', () => ({ getRemoteMode: () => ({ active: false }) }));
vi.mock('$lib/signal/webSession', () => ({ getWebSignalSession: () => ({ connected: false }) }));
vi.mock('$lib/components/layout/navActions.svelte', () => ({
	createNavActions: () => ({
		save: vi.fn(),
		eject: vi.fn(),
		openFolder: vi.fn(),
		settings: vi.fn()
	})
}));
vi.mock('$env/static/public', () => ({ PUBLIC_DESKTOP_MODE: 'true' }));
vi.mock('$app/state', () => ({ page: { url: new URL('http://localhost/overview') } }));

const { setViewport } = installViewportStub();
const { default: ShellNavHarness } = await import('./ShellNavHarness.svelte');

describe('shell navigation', () => {
	beforeEach(() => {
		vi.clearAllMocks();
		chrome.on = true;
		appState.saveFile = { name: 'Level.sav' };
	});

	it('offers a drawer trigger on a phone', async () => {
		setViewport(390);
		render(ShellNavHarness);
		await tick();

		expect(screen.getByRole('button', { name: /navigation/i })).not.toBeNull();
	});

	it('shows no drawer trigger at desktop width', async () => {
		setViewport(1440);
		render(ShellNavHarness);
		await tick();

		expect(screen.queryByRole('button', { name: /navigation/i })).toBeNull();
	});

	it('never covers a small viewport with a blocking alert', async () => {
		setViewport(320);
		render(ShellNavHarness);
		await tick();

		expect(screen.queryByRole('alert')).toBeNull();
	});
});
