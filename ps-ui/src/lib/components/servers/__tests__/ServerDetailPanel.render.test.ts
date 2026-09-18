// @vitest-environment jsdom
import type { ApplyResult, Server } from '$types';
import { fireEvent, render, screen } from '@testing-library/svelte';
import { tick } from 'svelte';
import { beforeEach, describe, expect, it, vi } from 'vitest';

const { holder, modal, send, modsPanel, applyResult } = vi.hoisted(() => ({
	holder: { server: undefined as unknown },
	modal: { showModal: vi.fn(), showConfirmModal: vi.fn() },
	send: vi.fn(),
	modsPanel: [] as { tab: unknown }[],
	applyResult: [] as { result: unknown; targetId: unknown; onShowTab: (tab: string) => void }[]
}));

vi.mock('$lib/utils/websocketUtils', () => ({
	send: (type: unknown, data?: unknown) => send(type, data),
	sendAndWait: vi.fn()
}));

vi.mock('$states', async () => {
	const { ServerState } = await import('$lib/states/serverState.svelte');
	holder.server = new ServerState();
	return {
		getServerState: () => holder.server,
		getModalState: () => modal
	};
});

vi.mock('../ServerSettingsForm.svelte', () => ({ default: () => {} }));
vi.mock('../ServerConsole.svelte', () => ({ default: () => {} }));
vi.mock('../ServerSavePanel.svelte', () => ({ default: () => {} }));
vi.mock('../ServerModsPanel.svelte', () => ({
	default: (_anchor: unknown, props: { tab: unknown }) => {
		modsPanel.push({ tab: props.tab });
	}
}));
vi.mock('$components/mods/ApplyResult.svelte', () => ({
	default: (
		_anchor: unknown,
		props: { result: unknown; targetId: unknown; onShowTab: (tab: string) => void }
	) => {
		applyResult.push({
			result: props.result,
			targetId: props.targetId,
			onShowTab: props.onShowTab
		});
	}
}));

import { applyResult as applyResultFixture } from '$lib/components/mods/__tests__/fixtures';
import { ServerState } from '$lib/states/serverState.svelte';
import ServerDetailPanel from '../ServerDetailPanel.svelte';

const state = () => holder.server as ServerState;
const recreateNotice =
	"This server's container will be recreated the next time it starts, so it can load pak mods. Your saves and mods are kept.";
const relocationNotice =
	"This server's mod folders were changed but the mods could not be moved yet.";

function server(overrides: Partial<Server> = {}): Server {
	return {
		id: 7,
		name: 'Main World',
		server_type: 'docker',
		container_name: 'psp-main',
		status: { status: 'exited', running: false },
		...overrides
	} as Server;
}

function apply(): ApplyResult {
	return applyResultFixture({
		target_id: 'server-7',
		error: { code: 'target_locked', message: 'raw' }
	});
}

beforeEach(() => {
	send.mockReset();
	modsPanel.length = 0;
	applyResult.length = 0;
	holder.server = new ServerState();
});

describe('ServerDetailPanel container recreation', () => {
	it('explains that the container will be recreated', () => {
		render(ServerDetailPanel, { server: server({ container_needs_recreate: true }) });

		expect(screen.getByText(recreateNotice)).toBeTruthy();
	});

	it('says nothing when recreation is not needed or unknown', () => {
		const { unmount } = render(ServerDetailPanel, {
			server: server({ container_needs_recreate: false })
		});
		expect(screen.queryByText(recreateNotice)).toBeNull();
		unmount();

		render(ServerDetailPanel, { server: server() });
		expect(screen.queryByText(recreateNotice)).toBeNull();
	});
});

describe('ServerDetailPanel start', () => {
	it('stays busy after a start and sends nothing on a second click', async () => {
		render(ServerDetailPanel, { server: server() });
		const start = screen.getByRole('button', { name: /Start/ });

		await fireEvent.click(start);
		await tick();
		expect(send).toHaveBeenCalledWith('start_server', { server_id: 7 });
		expect(start.getAttribute('aria-busy')).toBe('true');

		await fireEvent.click(start);
		expect(send).toHaveBeenCalledTimes(1);

		state().starting = {};
		await tick();
		expect(start.getAttribute('aria-busy')).toBe('false');
	});
});

describe('ServerDetailPanel relocation', () => {
	it('says the mods moved when only setting up the server failed', () => {
		state().setRelocation(
			7,
			apply(),
			{ code: 'container_create_failed', message: 'no such image' },
			true
		);
		render(ServerDetailPanel, { server: server() });

		expect(
			screen.getByText(
				"This server's mods were moved, but the server could not be set up afterwards."
			)
		).toBeTruthy();
		expect(screen.queryByText(relocationNotice)).toBeNull();
		expect(
			screen.getByText("The server's container could not be created: no such image")
		).toBeTruthy();
	});

	it('shows no relocation card without pending state', () => {
		render(ServerDetailPanel, { server: server() });

		expect(screen.queryByText(relocationNotice)).toBeNull();
	});

	it('shows the warning, the apply result and a Try again that sends a start', async () => {
		const result = apply();
		state().relocationPending = { 7: result };
		render(ServerDetailPanel, { server: server() });

		expect(screen.getByText(relocationNotice)).toBeTruthy();
		expect(applyResult).toHaveLength(1);
		expect(applyResult[0].result).toEqual(result);
		expect(applyResult[0].targetId).toBe('server-7');

		const retry = screen.getByRole('button', { name: 'Try again' });
		await fireEvent.click(retry);
		await tick();
		expect(send).toHaveBeenCalledWith('start_server', { server_id: 7 });
		expect(retry.getAttribute('aria-busy')).toBe('true');
		await fireEvent.click(retry);
		expect(send).toHaveBeenCalledTimes(1);
	});

	it('shows the stored error and no apply result for a relocation without one', () => {
		state().relocationPending = { 7: true };
		state().relocationError = { 7: { code: 'container_create_failed', message: 'no such image' } };
		render(ServerDetailPanel, { server: server() });

		expect(screen.getByText(relocationNotice)).toBeTruthy();
		expect(
			screen.getByText("The server's container could not be created: no such image")
		).toBeTruthy();
		expect(applyResult).toHaveLength(0);
	});

	it('opens the requested mods tab from the apply result', async () => {
		state().relocationPending = { 7: apply() };
		render(ServerDetailPanel, { server: server() });
		expect(modsPanel).toHaveLength(0);

		applyResult[0].onShowTab('backups');
		await tick();

		expect(modsPanel).toEqual([{ tab: 'backups' }]);
	});
});
