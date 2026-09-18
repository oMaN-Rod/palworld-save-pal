// @vitest-environment jsdom
import type { ModTarget } from '$types';
import { render, screen } from '@testing-library/svelte';
import { tick } from 'svelte';
import { beforeEach, describe, expect, it, vi } from 'vitest';

const {
	holder,
	send,
	goto,
	page,
	platform,
	remote,
	remoteMock,
	servers,
	nexus,
	nexusResets,
	modal,
	env,
	listProps,
	panelProps
} = vi.hoisted(() => {
	const remote = { active: false };
	/** Mirrors nexusState's own `#lastResets` field: a plain module-level value, so it survives
	 *  the route unmounting the same way the real singleton store does. */
	const nexusResets = { last: undefined as number | undefined };
	return {
		holder: { state: undefined as unknown },
		send: vi.fn(),
		goto: vi.fn(),
		page: { url: new URL('http://localhost/mods') },
		platform: { web: false },
		remote,
		remoteMock: vi.fn(() => remote),
		servers: { servers: [], loadServers: vi.fn() },
		nexus: {
			forgetInFlight: vi.fn(),
			reset: vi.fn(),
			subscribeLinks: vi.fn(),
			links: [] as unknown[],
			dismissLink: vi.fn(),
			shouldForget: (resets: number) => {
				const changed = nexusResets.last !== undefined && resets !== nexusResets.last;
				nexusResets.last = resets;
				return changed;
			}
		},
		nexusResets,
		modal: { showModal: vi.fn() },
		env: { desktop: 'true' },
		listProps: [] as Record<string, unknown>[],
		panelProps: [] as Record<string, unknown>[]
	};
});

vi.mock('$env/static/public', () => ({
	get PUBLIC_DESKTOP_MODE() {
		return env.desktop;
	}
}));

vi.mock('$lib/utils/websocketUtils', () => ({
	send: (type: unknown, data?: unknown) => send(type, data),
	sendAndWait: vi.fn(),
	sendBytes: vi.fn(),
	isReady: () => true,
	pushProgressMessage: vi.fn()
}));

vi.mock('$app/navigation', () => ({ goto: (...args: unknown[]) => goto(...args) }));
vi.mock('$app/state', () => ({ page }));
vi.mock('$lib/signal/remoteMode.svelte', async (importOriginal) => {
	const actual = await importOriginal<typeof import('$lib/signal/remoteMode.svelte')>();
	return { ...actual, getRemoteMode: remoteMock };
});
vi.mock('$lib/utils/platform', () => ({
	get isWebBuild() {
		return platform.web;
	}
}));

vi.mock('$components/mods', () => ({
	TargetList: (_anchor: unknown, props: Record<string, unknown>) => listProps.push(props),
	TargetPanel: (_anchor: unknown, props: Record<string, unknown>) => panelProps.push(props)
}));

vi.mock('$states', async () => {
	const { ModsState } = await import('$lib/states/modsState.svelte');
	holder.state = new ModsState();
	return {
		getModsState: () => holder.state,
		getServerState: () => servers,
		getNexusState: () => nexus,
		getModalState: () => modal
	};
});

import { modTarget } from '$lib/components/mods/__tests__/fixtures';
import { RemoteModeState } from '$lib/signal/remoteMode.svelte';
import type { ModsState } from '$lib/states/modsState.svelte';
import { modTargetListHandler, modTargetRemoveHandler } from '$lib/ws/handlers/modsHandler';
import ModsPage from './+page.svelte';

const state = () => holder.state as ModsState;

function target(id: string): ModTarget {
	return modTarget({ id, name: id, root_path: `C:/${id}` });
}

function sentTypes(): unknown[] {
	return send.mock.calls.map(([type]) => type);
}

