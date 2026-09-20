// @vitest-environment jsdom
import type { ModTarget } from '$types';
import { fireEvent, render, screen, within } from '@testing-library/svelte';
import { tick } from 'svelte';
import { beforeEach, describe, expect, it, vi } from 'vitest';

const { holder, modal, remote, platform, send } = vi.hoisted(() => ({
	holder: { state: undefined as unknown },
	modal: { showModal: vi.fn(), showConfirmModal: vi.fn() },
	remote: { active: false },
	platform: { web: false },
	send: vi.fn()
}));

vi.mock('$lib/utils/websocketUtils', () => ({
	send: (type: unknown, data?: unknown) => send(type, data),
	sendAndWait: vi.fn(),
	sendBytes: vi.fn(),
	isReady: () => true,
	pushProgressMessage: vi.fn()
}));

vi.mock('$lib/signal/remoteMode.svelte', () => ({ getRemoteMode: () => remote }));

vi.mock('$lib/utils/platform', () => ({
	get isWebBuild() {
		return platform.web;
	}
}));

vi.mock('$states', async () => {
	const { ModsState } = await import('$lib/states/modsState.svelte');
	holder.state = new ModsState();
	return {
		getModsState: () => holder.state,
		getModalState: () => modal,
		getServerState: () => ({ servers: [{ id: 7, name: 'Main World' }] })
	};
});

import type { ModsState } from '$lib/states/modsState.svelte';
import AddTargetModal from '../AddTargetModal.svelte';
import TargetList from '../TargetList.svelte';
import { modTarget } from './fixtures';

function target(overrides: Partial<ModTarget>): ModTarget {
	return modTarget({ detected: { hazards: [] }, ...overrides });
}

const state = () => holder.state as ModsState;

beforeEach(() => {
	modal.showModal.mockReset().mockResolvedValue(null);
	modal.showConfirmModal.mockReset().mockResolvedValue(false);
	send.mockReset();
	remote.active = false;
	platform.web = false;
	state().targets = [
		target({ id: 'client-abc', name: 'Palworld' }),
		target({
			id: 'server-7',
			kind: 'server',
			server_id: 7,
			name: 'old-container',
			root_path: 'C:/PalStudio/servers/main',
			platform: 'amiga'
		})
	];
});

function description(element: HTMLElement): string {
	return (element.getAttribute('aria-describedby') ?? '')
		.split(' ')
		.filter(Boolean)
		.map((id) => document.getElementById(id)?.textContent?.trim() ?? '')
		.join(' ');
}

function row(id: string): HTMLElement {
	const found = document.querySelector(`[data-target-id="${id}"]`);
	expect(found).toBeTruthy();
	return found as HTMLElement;
}

