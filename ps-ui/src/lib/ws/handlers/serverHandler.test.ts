import { beforeEach, describe, expect, it, vi } from 'vitest';

const { send, toast, holder } = vi.hoisted(() => ({
	send: vi.fn(),
	toast: { add: vi.fn() },
	holder: { server: undefined as unknown, mods: undefined as unknown }
}));

vi.mock('$lib/utils/websocketUtils', () => ({
	send: (type: unknown, data?: unknown) => send(type, data),
	sendAndWait: vi.fn()
}));

vi.mock('$states', async () => {
	const { ServerState } = await import('$lib/states/serverState.svelte');
	const { ModsState } = await import('$lib/states/modsState.svelte');
	holder.server = new ServerState();
	holder.mods = new ModsState();
	return {
		getServerState: () => holder.server,
		getModsState: () => holder.mods,
		getToastState: () => toast
	};
});

import { applyResult as baseApplyResult } from '$lib/components/mods/__tests__/fixtures';
import { ModsState } from '$lib/states/modsState.svelte';
import { ServerState } from '$lib/states/serverState.svelte';
import type { ApplyResult, Server, ServerStatus } from '$types';
import {
	deleteServerHandler,
	ensureGamedataLaunchArgHandler,
	getServerHandler,
	listServersHandler,
	serverStatusUpdateHandler,
	updateServerHandler
} from './serverHandler';

const context = { goto: vi.fn() } as never;
const state = () => holder.server as ServerState;

const stopped: ServerStatus = { status: 'exited', running: false };
const running: ServerStatus = { status: 'running', running: true };

function server(overrides: Partial<Server> = {}): Server {
	return {
		id: 7,
		name: 'Main World',
		server_type: 'docker',
		container_name: 'psp-main',
		status: stopped,
		...overrides
	} as Server;
}

function applyResult(overrides: Partial<ApplyResult> = {}): ApplyResult {
	return baseApplyResult({ target_id: 'server-7', ...overrides });
}

function toastTexts(): string[] {
	return toast.add.mock.calls.map((call) => call[0] as string);
}

beforeEach(() => {
	send.mockReset();
	toast.add.mockReset();
	holder.server = new ServerState();
	holder.mods = new ModsState();
	state().servers = [server()];
	state().selectedServer = server();
});

describe('mods target after a relocation', () => {
	const mods = () => holder.mods as ModsState;

	it('reloads the target, its plan and its scan when an update reply carries an apply', async () => {
		mods().scan('server-7');
		send.mockReset();

		await updateServerHandler.handle(
			{ ...server(), relocation_pending: false, apply: applyResult() },
			context
		);

		expect(send.mock.calls).toEqual([
			['mod_target_list', undefined],
			['profile_plan', { target_id: 'server-7' }],
			['mod_target_scan', { target_id: 'server-7', candidates_only: true }]
		]);
	});

	it('reloads nothing for an update that moved no mods', async () => {
		await updateServerHandler.handle({ ...server(), name: 'Renamed' }, context);

		expect(send).not.toHaveBeenCalled();
	});

	it('reloads the target when a start closes a pending move', async () => {
		state().setRelocation(7, null);

		await serverStatusUpdateHandler.handle(
			{ server_id: 7, status: running, success: true },
			context
		);

		expect(send.mock.calls).toEqual([
			['mod_target_list', undefined],
			['profile_plan', { target_id: 'server-7' }]
		]);
	});

	it('reloads nothing when a start fails for a reason unrelated to a pending move', async () => {
		state().setRelocation(7, null);

		await serverStatusUpdateHandler.handle(
			{
				server_id: 7,
				status: stopped,
				success: false,
				error: { code: 'not_found', message: 'Server not found' }
			},
			context
		);

		expect(send).not.toHaveBeenCalled();
	});
});

