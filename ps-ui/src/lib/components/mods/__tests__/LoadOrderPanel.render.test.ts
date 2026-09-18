// @vitest-environment jsdom
import type { ModProfile, ProfileMod } from '$types';
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
import LoadOrderPanel from '../LoadOrderPanel.svelte';
import { libraryMod } from './fixtures';

const state = () => holder.state as ModsState;

function entry(mod_id: string, load_order: number, enabled = true): ProfileMod {
	return { profile_id: 't/default', mod_id, mod_version_id: null, enabled, load_order };
}

function profile(overrides: Partial<ModProfile> = {}): ModProfile {
	return {
		id: 't/default',
		target_id: 't',
		name: 'Default',
		is_active: true,
		is_default: true,
		mods: [entry('alpha', 0), entry('schema', 1), entry('beta', 2, false), entry('pak', 3)],
		ue4ss_control_mode: 'enabled_txt',
		force_order_ue4ss: false,
		force_order_palschema: false,
		created_at: '',
		updated_at: '',
		...overrides
	};
}

function ue4ssRows(): string[] {
	const section = screen.getByRole('list', { name: 'UE4SS mods' });
	return within(section)
		.getAllByRole('listitem')
		.map((item) => item.getAttribute('data-mod-id') ?? '');
}

beforeEach(() => {
	send.mockReset();
	state().reset();
	state().mods = [
		libraryMod({ id: 'alpha', name: 'Alpha', mod_type: 'ue4ss' }),
		libraryMod({ id: 'beta', name: 'Beta', mod_type: 'hybrid' }),
		libraryMod({ id: 'schema', name: 'Schema', mod_type: 'palschema' }),
		libraryMod({ id: 'pak', name: 'Paky', mod_type: 'pak' })
	];
	state().profiles = { t: [profile()] };
});

describe('LoadOrderPanel', () => {
	it('lists each orderable kind in load order and pak mods read-only', () => {
		render(LoadOrderPanel, { targetId: 't' });

		expect(ue4ssRows()).toEqual(['alpha', 'beta']);
		expect(
			within(screen.getByRole('list', { name: 'UE4SS mods' })).getByText('Disabled')
		).toBeTruthy();
		const schema = screen.getByRole('list', { name: 'PalSchema mods' });
		expect(within(schema).getByText('Schema')).toBeTruthy();
		const paks = screen.getByRole('list', { name: 'Pak and logic mods' });
		expect(within(paks).getByText('Paky')).toBeTruthy();
		expect(within(paks).queryByRole('button')).toBeNull();
	});

	it('moves a mod down, sending the whole new order and showing it at once', async () => {
		render(LoadOrderPanel, { targetId: 't' });
		expect(
			(screen.getByRole('button', { name: 'Move Alpha up' }) as HTMLButtonElement).disabled
		).toBe(true);

		await fireEvent.click(screen.getByRole('button', { name: 'Move Alpha down' }));
		await tick();

		expect(send).toHaveBeenCalledWith('profile_reorder', {
			target_id: 't',
			profile_id: 't/default',
			kind: 'ue4ss',
			ordered_mod_ids: ['beta', 'alpha']
		});
		expect(ue4ssRows()).toEqual(['beta', 'alpha']);
	});

	it('moves a mod with Alt and the arrow keys', async () => {
		render(LoadOrderPanel, { targetId: 't' });
		const beta = document.querySelector('[data-mod-id="beta"]') as HTMLElement;

		await fireEvent.keyDown(beta, { key: 'ArrowUp', altKey: true });

		expect(send).toHaveBeenCalledWith('profile_reorder', {
			target_id: 't',
			profile_id: 't/default',
			kind: 'ue4ss',
			ordered_mod_ids: ['beta', 'alpha']
		});
	});

	it('ignores arrow keys without Alt', async () => {
		render(LoadOrderPanel, { targetId: 't' });
		await fireEvent.keyDown(document.querySelector('[data-mod-id="beta"]') as HTMLElement, {
			key: 'ArrowUp'
		});
		expect(send).not.toHaveBeenCalled();
	});

	it('allows one order change at a time', () => {
		state().ordering = { t: true };
		render(LoadOrderPanel, { targetId: 't' });
		expect(
			(screen.getByRole('button', { name: 'Move Alpha down' }) as HTMLButtonElement).disabled
		).toBe(true);
		expect((screen.getByLabelText('Force PalSchema order') as HTMLInputElement).disabled).toBe(
			true
		);
	});

	it('waits for the library before allowing moves', () => {
		state().mods = state().mods.filter((mod) => mod.id !== 'pak');
		render(LoadOrderPanel, { targetId: 't' });
		expect(screen.getByText('Loading the library…')).toBeTruthy();
		expect(
			(screen.getByRole('button', { name: 'Move Alpha down' }) as HTMLButtonElement).disabled
		).toBe(true);
	});

	it('sends each option change', async () => {
		render(LoadOrderPanel, { targetId: 't' });

		await fireEvent.click(screen.getByLabelText('Force PalSchema order'));
		await fireEvent.click(screen.getByLabelText('Force UE4SS order'));
		await fireEvent.change(screen.getByLabelText('UE4SS control mode'), {
			target: { value: 'mods_txt' }
		});

		expect(send.mock.calls).toEqual([
			[
				'profile_set_options',
				{ target_id: 't', profile_id: 't/default', force_order_palschema: true }
			],
			['profile_set_options', { target_id: 't', profile_id: 't/default', force_order_ue4ss: true }],
			[
				'profile_set_options',
				{ target_id: 't', profile_id: 't/default', ue4ss_control_mode: 'mods_txt' }
			]
		]);
	});

	it('shows a stale-order refusal in words', () => {
		state().recordRefusal(
			'profile_reorder',
			{ code: 'invalid_order', message: 'raw', expected: [] },
			't'
		);
		render(LoadOrderPanel, { targetId: 't' });
		expect(screen.getByRole('alert').textContent).toContain('The load order changed');
	});

	it('renders refusals from two message types sharing the same code', () => {
		state().recordRefusal('profile_reorder', { code: 'target_not_found', message: 'raw' }, 't');
		state().recordRefusal('profile_set_options', { code: 'target_not_found', message: 'raw' }, 't');
		render(LoadOrderPanel, { targetId: 't' });

		expect(screen.getAllByRole('alert')).toHaveLength(2);
	});

	it("marks a profile that isn't active", () => {
		state().profiles = {
			t: [profile({ is_active: false }), profile({ id: 't/other', name: 'Other' })]
		};
		state().viewProfile('t', 't/default');
		render(LoadOrderPanel, { targetId: 't' });
		expect(screen.getByText(/You're editing Default/)).toBeTruthy();
	});
});
