// @vitest-environment jsdom
import '$utils/__tests__/fixtures/animatePolyfill';
import { fireEvent, render, screen, within } from '@testing-library/svelte';
import { tick } from 'svelte';
import { beforeEach, describe, expect, it, vi } from 'vitest';

import { installViewportStub } from '$utils/__tests__/fixtures/viewportStub';

const { serverState, showModal } = vi.hoisted(() => ({
	showModal: vi.fn(),
	serverState: {
		servers: [] as unknown[],
		selectedServer: null as unknown,
		loading: false,
		creationProgress: null as unknown,
		loadServers: vi.fn(),
		startPolling: vi.fn(),
		stopPolling: vi.fn(),
		selectServer: vi.fn(),
		startServer: vi.fn(),
		stopServer: vi.fn()
	}
}));

vi.mock('$states', () => ({
	getServerState: () => serverState,
	getModalState: () => ({ showModal })
}));

vi.mock('$components/servers', async () => ({
	ServerCard: () => {},
	ServerDetailPanel: (await import('./fixtures/ServerDetailStub.svelte')).default,
	CreateServerModal: {},
	ImportServerModal: {}
}));

// Installed before the page import, which reaches the layout store.
const { setViewport } = installViewportStub();
const { default: ServersPage } = await import('../+page.svelte');

const server = { id: 1, name: 'Main', game_port: 8211, query_port: 27015, rest_api_port: 8212 };

beforeEach(() => {
	vi.clearAllMocks();
	setViewport(1440);
	serverState.servers = [server];
	serverState.selectedServer = null;
});

describe('servers page', () => {
	it('stacks the list over the detail below the breakpoint', async () => {
		setViewport(390);
		render(ServersPage);
		await tick();

		const shell = document.querySelector('#servers-shell');
		expect(shell?.className).toContain('flex-col');
		expect(shell?.className).toContain('md:flex-row');

		const list = document.querySelector('#servers-list');
		expect(list?.className).toContain('w-full');
		expect(list?.className).toContain('md:w-80');
		const unprefixed = (list?.className ?? '').split(/ +/).filter((token) => !token.includes(':'));
		expect(unprefixed).not.toContain('w-80');
	});

	it('keeps the detail beside the list on a desktop', async () => {
		serverState.selectedServer = server;
		render(ServersPage);
		await tick();

		expect(screen.queryByRole('dialog')).toBeNull();
		expect(screen.getByTestId('server-detail')).not.toBeNull();
	});

	it('lifts the detail into a sheet on a phone', async () => {
		setViewport(390);
		serverState.selectedServer = server;
		render(ServersPage);
		await tick();

		const sheet = screen.getByRole('dialog');
		expect(within(sheet).getByTestId('server-detail')).not.toBeNull();
	});

	it('leaves the list alone until a server is picked', async () => {
		setViewport(390);
		render(ServersPage);
		await tick();

		expect(screen.queryByRole('dialog')).toBeNull();
	});

	it('drops the selection when the sheet is dismissed', async () => {
		setViewport(390);
		serverState.selectedServer = server;
		render(ServersPage);
		await tick();

		await fireEvent.click(screen.getByTestId('sheet-backdrop'));

		expect(serverState.selectedServer).toBeNull();
	});
});

describe('servers page empty state', () => {
	it('offers a hosted server when there are none', async () => {
		serverState.servers = [];
		render(ServersPage);
		await tick();

		await fireEvent.click(screen.getByRole('button', { name: /rent a hosted server/i }));

		expect(showModal).toHaveBeenCalledWith({}, expect.objectContaining({ initialType: 'hosted' }));
	});

	it('hides the offer once a server exists', async () => {
		render(ServersPage);
		await tick();

		expect(screen.queryByRole('button', { name: /rent a hosted server/i })).toBeNull();
	});
});