describe('TargetList', () => {
	it('renders one row per target, naming server targets after their server', () => {
		render(TargetList, { selectedId: 'client-abc', onselect: vi.fn() });

		expect(within(row('client-abc')).getByText('Palworld')).toBeTruthy();
		expect(within(row('server-7')).getByText('Main World')).toBeTruthy();
		expect(within(row('client-abc')).getByText('C:/Steam/steamapps/common/Palworld')).toBeTruthy();
	});

	it('names the platform, falling back to its code', () => {
		render(TargetList, { selectedId: 'client-abc', onselect: vi.fn() });

		expect(within(row('client-abc')).getByText('Windows')).toBeTruthy();
		expect(within(row('server-7')).getByText('amiga')).toBeTruthy();
	});

	it('offers remove on a client target but not on a server target', () => {
		render(TargetList, { selectedId: 'client-abc', onselect: vi.fn() });

		expect(within(row('client-abc')).queryByRole('button', { name: 'Remove' })).toBeTruthy();
		expect(within(row('server-7')).queryByRole('button', { name: 'Remove' })).toBeNull();
	});

	it('removes a target only after the confirmation is accepted', async () => {
		render(TargetList, { selectedId: 'client-abc', onselect: vi.fn() });
		const remove = within(row('client-abc')).getByRole('button', { name: 'Remove' });

		await fireEvent.click(remove);
		await tick();
		expect(modal.showConfirmModal).toHaveBeenCalledTimes(1);
		expect(send).not.toHaveBeenCalled();

		modal.showConfirmModal.mockResolvedValue(true);
		await fireEvent.click(remove);
		await vi.waitFor(() =>
			expect(send).toHaveBeenCalledWith('mod_target_remove', { target_id: 'client-abc' })
		);
	});

	it('labels the warning icon on a target with a hazard and describes the row with it', () => {
		state().targets = [target({ detected: { hazards: ['ue4ss_dual_instance'] } })];
		render(TargetList, { selectedId: 'client-abc', onselect: vi.fn() });

		expect(within(row('client-abc')).getByLabelText('This install has a problem')).toBeTruthy();
		const select = within(row('client-abc')).getByRole('button', { name: 'Palworld' });
		expect(description(select)).toContain('Running both crashes the game');
		const hazardText = [...row('client-abc').querySelectorAll('[hidden]')].map(
			(element) => element.textContent
		);
		expect(hazardText.join(' ')).toContain('Running both crashes the game');
	});

	it('describes each row by its kind, platform and path and marks the selected one', () => {
		render(TargetList, { selectedId: 'client-abc', onselect: vi.fn() });

		const client = within(row('client-abc')).getByRole('button', { name: 'Palworld' });
		expect(description(client)).toContain('Client');
		expect(description(client)).toContain('Windows');
		expect(description(client)).toContain('C:/Steam/steamapps/common/Palworld');
		expect(client.getAttribute('aria-current')).toBe('true');

		const server = within(row('server-7')).getByRole('button', { name: 'Main World' });
		expect(description(server)).toContain('Server');
		expect(description(server)).toContain('C:/PalStudio/servers/main');
		expect(server.hasAttribute('aria-current')).toBe(false);
	});

	it('keeps tooltips out of the row button', () => {
		state().targets = [target({ detected: { hazards: ['ue4ss_dual_instance'] } })];
		render(TargetList, { selectedId: 'client-abc', onselect: vi.fn() });

		const select = within(row('client-abc')).getByRole('button', { name: 'Palworld' });
		expect(select.querySelector('[data-tooltip-trigger]')).toBeNull();
		expect(select.querySelector('div')).toBeNull();
	});

	it('shows no warning icon on a target without hazards', () => {
		render(TargetList, { selectedId: 'client-abc', onselect: vi.fn() });

		expect(screen.queryByLabelText('This install has a problem')).toBeNull();
	});

	it('opens the add modal from Add game install and selects the added target', async () => {
		modal.showModal.mockResolvedValue('client-new');
		const onselect = vi.fn();
		render(TargetList, { selectedId: 'client-abc', onselect });

		await fireEvent.click(screen.getByRole('button', { name: /Add game install/ }));

		expect(modal.showModal).toHaveBeenCalledWith(AddTargetModal, expect.anything());
		await vi.waitFor(() => expect(onselect).toHaveBeenCalledWith('client-new'));
	});

	it('keeps the selection when the add modal closes without adding', async () => {
		const onselect = vi.fn();
		render(TargetList, { selectedId: 'client-abc', onselect });

		await fireEvent.click(screen.getByRole('button', { name: /Add game install/ }));
		await tick();

		expect(onselect).not.toHaveBeenCalled();
	});

	it('hides Add game install in a remote session and says where installs are added', () => {
		remote.active = true;
		state().targets = [];
		render(TargetList, { selectedId: undefined, onselect: vi.fn() });

		expect(screen.queryByRole('button', { name: /Add game install/ })).toBeNull();
		expect(
			screen.getByText('Game installs are added on the computer running PalStudio.')
		).toBeTruthy();
		expect(screen.queryByText('Add a game install to start managing its mods.')).toBeNull();
	});

	it('hides Add game install in the web build', () => {
		platform.web = true;
		render(TargetList, { selectedId: 'client-abc', onselect: vi.fn() });

		expect(screen.queryByRole('button', { name: /Add game install/ })).toBeNull();
	});

	it('selects a target when its row is clicked', async () => {
		const onselect = vi.fn();
		render(TargetList, { selectedId: 'client-abc', onselect });

		await fireEvent.click(within(row('server-7')).getByRole('button', { name: 'Main World' }));

		expect(onselect).toHaveBeenCalledWith('server-7');
	});
});