describe('/mods after a reconnect', () => {
	it('loads targets, the library and servers again after a drop during an install', async () => {
		state().connectionChanged(true);
		render(ModsPage);
		await modTargetListHandler.handle({ targets: [target('client-a')] }, {
			goto: vi.fn()
		} as never);
		state().install('client-a', 'C:/a.zip');
		state().connectionChanged(false);
		send.mockReset();

		state().connectionChanged(true);
		await tick();

		expect(sentTypes()).toEqual(['mod_target_list', 'mod_list', 'mod_verification_subscribe']);
		expect(servers.loadServers).toHaveBeenCalledTimes(2);
	});

	it('subscribes to nxm links again after a drop and reconnect', async () => {
		state().connectionChanged(true);
		render(ModsPage);
		expect(nexus.subscribeLinks).toHaveBeenCalledTimes(1);

		state().connectionChanged(false);
		nexus.subscribeLinks.mockClear();

		state().connectionChanged(true);
		await tick();

		expect(nexus.subscribeLinks).toHaveBeenCalledTimes(1);
	});
});

beforeEach(() => {
	send.mockReset();
	goto.mockReset();
	servers.loadServers.mockReset();
	nexus.forgetInFlight.mockReset();
	nexus.reset.mockReset();
	nexus.subscribeLinks.mockReset();
	nexus.dismissLink.mockReset();
	nexus.links = [];
	nexusResets.last = undefined;
	modal.showModal.mockReset();
	listProps.length = 0;
	panelProps.length = 0;
	page.url = new URL('http://localhost/mods');
	platform.web = false;
	remote.active = false;
	env.desktop = 'true';
	state().reset();
});

