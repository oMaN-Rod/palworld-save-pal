// @vitest-environment jsdom
import type { LocalSave, ModProfile } from '$types';
import { fireEvent, render, screen, within } from '@testing-library/svelte';
import { tick } from 'svelte';
import { beforeEach, describe, expect, it, vi } from 'vitest';

const { holder, send } = vi.hoisted(() => ({
	holder: { state: undefined as unknown },
	send: vi.fn()
}));

vi.mock('$lib/utils/websocketUtils', () => ({
	send: (type: unknown, data?: unknown) => send(type, data),
	sendAndWait: vi.fn(),
	sendBytes: vi.fn(),
	isReady: () => true,
	pushProgressMessage: vi.fn()
}));

vi.mock('$states', async () => {
	const { ModsState } = await import('$lib/states/modsState.svelte');
	holder.state = new ModsState();
	return { getModsState: () => holder.state };
});

import type { ModsState } from '$lib/states/modsState.svelte';
import WorldsPanel from '../WorldsPanel.svelte';

const state = () => holder.state as ModsState;

function profile(id: string, name: string, isActive: boolean): ModProfile {
	return {
		id,
		target_id: 'client-abc',
		name,
		is_active: isActive,
		is_default: isActive,
		mods: [],
		ue4ss_control_mode: 'enabled_txt',
		force_order_ue4ss: false,
		force_order_palschema: false,
		created_at: '',
		updated_at: ''
	};
}

function save(name: string, overrides: Partial<LocalSave> = {}): LocalSave {
	return {
		path: `C:/saves/1/${name}/Level.sav`,
		name,
		save_type: 'steam',
		modified_ms: 0,
		world_key: `C:/saves/1/${name}`,
		mod_profile: null,
		...overrides
	};
}

function row(name: string): HTMLElement {
	return document.querySelector(`[data-world-key="C:/saves/1/${name}"]`) as HTMLElement;
}

beforeEach(() => {
	send.mockReset();
	state().reset();
	state().profiles = {
		'client-abc': [
			profile('client-abc/default', 'Default', true),
			profile('client-abc/hard', 'Hard', false)
		]
	};
});

