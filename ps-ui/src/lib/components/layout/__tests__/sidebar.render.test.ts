// @vitest-environment jsdom
import { render, screen } from '@testing-library/svelte';
import { beforeEach, describe, expect, it, vi } from 'vitest';

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
// navItems.ts reads signal and remote-mode state inside its badge and `visible`
// predicates, so both must be stubbed or rendering the nav throws.
vi.mock('$states', () => ({
	getAppState: () => appState,
	getModalState: () => ({}),
	getSignalState: () => ({ armed: false })
}));
vi.mock('$lib/signal/remoteMode.svelte', () => ({ getRemoteMode: () => ({ active: false }) }));
vi.mock('$lib/signal/webSession', () => ({ getWebSignalSession: () => ({ connected: false }) }));
vi.mock('../navActions.svelte', () => ({
	createNavActions: () => ({
		save: vi.fn(),
		eject: vi.fn(),
		openFolder: vi.fn(),
		settings: vi.fn()
	})
}));
vi.mock('$env/static/public', () => ({ PUBLIC_DESKTOP_MODE: 'true' }));
vi.mock('$app/state', () => ({ page: { url: new URL('http://localhost/overview') } }));

import Sidebar from '../Sidebar.svelte';

const ACTION_IDS = ['save', 'eject', 'open-folder', 'settings'];

describe('Sidebar', () => {
	beforeEach(() => {
		vi.clearAllMocks();
		chrome.on = true;
		appState.saveFile = { name: 'Level.sav' };
	});

	it('drops the relocated actions when the title bar owns them', () => {
		const { container } = render(Sidebar);

		for (const id of ACTION_IDS) {
			expect(container.querySelector(`[data-testid="nav-action-${id}"]`), id).toBeNull();
		}
	});

	it('keeps eject and settings when there is no title bar', () => {
		chrome.on = false;
		const { container } = render(Sidebar);

		expect(container.querySelector('[data-testid="nav-action-eject"]')).toBeTruthy();
		expect(container.querySelector('[data-testid="nav-action-settings"]')).toBeTruthy();
	});

	it('no longer renders the header block', () => {
		const { container } = render(Sidebar);

		expect(container.querySelector('.sidebar-header')).toBeNull();
	});

	it('still offers the collapse toggle', () => {
		render(Sidebar);

		expect(screen.getByTestId('nav-action-menu')).toBeTruthy();
	});

	it('labels the collapse toggle instead of rendering a blank row', () => {
		render(Sidebar);
		const toggle = screen.getByTestId('nav-action-menu');

		// Without a `label` the shared action-button snippet still emits the
		// full-width label span, so the row reads as a chevron beside empty text.
		expect(toggle.querySelector('.sidebar-label')?.textContent?.trim()).toBeTruthy();
	});

	it('gives the collapse toggle a complete accessible name', () => {
		render(Sidebar);
		const name = screen.getByTestId('nav-action-menu').getAttribute('aria-label') ?? '';

		// An empty `entity` leaves the dangling fragment "Toggle " behind.
		expect(name.trim()).toBe(name);
		expect(name.split(/\s+/).length).toBeGreaterThan(1);
	});
});