describe('/mods', () => {
	it('explains mods are unavailable in the web build outside a remote session', () => {
		platform.web = true;
		render(ModsPage);

		expect(screen.getByText("Mods aren't available in the browser")).toBeTruthy();
		expect(send).not.toHaveBeenCalled();
		expect(servers.loadServers).not.toHaveBeenCalled();
		expect(listProps).toHaveLength(0);
	});

	it('loads targets, the library and servers in a remote session from the web build', () => {
		platform.web = true;
		remote.active = true;
		render(ModsPage);

		expect(sentTypes()).toEqual(['mod_target_list', 'mod_list', 'mod_verification_subscribe']);
		expect(servers.loadServers).toHaveBeenCalledTimes(1);
		expect(listProps).toHaveLength(1);
	});

	it('loads everything again after the store is reset', async () => {
		render(ModsPage);
		send.mockReset();

		state().reset();
		await tick();

		expect(sentTypes()).toEqual(['mod_target_list', 'mod_list', 'mod_verification_subscribe']);
	});

	it('subscribes to mod verification once', () => {
		render(ModsPage);

		expect(send).toHaveBeenCalledWith('mod_verification_subscribe', {});
		expect(send.mock.calls.filter(([type]) => type === 'mod_verification_subscribe')).toHaveLength(
			1
		);
	});

	it('subscribes to nxm links once in desktop mode', () => {
		render(ModsPage);

		expect(nexus.subscribeLinks).toHaveBeenCalledTimes(1);
	});

	it('does not subscribe to nxm links outside the desktop app', () => {
		env.desktop = 'false';
		render(ModsPage);

		expect(nexus.subscribeLinks).not.toHaveBeenCalled();
	});

	it('does not subscribe to nxm links in a remote session', () => {
		platform.web = true;
		remote.active = true;
		render(ModsPage);

		expect(nexus.subscribeLinks).not.toHaveBeenCalled();
	});

	it('selects the first target when none is requested', async () => {
		state().targets = [target('client-a'), target('client-b')];
		render(ModsPage);
		await tick();

		expect(panelProps.at(-1)?.targetId).toBe('client-a');
	});

	it('corrects an unknown target in the URL once targets have loaded', async () => {
		page.url = new URL('http://localhost/mods?target=client-gone');
		render(ModsPage);
		await tick();
		expect(goto).not.toHaveBeenCalled();

		await modTargetListHandler.handle({ targets: [target('client-a')] }, {
			goto: vi.fn()
		} as never);
		await tick();

		expect(goto).toHaveBeenCalledTimes(1);
		const [url, options] = goto.mock.calls[0] as [URL, Record<string, boolean>];
		expect(url.searchParams.get('target')).toBe('client-a');
		expect(options).toMatchObject({ replaceState: true, keepFocus: true, noScroll: true });
	});

	it('leaves a known target in the URL alone', async () => {
		page.url = new URL('http://localhost/mods?target=client-b');
		render(ModsPage);

		await modTargetListHandler.handle({ targets: [target('client-a'), target('client-b')] }, {
			goto: vi.fn()
		} as never);
		await tick();

		expect(goto).not.toHaveBeenCalled();
		expect(panelProps.at(-1)?.targetId).toBe('client-b');
	});

	it('drops the target from the URL when no targets exist', async () => {
		page.url = new URL('http://localhost/mods?target=client-gone&keep=1');
		render(ModsPage);

		await modTargetListHandler.handle({ targets: [] }, { goto: vi.fn() } as never);
		await tick();

		expect(goto).toHaveBeenCalledTimes(1);
		const [url, options] = goto.mock.calls[0] as [URL, Record<string, boolean>];
		expect(url.searchParams.has('target')).toBe(false);
		expect(url.searchParams.get('keep')).toBe('1');
		expect(options).toMatchObject({ replaceState: true, keepFocus: true, noScroll: true });
	});

	it('moves the URL to the first remaining target when the selected one is removed', async () => {
		page.url = new URL('http://localhost/mods?target=client-b');
		render(ModsPage);
		await modTargetListHandler.handle({ targets: [target('client-a'), target('client-b')] }, {
			goto: vi.fn()
		} as never);
		await tick();
		expect(goto).not.toHaveBeenCalled();

		await modTargetRemoveHandler.handle({ target_id: 'client-b', removed: true }, {
			goto: vi.fn()
		} as never);
		await tick();

		expect(goto).toHaveBeenCalledTimes(1);
		const [url] = goto.mock.calls[0] as [URL];
		expect(url.searchParams.get('target')).toBe('client-a');
	});

	it('clears Nexus in-flight state on a resets bump but leaves the account and results alone', async () => {
		render(ModsPage);
		nexus.forgetInFlight.mockClear();

		state().reset();
		await tick();

		expect(nexus.forgetInFlight).toHaveBeenCalledTimes(1);
		expect(nexus.reset).not.toHaveBeenCalled();
	});

	it('does not clear an in-flight download just from mounting, only on an actual resets bump', () => {
		render(ModsPage);

		expect(nexus.forgetInFlight).not.toHaveBeenCalled();
	});

	it('does not clear an in-flight download on a second mount without a resets bump', () => {
		const { unmount } = render(ModsPage);
		nexus.forgetInFlight.mockClear();
		unmount();

		render(ModsPage);

		expect(nexus.forgetInFlight).not.toHaveBeenCalled();
	});

	it('still clears in-flight state after a real resets bump following a remount', async () => {
		const { unmount } = render(ModsPage);
		unmount();
		render(ModsPage);
		nexus.forgetInFlight.mockClear();

		state().reset();
		await tick();

		expect(nexus.forgetInFlight).toHaveBeenCalledTimes(1);
	});

	it('still clears in-flight state after a resets bump that landed while the route was unmounted', async () => {
		const { unmount } = render(ModsPage);
		unmount();

		state().reset();
		nexus.forgetInFlight.mockClear();

		render(ModsPage);
		await tick();

		expect(nexus.forgetInFlight).toHaveBeenCalledTimes(1);
	});

	it('resets Nexus state when a remote session is entered or left, not on first mount', async () => {
		const session = { connected: true };
		const remoteInstance = new RemoteModeState({
			getSession: () => session as never,
			createTransport: () => ({ dispose: vi.fn() }) as never,
			setTransportDelegate: vi.fn(),
			resetTransportDelegate: vi.fn()
		});
		remoteMock.mockReturnValueOnce(remoteInstance);

		render(ModsPage);
		await tick();
		expect(nexus.reset).not.toHaveBeenCalled();

		remoteInstance.enter();
		await tick();
		expect(nexus.reset).toHaveBeenCalledTimes(1);

		remoteInstance.exit();
		await tick();
		expect(nexus.reset).toHaveBeenCalledTimes(2);
	});
});