describe('ServerState.startServer', () => {
	it('releases a start refused because the server was not found', async () => {
		state().startServer(7);

		await serverStatusUpdateHandler.handle(
			{
				server_id: 7,
				status: null,
				success: false,
				error: { code: 'not_found', message: 'Server not found' }
			},
			context
		);

		expect(state().starting[7]).toBeFalsy();
		expect(toast.add).toHaveBeenCalledWith('Server not found', 'Error', 'error');
		state().startServer(7);
		expect(send).toHaveBeenCalledTimes(2);
	});

	it('sends one start while that server is starting', () => {
		state().startServer(7);
		state().startServer(7);

		expect(send).toHaveBeenCalledTimes(1);
		expect(send).toHaveBeenCalledWith('start_server', { server_id: 7 });
		expect(state().starting[7]).toBe(true);
	});

	it('starts another server independently', () => {
		state().startServer(7);
		state().startServer(8);

		expect(send).toHaveBeenCalledTimes(2);
	});

	it('stays busy until the status update for that server arrives', async () => {
		state().startServer(7);
		await serverStatusUpdateHandler.handle(
			{ server_id: 8, status: running, success: true },
			context
		);
		expect(state().starting[7]).toBe(true);

		await serverStatusUpdateHandler.handle(
			{ server_id: 7, status: running, success: true },
			context
		);
		expect(state().starting[7]).toBeFalsy();
		state().startServer(7);
		expect(send).toHaveBeenCalledTimes(2);
	});

	it("releases only that server's start on a refused status update", async () => {
		state().startServer(7);
		state().startServer(8);

		await serverStatusUpdateHandler.handle(
			{
				server_id: 7,
				status: stopped,
				success: false,
				error: { code: 'apply_in_progress', message: 'raw', target_id: 'server-7' }
			},
			context
		);

		expect(state().starting[7]).toBeFalsy();
		expect(state().starting[8]).toBe(true);
		state().startServer(7);
		state().startServer(8);
		expect(send).toHaveBeenCalledTimes(3);
	});
});

describe('listed relocation state', () => {
	it('clears relocation state for a server listed as no longer relocating', async () => {
		state().setRelocation(7, applyResult(), { code: 'container_create_failed', message: 'x' });

		await listServersHandler.handle(
			{ servers: [{ ...server(), relocation_pending: false }] },
			context
		);

		expect(state().relocationPending[7]).toBeUndefined();
		expect(state().relocationError[7]).toBeUndefined();
		expect(state().servers[0]).not.toHaveProperty('relocation_pending');
		expect(state().selectedServer).not.toHaveProperty('relocation_pending');
	});

	it('shows a listed pending relocation and keeps a stored apply result', async () => {
		const apply = applyResult();
		state().setRelocation(8, apply);

		await listServersHandler.handle(
			{
				servers: [
					{ ...server(), relocation_pending: true },
					{ ...server({ id: 8 }), relocation_pending: true }
				]
			},
			context
		);

		expect(state().relocationPending[7]).toBe(true);
		expect(state().relocationPending[8]).toEqual(apply);
	});

	it('leaves relocation state alone for a listing without the field', async () => {
		state().setRelocation(7, applyResult());

		await listServersHandler.handle({ servers: [server()] }, context);

		expect(state().relocationPending[7]).toEqual(applyResult());
	});

	it('clears relocation state from a fetched server', async () => {
		state().setRelocation(7, true as never);

		await getServerHandler.handle({ ...server(), relocation_pending: false }, context);

		expect(state().relocationPending[7]).toBeUndefined();
		expect(state().selectedServer).not.toHaveProperty('relocation_pending');
	});
});

