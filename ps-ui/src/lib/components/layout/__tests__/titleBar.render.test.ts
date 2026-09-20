// @vitest-environment jsdom
import { render, screen } from '@testing-library/svelte';
import { readFileSync } from 'node:fs';
import { resolve } from 'node:path';
import { beforeEach, describe, expect, it, vi } from 'vitest';

const { appState, platform, actions, github } = vi.hoisted(() => ({
	appState: { saveFile: undefined as { name: string; world_name?: string } | undefined },
	platform: { os: 'windows' as 'windows' | 'macos' | 'linux' | 'other' },
	actions: { save: vi.fn(), eject: vi.fn(), openFolder: vi.fn(), settings: vi.fn() },
	github: { stars: null as number | null }
}));

vi.mock('$lib/utils/platform', async (original) => ({
	...(await original<Record<string, unknown>>()),
	osFamily: () => platform.os,
	desktopChrome: () => true
}));
// navItems.ts reads signal and remote-mode state inside its badge and `visible`
// predicates, so both must be stubbed or resolving the nav items throws.
vi.mock('$states', () => ({
	getAppState: () => appState,
	getModalState: () => ({}),
	getSignalState: () => ({ armed: false })
}));
vi.mock('$lib/signal/remoteMode.svelte', () => ({ getRemoteMode: () => ({ active: false }) }));
vi.mock('$lib/signal/webSession', () => ({ getWebSignalSession: () => ({ connected: false }) }));
vi.mock('../navActions.svelte', () => ({ createNavActions: () => actions }));
// Keep the suite off the network.
vi.mock('$lib/utils/githubStars', async (original) => ({
	...(await original<Record<string, unknown>>()),
	fetchGithubStars: async () => github.stars
}));
vi.mock('$env/static/public', () => ({ PUBLIC_DESKTOP_MODE: 'true' }));

import { TITLE_BAR_ACTION_IDS } from '../navItems';
import TitleBar from '../TitleBar.svelte';

