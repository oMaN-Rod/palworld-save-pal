// @vitest-environment jsdom
import { MessageType, type TargetVerification } from '$types';
import { fireEvent, render, screen, waitFor } from '@testing-library/svelte';
import { tick } from 'svelte';
import { beforeEach, describe, expect, it, vi } from 'vitest';

const { holder, remote, send } = vi.hoisted(() => ({
	holder: { state: undefined as unknown, game: undefined as unknown },
	remote: { active: false },
	send: vi.fn()
}));

vi.mock('$lib/signal/remoteMode.svelte', () => ({ getRemoteMode: () => remote }));

vi.mock('$lib/utils/websocketUtils', () => ({
	send: (type: unknown, data?: unknown) => send(type, data),
	sendAndWait: vi.fn(),
	sendBytes: vi.fn(),
	isReady: () => true,
	pushProgressMessage: vi.fn()
}));

vi.mock('$states', async () => {
	const { ModsState } = await import('$lib/states/modsState.svelte');
	const { GameCommandError, GameState } = await import('$lib/states/gameState.svelte');
	holder.state = new ModsState();
	holder.game = new GameState();
	return {
		getModsState: () => holder.state,
		getGameState: () => holder.game,
		GameCommandError
	};
});

import type { GameInstanceJson, GameState } from '$lib/states/gameState.svelte';
import { GameCommandError } from '$lib/states/gameState.svelte';
import type { ModsState } from '$lib/states/modsState.svelte';
import LivePanel from '../LivePanel.svelte';
import { modTarget } from './fixtures';

const state = () => holder.state as ModsState;
const game = () => holder.game as GameState;
const targetId = 't1';

function verification(overrides: Partial<TargetVerification> = {}): TargetVerification {
	return {
		target_id: 't1',
		instance_id: 'saved:1',
		live: true,
		checked_at: '2026-09-01T10:00:00Z',
		build_info: { ue4ssVersion: '3.0.1', amityVersion: '0.3.0', platform: 'win64' },
		resolution: { complete: true, missing: [] },
		status: [],
		...overrides
	};
}

function savedInstance(overrides: Partial<GameInstanceJson> = {}): GameInstanceJson {
	return {
		id: 'saved:1',
		source: 'saved',
		name: 'Remote box',
		host: '10.0.0.5',
		port: 8788,
		live: false,
		targetId: null,
		...overrides
	};
}

function autoInstance(overrides: Partial<GameInstanceJson> = {}): GameInstanceJson {
	return {
		id: 'auto:1',
		source: 'auto',
		name: 'This install',
		host: '127.0.0.1',
		port: 52104,
		live: true,
		targetId: null,
		...overrides
	};
}

beforeEach(() => {
	send.mockReset();
	remote.active = false;
	state().reset();
	game().instances = [];
	vi.spyOn(game(), 'refreshInstances').mockResolvedValue(undefined);
});