describe('WorldsPanel', () => {
	it('asks for the local worlds when shown', async () => {
		render(WorldsPanel, { targetId: 'client-abc', canLaunch: true });
		expect(send).toHaveBeenCalledWith('list_local_saves', { include_gamepass: true });
		expect(screen.getByText('Looking for worlds…')).toBeTruthy();

		state().loadingSaves = false;
		state().localSaves = [];
		await tick();
		expect(screen.getByText('No local worlds were found on this computer.')).toBeTruthy();
	});

	it("links a world to one of this install's profiles, or unlinks it", async () => {
		state().localSaves = [save('AAA')];
		render(WorldsPanel, { targetId: 'client-abc', canLaunch: true });
		const select = within(row('AAA')).getByLabelText('Mods profile') as HTMLSelectElement;
		expect([...select.options].map((option) => option.textContent?.trim())).toEqual([
			'No link (use the active profile)',
			'Default',
			'Hard'
		]);

		await fireEvent.change(select, { target: { value: 'client-abc/hard' } });
		await fireEvent.change(select, { target: { value: '' } });

		expect(send.mock.calls.filter(([type]) => type === 'world_profile_set')).toEqual([
			[
				'world_profile_set',
				{ world_key: 'C:/saves/1/AAA', world_name: 'AAA', profile_id: 'client-abc/hard' }
			],
			['world_profile_set', { world_key: 'C:/saves/1/AAA', world_name: 'AAA', profile_id: null }]
		]);
	});

	it('keeps showing the stored link after a refused link, and the new one once it is recorded', async () => {
		state().localSaves = [save('AAA')];
		render(WorldsPanel, { targetId: 'client-abc', canLaunch: true });
		const select = within(row('AAA')).getByLabelText('Mods profile') as HTMLSelectElement;

		await fireEvent.change(select, { target: { value: 'client-abc/hard' } });
		state().finishBusy('linkingWorld', 'C:/saves/1/AAA');
		state().recordRefusal('world_profile_set', {
			code: 'profile_not_found',
			message: 'raw',
			world_key: 'C:/saves/1/AAA',
			world_name: 'AAA'
		});
		await tick();
		expect(within(row('AAA')).getByRole('alert')).toBeTruthy();
		expect(select.value).toBe('');

		await fireEvent.change(select, { target: { value: 'client-abc/hard' } });
		state().finishBusy('linkingWorld', 'C:/saves/1/AAA');
		state().clearLastError('world_profile_set');
		state().recordWorldLink('C:/saves/1/AAA', 'client-abc/hard');
		await tick();
		expect(select.value).toBe('client-abc/hard');
		expect(select.selectedOptions[0].textContent?.trim()).toBe('Hard');
	});

	it('names a link to another install', () => {
		state().localSaves = [
			save('AAA', {
				mod_profile: { profile_id: 'client-x/pvp', profile_name: 'PvP', target_id: 'client-x' }
			})
		];
		render(WorldsPanel, { targetId: 'client-abc', canLaunch: true });
		const select = within(row('AAA')).getByLabelText('Mods profile') as HTMLSelectElement;
		expect(select.value).toBe('client-x/pvp');
		expect(select.selectedOptions[0].textContent?.trim()).toBe('PvP (another install)');
	});

	it("names each world's profile select and Launch button after the world", () => {
		state().localSaves = [
			save('AAA', { world_key: 'C:/Users/Jo Doe/saves/AAA' }),
			save('BBB', { world_key: 'C:/Users/Jo Doe/saves/BBB' })
		];
		render(WorldsPanel, { targetId: 'client-abc', canLaunch: true });

		expect(screen.getAllByRole('combobox', { name: 'AAA Mods profile' })).toHaveLength(1);
		expect(screen.getAllByRole('combobox', { name: 'BBB Mods profile' })).toHaveLength(1);
		expect(screen.getAllByRole('button', { name: 'Launch AAA' })).toHaveLength(1);
		expect(screen.getAllByRole('button', { name: 'Launch BBB' })).toHaveLength(1);
	});

	it('launches a world from its row, and offers no launch when launching is unavailable', async () => {
		state().localSaves = [save('AAA')];
		const view = render(WorldsPanel, { targetId: 'client-abc', canLaunch: true });

		await fireEvent.click(screen.getByRole('button', { name: 'Launch AAA' }));
		expect(send).toHaveBeenCalledWith('game_launch', {
			target_id: 'client-abc',
			world_key: 'C:/saves/1/AAA'
		});
		view.unmount();

		render(WorldsPanel, { targetId: 'client-abc', canLaunch: false });
		expect(screen.queryByRole('button', { name: 'Launch AAA' })).toBeNull();
	});

	it('distinguishes a Game Pass world from a Steam world with the same name', async () => {
		state().localSaves = [
			save('Home', { world_key: 'C:\\saves\\A', save_type: 'steam' }),
			save('Home', { world_key: 'gamepass:ABC', save_type: 'gamepass', path: 'C:/gp/Saved' })
		];
		render(WorldsPanel, { targetId: 'client-abc', canLaunch: true });

		expect(document.querySelector('[data-world-key="C:\\\\saves\\\\A"]')).toBeTruthy();
		expect(document.querySelector('[data-world-key="gamepass:ABC"]')).toBeTruthy();

		const gamepassRow = document.querySelector('[data-world-key="gamepass:ABC"]') as HTMLElement;
		expect(within(gamepassRow).getByText('Game Pass')).toBeTruthy();

		expect(screen.getByRole('combobox', { name: 'Home Mods profile' })).toBeTruthy();
		expect(screen.getByRole('combobox', { name: 'Home (Game Pass) Mods profile' })).toBeTruthy();

		expect(screen.getByRole('button', { name: 'Launch Home (Game Pass)' })).toBeTruthy();

		await fireEvent.change(
			screen.getByRole('combobox', { name: 'Home (Game Pass) Mods profile' }),
			{ target: { value: 'client-abc/hard' } }
		);
		expect(send).toHaveBeenCalledWith('world_profile_set', {
			world_key: 'gamepass:ABC',
			world_name: 'Home',
			profile_id: 'client-abc/hard'
		});
	});

	it('shows a listing refusal and a link refusal under its row', () => {
		state().localSaves = [save('AAA'), save('BBB')];
		state().recordRefusal('list_local_saves', {
			code: 'unknown',
			message: 'Desktop mode is required to list local saves'
		});
		state().recordRefusal('world_profile_set', {
			code: 'profile_not_found',
			message: 'raw',
			world_key: 'C:/saves/1/BBB',
			world_name: 'BBB'
		});
		render(WorldsPanel, { targetId: 'client-abc', canLaunch: true });

		expect(
			screen.getByText('Worlds could not be listed: Desktop mode is required to list local saves')
		).toBeTruthy();
		expect(within(row('AAA')).queryByRole('alert')).toBeNull();
		expect(within(row('BBB')).getByRole('alert').textContent).toContain(
			'BBB could not be linked: That profile no longer exists.'
		);
	});
});