describe('updateServerHandler', () => {
	it('stores a successful update and keeps a known container recreation', async () => {
		state().servers = [server({ container_needs_recreate: true })];
		state().selectedServer = server({ container_needs_recreate: true });
		state().saving = true;

		await updateServerHandler.handle(
			{ ...server({ name: 'Renamed' }), relocation_pending: false },
			context
		);

		expect(state().servers[0].name).toBe('Renamed');
		expect(state().servers[0].container_needs_recreate).toBe(true);
		expect(state().selectedServer?.container_needs_recreate).toBe(true);
		expect(state().servers[0]).not.toHaveProperty('relocation_pending');
		expect(state().saving).toBe(false);
		expect(toast.add).toHaveBeenCalledWith('Server "Renamed" updated', 'Success', 'success');
	});

	it('takes an explicit container recreation value from the reply', async () => {
		state().servers = [server({ container_needs_recreate: true })];

		await getServerHandler.handle(server({ container_needs_recreate: false }), context);

		expect(state().servers[0].container_needs_recreate).toBe(false);
	});

	it('toasts a refusal carrying only the server id and leaves the server alone', async () => {
		state().saving = true;

		await updateServerHandler.handle(
			{
				server_id: 7,
				error: { code: 'server_state_unknown', message: 'raw' }
			},
			context
		);

		expect(state().saving).toBe(false);
		expect(state().servers).toEqual([server()]);
		expect(toastTexts()).toEqual([
			"PalStudio couldn't tell whether this server is running, so none of your changes were saved. Check that Docker is running and try again."
		]);
		expect(toastTexts().some((text) => text.includes('undefined'))).toBe(false);
	});

	it('stores the saved settings of an update refused by a running apply', async () => {
		await updateServerHandler.handle(
			{
				...server({ name: 'Renamed' }),
				error: { code: 'apply_in_progress', message: 'raw', target_id: 'server-7' }
			},
			context
		);

		expect(state().servers[0].name).toBe('Renamed');
		expect(state().servers[0]).not.toHaveProperty('error');
		expect(toast.add).toHaveBeenCalledTimes(1);
		expect(toastTexts()[0]).toMatch(/saved, but the server was not restarted/);
		expect(toast.add.mock.calls[0][2]).toBe('warning');
	});

	it('shows the raw message of an unrecognised refusal', async () => {
		await updateServerHandler.handle(
			{ server_id: 7, error: { code: 'db', message: 'disk I/O error' } },
			context
		);

		expect(toastTexts()).toEqual(['disk I/O error']);
	});

	it('stores a failed relocation close as relocation state, not as success', async () => {
		const apply = applyResult();

		await updateServerHandler.handle(
			{
				...server(),
				relocation_pending: true,
				apply,
				error: { code: 'container_create_failed', message: 'no such image' }
			},
			context
		);

		expect(state().relocationPending[7]).toEqual(apply);
		expect(state().relocationError[7]).toMatchObject({ code: 'container_create_failed' });
		expect(toastTexts()).toEqual(["The server's container could not be created: no such image"]);
		expect(toast.add.mock.calls[0][2]).toBe('error');
	});

	it('stores a pending relocation without an apply result as true', async () => {
		await updateServerHandler.handle(
			{ ...server(), relocation_pending: true, apply: null },
			context
		);

		expect(state().relocationPending[7]).toBe(true);
		expect(state().relocationError[7]).toBeUndefined();
		expect(toastTexts()).toEqual([
			"This server's mod folders were changed but the mods could not be moved yet."
		]);
	});

	it('clears relocation state when a full reply arrives without it', async () => {
		state().relocationPending = { 7: true };
		state().relocationError = { 7: { code: 'container_create_failed', message: 'x' } };

		await updateServerHandler.handle({ ...server(), relocation_pending: false }, context);

		expect(state().relocationPending[7]).toBeUndefined();
		expect(state().relocationError[7]).toBeUndefined();
	});

	it('keeps relocation state across a refusal that carries only the server id', async () => {
		state().relocationPending = { 7: true };

		await updateServerHandler.handle(
			{ server_id: 7, error: { code: 'server_state_unknown', message: 'raw' } },
			context
		);

		expect(state().relocationPending[7]).toBe(true);
	});

	it('keeps the stored apply result and cause when a save reports the move still pending', async () => {
		const apply = applyResult({ error: { code: 'target_locked', message: 'raw' } });
		const cause = { code: 'settings_write_failed', message: 'denied' };
		state().setRelocation(7, apply, cause);

		await updateServerHandler.handle(
			{ ...server({ name: 'Renamed' }), relocation_pending: true },
			context
		);

		expect(state().servers[0].name).toBe('Renamed');
		expect(state().relocationPending[7]).toEqual(apply);
		expect(state().relocationError[7]).toEqual(cause);
	});

	it('marks a move reported pending without an apply result when nothing is stored', async () => {
		await updateServerHandler.handle({ ...server(), relocation_pending: true }, context);

		expect(state().relocationPending[7]).toBe(true);
	});

	it('records that the mods moved when only the container could not be created', async () => {
		await updateServerHandler.handle(
			{
				...server(),
				relocation_pending: true,
				apply: applyResult(),
				error: { code: 'container_create_failed', message: 'no such image' }
			},
			context
		);

		expect(state().relocationMoved[7]).toBe(true);
	});

	it('records that the mods did not move when the apply did not close', async () => {
		await updateServerHandler.handle(
			{ ...server(), relocation_pending: true, apply: applyResult({ mid_apply: true }) },
			context
		);

		expect(state().relocationPending[7]).toEqual(applyResult({ mid_apply: true }));
		expect(state().relocationMoved[7]).toBeFalsy();
	});

	it('records that the mods did not move when the database failed before the apply', async () => {
		await updateServerHandler.handle(
			{
				...server(),
				relocation_pending: true,
				apply: null,
				error: { code: 'db', message: 'locked' }
			},
			context
		);

		expect(state().relocationPending[7]).toBe(true);
		expect(state().relocationMoved[7]).toBeFalsy();
	});
});

