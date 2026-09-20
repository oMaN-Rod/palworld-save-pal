// @vitest-environment jsdom
import '$utils/__tests__/fixtures/animatePolyfill';
import { render, screen } from '@testing-library/svelte';
import userEvent from '@testing-library/user-event';
import { tick } from 'svelte';
import { beforeEach, describe, expect, it, vi } from 'vitest';

const { appState, chrome, navActionMocks } = vi.hoisted(() => ({
	appState: {
		saveFile: { name: 'Level.sav' } as { name: string } | undefined,
		settings: { debug_mode: false },
		hasGpsAvailable: false,
		saveState: vi.fn().mockResolvedValue(undefined)
	},
	chrome: { on: true },
	navActionMocks: {
		save: vi.fn(),
		eject: vi.fn().mockResolvedValue(undefined),
		openFolder: vi.fn().mockResolvedValue(undefined),
		settings: vi.fn().mockResolvedValue(undefined)
	}
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
vi.mock('../navActions.svelte', () => ({
	createNavActions: () => navActionMocks
}));
vi.mock('$env/static/public', () => ({ PUBLIC_DESKTOP_MODE: 'true' }));
vi.mock('$app/state', () => ({ page: { url: new URL('http://localhost/overview') } }));

import NavDrawer from '../NavDrawer.svelte';
import { navItems, type NavContext } from '../navItems';

describe('NavDrawer', () => {
	beforeEach(() => {
		vi.clearAllMocks();
		chrome.on = true;
		appState.saveFile = { name: 'Level.sav' };
	});

	it('renders nothing while closed', () => {
		render(NavDrawer, { open: false, onClose: vi.fn() });
		expect(screen.queryByRole('dialog')).toBeNull();
	});

	it('exposes a labelled navigation dialog when open', async () => {
		render(NavDrawer, { open: true, onClose: vi.fn() });
		await tick();

		const dialog = screen.getByRole('dialog');
		expect(dialog.getAttribute('aria-modal')).toBe('true');
		expect(dialog.getAttribute('aria-label')).toBeTruthy();
	});

	it('closes on Escape', async () => {
		const onClose = vi.fn();
		render(NavDrawer, { open: true, onClose });
		await tick();

		await userEvent.keyboard('{Escape}');

		expect(onClose).toHaveBeenCalledTimes(1);
	});

	it('closes when the backdrop is clicked', async () => {
		const onClose = vi.fn();
		render(NavDrawer, { open: true, onClose });
		await tick();

		await userEvent.click(screen.getByTestId('nav-drawer-backdrop'));

		expect(onClose).toHaveBeenCalledTimes(1);
	});

	it('offers an explicit close control', async () => {
		const onClose = vi.fn();
		render(NavDrawer, { open: true, onClose });
		await tick();

		await userEvent.click(screen.getByRole('button', { name: /close/i }));

		expect(onClose).toHaveBeenCalledTimes(1);
	});

	it('moves focus into the drawer when it opens', async () => {
		render(NavDrawer, { open: true, onClose: vi.fn() });
		await tick();

		expect(screen.getByRole('dialog').contains(document.activeElement)).toBe(true);
	});

	it('closes the drawer when a nav link is activated', async () => {
		const onClose = vi.fn();
		render(NavDrawer, { open: true, onClose });
		await tick();

		await userEvent.click(screen.getByRole('link', { name: 'Edit' }));

		expect(onClose).toHaveBeenCalledTimes(1);
	});

	it('renders every visible tile from navItems, tracking the source of truth', async () => {
		render(NavDrawer, { open: true, onClose: vi.fn() });
		await tick();

		const ctx: NavContext = {
			appState: appState as unknown as NavContext['appState'],
			desktop: true,
			expanded: true,
			titleBar: chrome.on
		};
		const expectedLabels = navItems
			.filter((item) => item.section === 'tiles' && (item.visible?.(ctx) ?? true))
			.map((item) => item.label?.())
			.filter((label): label is string => Boolean(label))
			.sort();

		const renderedLabels = screen
			.getAllByRole('link')
			.map((link) => link.textContent?.trim())
			.filter((label): label is string => Boolean(label))
			.sort();

		expect(renderedLabels).toEqual(expectedLabels);
	});

	it('exposes settings and eject as reachable actions when there is no title bar', async () => {
		chrome.on = false;
		appState.saveFile = { name: 'Level.sav' };
		render(NavDrawer, { open: true, onClose: vi.fn() });
		await tick();

		const settingsButton = screen.getByTestId('nav-action-settings');
		const ejectButton = screen.getByTestId('nav-action-eject');

		await userEvent.click(settingsButton);
		expect(navActionMocks.settings).toHaveBeenCalledTimes(1);

		await userEvent.click(ejectButton);
		expect(navActionMocks.eject).toHaveBeenCalledTimes(1);
	});

	it('hides eject but keeps settings reachable when no save is loaded', async () => {
		chrome.on = false;
		appState.saveFile = undefined;
		render(NavDrawer, { open: true, onClose: vi.fn() });
		await tick();

		expect(screen.queryByTestId('nav-action-eject')).toBeNull();
		expect(screen.getByTestId('nav-action-settings')).toBeTruthy();
	});
});