describe('LivePanel', () => {
	it('requests verification on mount and shows nothing connected without an entry', () => {
		render(LivePanel, { targetId });
		expect(send).toHaveBeenCalledWith('mod_verification_get', { target_id: 't1' });
		expect(screen.getByText(/No game has been checked for this install yet/)).toBeTruthy();
	});

	it('shows the connection state and the build info versions', () => {
		state().verification = { t1: verification() };
		render(LivePanel, { targetId });
		expect(screen.getByText('Connected to the running game')).toBeTruthy();
		expect(screen.getByText(/UE4SS 3.0.1/)).toBeTruthy();
		expect(screen.getByText(/Amity 0.3.0/)).toBeTruthy();
	});

	it('shows only the present build info parts', () => {
		state().verification = {
			t1: verification({ build_info: { platform: 'win64' } })
		};
		render(LivePanel, { targetId });
		expect(screen.getByText('win64')).toBeTruthy();
		expect(screen.queryByText(/UE4SS/)).toBeNull();
		expect(screen.queryByText(/Amity/)).toBeNull();
	});

	it('shows no build info line when none of the three keys are present', () => {
		state().verification = { t1: verification({ build_info: {} }) };
		render(LivePanel, { targetId });
		expect(screen.queryByText(/UE4SS/)).toBeNull();
		expect(screen.queryByText(/Amity/)).toBeNull();
		expect(screen.queryByText('win64')).toBeNull();
	});

	it('lists the missing signature names when the resolution is incomplete', () => {
		state().verification = {
			t1: verification({ resolution: { complete: false, missing: ['Sanity_Get'] } })
		};
		render(LivePanel, { targetId });
		expect(screen.getByText(/Sanity_Get/)).toBeTruthy();
	});

	it('shows unexpected mods under their own heading', () => {
		state().verification = {
			t1: verification({
				status: [{ mod_id: null, name: 'Ghost', kind: 'pak', status: 'unexpected' }]
			})
		};
		render(LivePanel, { targetId });
		expect(screen.getByText('Loaded but not in this profile')).toBeTruthy();
		expect(screen.getByText('Ghost')).toBeTruthy();
	});

	it('binds a saved instance to this install through its select', async () => {
		const setInstanceTarget = vi.spyOn(game(), 'setInstanceTarget').mockResolvedValue(undefined);
		game().instances = [savedInstance()];
		render(LivePanel, { targetId });

		const select = screen.getByLabelText('Remote box mod checks') as HTMLSelectElement;
		await fireEvent.change(select, { target: { value: 't1' } });

		expect(setInstanceTarget).toHaveBeenCalledWith('saved:1', 't1');
	});

	it('shows a saved instance bound to a server target as its own option', () => {
		state().targets = [
			modTarget({ id: 'server-7', kind: 'server', server_id: 7, name: 'Main World' })
		];
		game().instances = [savedInstance({ targetId: 'server-7' })];
		render(LivePanel, { targetId });

		const select = screen.getByLabelText('Remote box mod checks') as HTMLSelectElement;
		expect(select.value).toBe('server-7');
		expect(screen.getByText('Main World (not a game install)')).toBeTruthy();
	});

	it('shows a saved instance bound to an unknown target as unknown', () => {
		game().instances = [savedInstance({ targetId: 'gone' })];
		render(LivePanel, { targetId });

		const select = screen.getByLabelText('Remote box mod checks') as HTMLSelectElement;
		expect(select.value).toBe('gone');
		expect(screen.getByText('An install that no longer exists')).toBeTruthy();
	});

	it('disables the select while its own change is in flight', async () => {
		let resolveChange: () => void = () => {};
		const pending = new Promise<void>((resolve) => {
			resolveChange = resolve;
		});
		vi.spyOn(game(), 'setInstanceTarget').mockReturnValue(pending);
		game().instances = [savedInstance()];
		render(LivePanel, { targetId });

		const select = screen.getByLabelText('Remote box mod checks') as HTMLSelectElement;
		fireEvent.change(select, { target: { value: 't1' } });
		await tick();
		expect(select.disabled).toBe(true);

		resolveChange();
		await waitFor(() => expect(select.disabled).toBe(false));
	});

	it('refreshes instances on mount and again when the panel switches target', async () => {
		game().instances = [savedInstance()];
		const { rerender } = render(LivePanel, { targetId });
		expect(game().refreshInstances).toHaveBeenCalledTimes(1);

		await rerender({ targetId: 't2' });
		await tick();

		expect(game().refreshInstances).toHaveBeenCalledTimes(2);
	});

	it('does not refetch instances after a successful bind, which already returns them', async () => {
		vi.spyOn(game(), 'setInstanceTarget').mockResolvedValue(undefined);
		game().instances = [savedInstance()];
		render(LivePanel, { targetId });

		const select = screen.getByLabelText('Remote box mod checks') as HTMLSelectElement;
		await fireEvent.change(select, { target: { value: 't1' } });
		await tick();

		expect(game().setInstanceTarget).toHaveBeenCalledWith('saved:1', 't1');
		expect(game().refreshInstances).toHaveBeenCalledTimes(1);
	});

	it('renders a mapped refusal as an alert and resets the select', async () => {
		vi.spyOn(game(), 'setInstanceTarget').mockRejectedValue(
			new GameCommandError('nope', 'target_not_found')
		);
		game().instances = [savedInstance()];
		render(LivePanel, { targetId });

		const select = screen.getByLabelText('Remote box mod checks') as HTMLSelectElement;
		await fireEvent.change(select, { target: { value: 't1' } });
		await tick();

		expect(screen.getByRole('alert').textContent).toContain('That install no longer exists.');
		expect(select.value).toBe('');
	});

	it('shows a plain error message and resets the select on a non-GameCommandError rejection', async () => {
		vi.spyOn(game(), 'setInstanceTarget').mockRejectedValue(new Error('socket closed'));
		game().instances = [savedInstance()];
		render(LivePanel, { targetId });

		const select = screen.getByLabelText('Remote box mod checks') as HTMLSelectElement;
		await fireEvent.change(select, { target: { value: 't1' } });
		await tick();

		expect(screen.getByRole('alert').textContent).toContain('socket closed');
		expect(select.value).toBe('');
	});

	it('keeps a refusal only on its own row', async () => {
		const setInstanceTarget = vi.spyOn(game(), 'setInstanceTarget');
		setInstanceTarget.mockRejectedValueOnce(new GameCommandError('nope', 'target_not_found'));
		setInstanceTarget.mockResolvedValueOnce(undefined);
		game().instances = [
			savedInstance({ id: 'saved:1', name: 'Remote box' }),
			savedInstance({ id: 'saved:2', name: 'Other box' })
		];
		render(LivePanel, { targetId });

		const first = screen.getByLabelText('Remote box mod checks') as HTMLSelectElement;
		const second = screen.getByLabelText('Other box mod checks') as HTMLSelectElement;

		await fireEvent.change(first, { target: { value: 't1' } });
		await tick();
		expect(screen.getAllByRole('alert')).toHaveLength(1);

		await fireEvent.change(second, { target: { value: 't1' } });
		await tick();

		expect(screen.getAllByRole('alert')).toHaveLength(1);
		expect(screen.getByRole('alert').textContent).toContain('That install no longer exists.');
	});

	it('shows an auto instance bound to this install as read-only text', () => {
		game().instances = [autoInstance({ targetId: 't1' })];
		render(LivePanel, { targetId });

		expect(screen.getByText('Checks this install')).toBeTruthy();
		expect(screen.queryByRole('combobox')).toBeNull();
	});

	it('hides the bind select in a remote session', () => {
		remote.active = true;
		game().instances = [savedInstance()];
		render(LivePanel, { targetId });

		expect(screen.queryByLabelText('Remote box mod checks')).toBeNull();
	});

	it('never requests game instances in a remote session and shows the remote line instead', () => {
		remote.active = true;
		render(LivePanel, { targetId });

		expect(game().refreshInstances).not.toHaveBeenCalled();
		expect(
			screen.getByText('Game connections can be viewed on the computer running PalStudio.')
		).toBeTruthy();
	});

	it('shows a mod_verification_get refusal as an alert', async () => {
		render(LivePanel, { targetId });
		state().recordRefusal(
			MessageType.MOD_VERIFICATION_GET,
			{ code: 'target_not_found', message: 'That install no longer exists.' },
			't1'
		);
		await tick();

		expect(screen.getByRole('alert').textContent).toContain('That install no longer exists.');
	});
});