describe('ensureGamedataLaunchArgHandler', () => {
	it('stores nothing for a refusal given as a message', async () => {
		await ensureGamedataLaunchArgHandler.handle({ error: 'Server not found' }, context);

		expect(state().servers).toEqual([server()]);
	});

	it('stores the saved server of a coded refusal without its error', async () => {
		state().servers = [server({ container_needs_recreate: true })];

		await ensureGamedataLaunchArgHandler.handle(
			{
				...server({ launch_args: '-enable-gamedata-api' }),
				error: { code: 'apply_in_progress', message: 'raw', target_id: 'server-7' }
			},
			context
		);

		expect(state().servers[0].launch_args).toBe('-enable-gamedata-api');
		expect(state().servers[0]).not.toHaveProperty('error');
		expect(state().servers[0].container_needs_recreate).toBe(true);
	});

	it('keeps a stored container recreation the reply omits', async () => {
		state().servers = [server({ container_needs_recreate: true })];

		await ensureGamedataLaunchArgHandler.handle(server({ launch_args: '-x' }), context);

		expect(state().servers[0].launch_args).toBe('-x');
		expect(state().servers[0].container_needs_recreate).toBe(true);
	});
});

describe('deleteServerHandler', () => {
	it('keeps a server whose deletion was refused', async () => {
		await deleteServerHandler.handle(
			{
				server_id: 7,
				error: { code: 'apply_in_progress', message: 'raw', target_id: 'server-7' }
			},
			context
		);

		expect(state().servers).toHaveLength(1);
		expect(state().selectedServer?.id).toBe(7);
		expect(toastTexts()[0]).toMatch(/so it was not deleted/);
		expect(toast.add.mock.calls[0][2]).toBe('error');
	});

	it('removes a deleted server', async () => {
		await deleteServerHandler.handle({ server_id: 7 }, context);

		expect(state().servers).toHaveLength(0);
		expect(state().selectedServer).toBeNull();
	});
});