describe('TitleBar', () => {
	beforeEach(() => {
		vi.clearAllMocks();
		appState.saveFile = undefined;
		platform.os = 'windows';
		github.stars = null;
	});

	it('marks the root as a deep drag region', () => {
		const { container } = render(TitleBar);
		const root = container.querySelector('[data-tauri-drag-region]');

		expect(root?.getAttribute('data-tauri-drag-region')).toBe('deep');
	});

	it('renders window controls on Windows', () => {
		render(TitleBar);

		expect(screen.getByTestId('window-minimize')).toBeTruthy();
		expect(screen.getByTestId('window-maximize')).toBeTruthy();
		expect(screen.getByTestId('window-close')).toBeTruthy();
	});

	it('omits window controls on macOS, where the traffic lights are native', () => {
		platform.os = 'macos';
		render(TitleBar);

		expect(screen.queryByTestId('window-minimize')).toBeNull();
		expect(screen.queryByTestId('window-close')).toBeNull();
		expect(screen.getByTestId('traffic-light-spacer')).toBeTruthy();
	});

	it('hides the save-context actions when no save is loaded', () => {
		render(TitleBar);

		expect(screen.queryByTestId('title-action-save')).toBeNull();
		expect(screen.queryByTestId('title-action-eject')).toBeNull();
	});

	it('shows the save-context actions once a save is loaded', () => {
		appState.saveFile = { name: 'Level.sav', world_name: 'MyWorld' };
		render(TitleBar);

		expect(screen.getByTestId('title-action-save')).toBeTruthy();
		expect(screen.getByTestId('title-action-eject')).toBeTruthy();
	});

	it('always shows the app-level actions', () => {
		render(TitleBar);

		expect(screen.getByTestId('title-action-open-folder')).toBeTruthy();
		expect(screen.getByTestId('title-action-settings')).toBeTruthy();
	});

	it('renders exactly the title-bar-owned action set, and nothing else', () => {
		appState.saveFile = { name: 'Level.sav' };
		const { container } = render(TitleBar);

		const rendered = [...container.querySelectorAll('[data-testid^="title-action-"]')].map((el) =>
			el.getAttribute('data-testid')!.replace('title-action-', '')
		);

		// Guards the invariant the sidebar's exclusion filter relies on: anything
		// missing here has already been dropped from the sidebar and would be
		// unreachable in the desktop build.
		expect(rendered.sort()).toEqual([...TITLE_BAR_ACTION_IDS].sort());
	});

	it('links out to GitHub and Discord ahead of settings', () => {
		const { container } = render(TitleBar);

		const order = [...container.querySelectorAll('[data-testid^="title-"]')].map((el) =>
			el.getAttribute('data-testid')
		);

		expect(order.indexOf('title-link-github')).toBeGreaterThan(
			order.indexOf('title-action-open-folder')
		);
		expect(order.indexOf('title-link-discord')).toBeGreaterThan(order.indexOf('title-link-github'));
		expect(order.indexOf('title-action-settings')).toBeGreaterThan(
			order.indexOf('title-link-discord')
		);
	});

	it('opens the external links in a new tab', () => {
		render(TitleBar);

		const github = screen.getByTestId('title-link-github');
		const discord = screen.getByTestId('title-link-discord');

		expect(github.getAttribute('href')).toBe('https://github.com/oMaN-Rod/palstudio');
		expect(discord.getAttribute('href')).toBe('https://discord.gg/YWZFPy9G8J');
		for (const link of [github, discord]) {
			expect(link.getAttribute('target')).toBe('_blank');
			expect(link.getAttribute('rel')).toBe('noopener noreferrer');
		}
	});

	it('shows the GitHub star count once it resolves', async () => {
		github.stars = 1234;
		render(TitleBar);

		const link = await screen.findByTestId('title-link-github');
		await vi.waitFor(() => expect(link.textContent).toContain('1.2k'));
		expect(link.getAttribute('aria-label')).toBe('GitHub (★1.2k)');
	});

	it('falls back to the bare GitHub icon when the star count is unavailable', async () => {
		render(TitleBar);

		const link = await screen.findByTestId('title-link-github');
		expect(link.textContent?.trim()).toBe('');
		expect(link.getAttribute('aria-label')).toBe('GitHub');
	});

	it('renders every action as a button so the drag script excludes it', () => {
		appState.saveFile = { name: 'Level.sav' };
		const { container } = render(TitleBar);

		for (const el of container.querySelectorAll('[data-testid^="title-action-"]')) {
			expect(el.tagName).toBe('BUTTON');
		}
	});

	it('leaves the label empty when no save is loaded', () => {
		render(TitleBar);

		expect(screen.getByTestId('title-bar-label').textContent).toBe('');
	});

	it('shows the world and file name when a save is loaded', () => {
		appState.saveFile = { name: 'Level.sav', world_name: 'MyWorld' };
		render(TitleBar);

		const label = screen.getByTestId('title-bar-label').textContent ?? '';
		expect(label).toContain('MyWorld');
		expect(label).toContain('Level.sav');
	});
});

// jsdom computes no layout and paints nothing, so the stacking order that keeps
// the window controls reachable cannot be observed from a rendered tree. These
// assert it at the source instead: a `z-index` on the shell wrapper traps the
// bar in a stacking context below the modal overlays, and the window becomes
// undraggable and unclosable for as long as any modal is open — including the
// Settings and Open Folder modals this bar itself opens.
describe('title bar stacking', () => {
	const appCss = readFileSync(resolve(import.meta.dirname, '../../../../app.css'), 'utf8');
	const layout = readFileSync(
		resolve(import.meta.dirname, '../../../../routes/+layout.svelte'),
		'utf8'
	);

	const MODAL_OVERLAY_Z = 50000;
	const RESIZE_WARNING_Z = 99999;

	it('paints above the modal overlays and below the resize warning', () => {
		const rule = appCss.match(/^\.title-bar \{([\s\S]*?)^\}/m)?.[1];
		const z = Number(rule?.match(/^\s*z-index:\s*(\d+);/m)?.[1]);

		expect(rule).toMatch(/^\s*position:\s*relative;/m);
		expect(z).toBeGreaterThan(MODAL_OVERLAY_Z);
		expect(z).toBeLessThan(RESIZE_WARNING_Z);
	});

	it('is not wrapped in a stacking context that would pin it below them', () => {
		const wrapper = layout.match(/<div class="([^"]*)">\s*\{#if showTitleBar\}/)?.[1];

		expect(wrapper).toBeDefined();
		expect(wrapper).not.toMatch(/(^|\s)z-/);
	});
});
