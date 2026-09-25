import { beforeEach, describe, expect, it, vi } from 'vitest';

const remote = vi.hoisted(() => ({ active: false }));

vi.mock('$states', () => ({ getSignalState: () => ({ armed: false }) }));
vi.mock('$lib/signal/remoteMode.svelte', () => ({ getRemoteMode: () => remote }));
vi.mock('$lib/signal/webSession', () => ({ getWebSignalSession: () => ({ connected: false }) }));

import { navItems, type NavContext } from '../navItems';

const save = navItems.find((item) => item.id === 'save')!;

function ctx(desktop: boolean, saveFile: object | null = { name: 'Level.sav' }): NavContext {
	return {
		appState: { saveFile } as never,
		desktop,
		expanded: true,
		titleBar: false
	};
}

beforeEach(() => {
	remote.active = false;
});

describe('save nav item', () => {
	it('shows on the desktop build once a save is loaded', () => {
		expect(save.visible!(ctx(true))).toBe(true);
	});

	it('hides with no save loaded', () => {
		remote.active = true;
		expect(save.visible!(ctx(false, null))).toBe(false);
	});

	it('hides on the web build for a local save', () => {
		expect(save.visible!(ctx(false))).toBe(false);
	});

	it('shows on the web build for a save loaded remotely over Signal', () => {
		remote.active = true;
		expect(save.visible!(ctx(false))).toBe(true);
	});
});