describe('serverStatusUpdateHandler', () => {
	it('stores a start refused for a pending relocation with its apply and cause', async () => {
		const apply = applyResult({ error: { code: 'target_locked', message: 'raw' } });

		await serverStatusUpdateHandler.handle(
			{
				server_id: 7,
				status: stopped,
				success: false,
				error: {
					code: 'relocation_pending',
					message: 'raw',
					apply,
					cause: { code: 'settings_write_failed', message: 'denied' }
				}
			},
			context
		);

		expect(state().relocationPending[7]).toEqual(apply);
		expect(state().relocationError[7]).toEqual({
			code: 'settings_write_failed',
			message: 'denied'
		});
		expect(state().servers[0].status?.running).toBe(false);
		expect(toastTexts()).toEqual([
			"This server's mod folders were changed but the mods could not be moved yet."
		]);
	});

	it('keeps a relocation whose container could not be created on start', async () => {
		await serverStatusUpdateHandler.handle(
			{
				server_id: 7,
				status: null,
				success: false,
				error: { code: 'container_create_failed', message: 'no such image', apply: null }
			},
			context
		);

		expect(state().relocationPending[7]).toBe(true);
		expect(state().relocationError[7]).toMatchObject({ code: 'container_create_failed' });
	});

	it('toasts a container that could not be created and keeps the server stopped', async () => {
		state().relocationPending = { 7: true };

		await serverStatusUpdateHandler.handle(
			{
				server_id: 7,
				status: stopped,
				success: false,
				error: { code: 'container_create_failed', message: 'no such image' }
			},
			context
		);

		expect(state().servers[0].status?.running).toBe(false);
		expect(state().relocationPending[7]).toBeUndefined();
		expect(toastTexts()).toEqual(["The server's container could not be created: no such image"]);
		expect(toast.add.mock.calls[0][2]).toBe('error');
	});

	it('clears relocation state when the server starts', async () => {
		state().relocationPending = { 7: true };

		await serverStatusUpdateHandler.handle(
			{ server_id: 7, status: running, success: true },
			context
		);

		expect(state().relocationPending[7]).toBeUndefined();
	});

	it('keeps relocation state when the server stops', async () => {
		state().relocationPending = { 7: true };

		await serverStatusUpdateHandler.handle(
			{ server_id: 7, status: stopped, success: true },
			context
		);

		expect(state().relocationPending[7]).toBe(true);
	});

	it('records that the mods did not move for a relocation_pending refusal', async () => {
		await serverStatusUpdateHandler.handle(
			{
				server_id: 7,
				status: stopped,
				success: false,
				error: { code: 'relocation_pending', message: 'raw', apply: applyResult() }
			},
			context
		);

		expect(state().relocationMoved[7]).toBeFalsy();
	});

	it('records that the mods moved when setting up the server after the move failed', async () => {
		await serverStatusUpdateHandler.handle(
			{
				server_id: 7,
				status: stopped,
				success: false,
				error: { code: 'container_create_failed', message: 'x', apply: applyResult() }
			},
			context
		);

		expect(state().relocationMoved[7]).toBe(true);
	});

	it('keeps relocation state when a start fails on the database', async () => {
		state().setRelocation(7, applyResult());

		await serverStatusUpdateHandler.handle(
			{ server_id: 7, status: null, success: false, error: { code: 'db', message: 'locked' } },
			context
		);

		expect(state().relocationPending[7]).toEqual(applyResult());
	});

	it('clears a known container recreation once the server starts', async () => {
		state().servers = [server({ container_needs_recreate: true })];
		state().selectedServer = server({ container_needs_recreate: true });

		await serverStatusUpdateHandler.handle(
			{ server_id: 7, status: running, success: true },
			context
		);

		expect(state().servers[0].container_needs_recreate).toBe(false);
		expect(state().selectedServer?.container_needs_recreate).toBe(false);
	});

	it('keeps a known container recreation when the server stops', async () => {
		state().servers = [server({ container_needs_recreate: true })];

		await serverStatusUpdateHandler.handle(
			{ server_id: 7, status: stopped, success: true },
			context
		);

		expect(state().servers[0].container_needs_recreate).toBe(true);
	});

	it('accepts a successful reply without a status', async () => {
		await serverStatusUpdateHandler.handle({ server_id: 7, status: null, success: true }, context);

		expect(toastTexts()).toEqual(['Server stopped']);
	});
});
