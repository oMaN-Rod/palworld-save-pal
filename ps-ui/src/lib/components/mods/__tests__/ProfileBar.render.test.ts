// @vitest-environment jsdom
import type { ModProfile } from '$types';
import { fireEvent, render, screen } from '@testing-library/svelte';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import '../../live/__tests__/fixtures/animatePolyfill';

const { holder, modal, remote, send } = vi.hoisted(() => ({
	holder: { state: undefined as unknown },
	modal: { showModal: vi.fn(), showConfirmModal: vi.fn() },
	remote: { active: false },
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

vi.mock('$states', async () => {
	const { ModsState } = await import('$lib/states/modsState.svelte');
	holder.state = new ModsState();
	return { getModsState: () => holder.state, getModalState: () => modal };
});

import type { ModsState } from '$lib/states/modsState.svelte';
import ExportProfileModal from '../ExportProfileModal.svelte';
import ImportProfileModal from '../ImportProfileModal.svelte';
import ProfileBar from '../ProfileBar.svelte';
import ProfileNameModal from '../ProfileNameModal.svelte';

const state = () => holder.state as ModsState;

async function openActions() {
	await fireEvent.click(screen.getByRole('button', { name: 'Profile actions' }));
}

async function chooseAction(name: string) {
	await openActions();
	await fireEvent.click(screen.getByRole('menuitem', { name }));
}

function profile(overrides: Partial<ModProfile> = {}): ModProfile {
	return {
		id: 'client-abc/default',
		target_id: 'client-abc',
		name: 'Default',
		is_active: true,
		is_default: true,
		mods: [],
		ue4ss_control_mode: 'enabled_txt',
		force_order_ue4ss: false,
		force_order_palschema: false,
		created_at: '',
		updated_at: '',
		worlds: [],
		...overrides
	};
}

const hard = profile({
	id: 'client-abc/hard',
	name: 'Hard',
	is_active: false,
	is_default: false,
	worlds: [{ world_key: 'C:/saves/1/AAA', world_name: 'AAA' }]
});

beforeEach(() => {
	send.mockReset();
	modal.showModal.mockReset();
	modal.showConfirmModal.mockReset().mockResolvedValue(false);
	remote.active = false;
	state().reset();
	state().profiles = { 'client-abc': [profile(), hard] };
});

describe('ProfileBar', () => {
	it('renders nothing before the profiles load', () => {
		state().profiles = {};
		render(ProfileBar, { targetId: 'client-abc' });
		expect(screen.queryByLabelText('Profile')).toBeNull();
	});

	it('lists the profiles with the active one marked, and views the one chosen', async () => {
		render(ProfileBar, { targetId: 'client-abc' });
		const select = screen.getByLabelText('Profile') as HTMLSelectElement;
		expect([...select.options].map((option) => option.textContent?.trim())).toEqual([
			'Default (active)',
			'Hard'
		]);
		expect(screen.getByText('Active')).toBeTruthy();
		expect(screen.queryByRole('button', { name: 'Activate profile' })).toBeNull();

		await fireEvent.change(select, { target: { value: 'client-abc/hard' } });

		expect(state().viewedProfile('client-abc')?.id).toBe('client-abc/hard');
		expect(screen.queryByText('Active')).toBeNull();
		expect(screen.getByText('Used by worlds: AAA')).toBeTruthy();
	});

	it('keeps profile management in a menu until it is opened', async () => {
		render(ProfileBar, { targetId: 'client-abc' });
		const trigger = screen.getByRole('button', { name: 'Profile actions' });
		expect(trigger.getAttribute('aria-haspopup')).toBe('menu');
		expect(screen.queryByRole('menuitem', { name: 'New profile' })).toBeNull();

		await openActions();

		expect(screen.getAllByRole('menuitem').map((item) => item.textContent?.trim())).toEqual([
			'New profile',
			'Rename profile',
			'Export profile',
			'Import profile',
			'Delete profile'
		]);
		expect(document.activeElement).toBe(screen.getByRole('menuitem', { name: 'New profile' }));

		await fireEvent.keyDown(screen.getByRole('menu'), { key: 'Escape' });
		expect(document.activeElement).toBe(trigger);
	});

	it('activates the viewed profile', async () => {
		state().viewProfile('client-abc', 'client-abc/hard');
		render(ProfileBar, { targetId: 'client-abc' });

		await fireEvent.click(screen.getByRole('button', { name: 'Activate profile' }));

		expect(send).toHaveBeenCalledWith('profile_activate', {
			target_id: 'client-abc',
			profile_id: 'client-abc/hard'
		});
		expect(state().activating['client-abc']).toBe(true);
	});

	it('creates a profile from the name dialog, copying the viewed one when asked', async () => {
		modal.showModal.mockResolvedValue({ name: 'Brutal', copy: true });
		render(ProfileBar, { targetId: 'client-abc' });

		await chooseAction('New profile');

		await vi.waitFor(() =>
			expect(send).toHaveBeenCalledWith('profile_create', {
				target_id: 'client-abc',
				name: 'Brutal',
				copy_from: 'client-abc/default'
			})
		);
		expect(modal.showModal.mock.calls[0][0]).toBe(ProfileNameModal);
		expect(modal.showModal.mock.calls[0][1]).toMatchObject({ copyFrom: 'Default' });
	});

	it('creates an empty profile when copying is off, and nothing when the dialog is cancelled', async () => {
		modal.showModal
			.mockResolvedValueOnce({ name: 'Empty', copy: false })
			.mockResolvedValueOnce(null);
		render(ProfileBar, { targetId: 'client-abc' });

		await chooseAction('New profile');
		await vi.waitFor(() =>
			expect(send).toHaveBeenCalledWith('profile_create', {
				target_id: 'client-abc',
				name: 'Empty'
			})
		);
		await chooseAction('New profile');
		await vi.waitFor(() => expect(modal.showModal).toHaveBeenCalledTimes(2));

		expect(send.mock.calls.filter(([type]) => type === 'profile_create')).toHaveLength(1);
	});

	it('renames the viewed profile', async () => {
		modal.showModal.mockResolvedValue({ name: 'Harder', copy: false });
		state().viewProfile('client-abc', 'client-abc/hard');
		render(ProfileBar, { targetId: 'client-abc' });

		await chooseAction('Rename profile');

		await vi.waitFor(() =>
			expect(send).toHaveBeenCalledWith('profile_rename', {
				target_id: 'client-abc',
				profile_id: 'client-abc/hard',
				name: 'Harder'
			})
		);
		expect(modal.showModal.mock.calls[0][1]).toMatchObject({ initialName: 'Hard' });
		expect(modal.showModal.mock.calls[0][1].copyFrom).toBeUndefined();
	});

	it('cannot delete the default profile', async () => {
		render(ProfileBar, { targetId: 'client-abc' });
		await openActions();
		expect(
			(screen.getByRole('menuitem', { name: 'Delete profile' }) as HTMLButtonElement).disabled
		).toBe(true);
		expect(screen.getByText("The default profile can't be deleted.")).toBeTruthy();
	});

	it('deletes another profile only after confirmation', async () => {
		state().viewProfile('client-abc', 'client-abc/hard');
		render(ProfileBar, { targetId: 'client-abc' });

		await chooseAction('Delete profile');
		await vi.waitFor(() => expect(modal.showConfirmModal).toHaveBeenCalledTimes(1));
		expect(send).not.toHaveBeenCalledWith('profile_delete', expect.anything());

		modal.showConfirmModal.mockResolvedValue(true);
		await chooseAction('Delete profile');
		await vi.waitFor(() =>
			expect(send).toHaveBeenCalledWith('profile_delete', {
				target_id: 'client-abc',
				profile_id: 'client-abc/hard'
			})
		);
	});

	it('shows profile refusals in words', () => {
		state().recordRefusal(
			'profile_create',
			{ code: 'name_taken', message: 'raw', profile_id: 'client-abc/hard' },
			'client-abc'
		);
		render(ProfileBar, { targetId: 'client-abc' });

		expect(screen.getByRole('alert').textContent).toContain(
			'Another profile on this install already has that name.'
		);
	});

	it('renders refusals from two message types sharing the same code', () => {
		state().recordRefusal(
			'profile_rename',
			{ code: 'profile_not_found', message: 'raw' },
			'client-abc'
		);
		state().recordRefusal(
			'profile_activate',
			{ code: 'profile_not_found', message: 'raw' },
			'client-abc'
		);
		render(ProfileBar, { targetId: 'client-abc' });

		expect(screen.getAllByRole('alert')).toHaveLength(2);
	});

	it('opens export for the viewed profile and import for this install', async () => {
		modal.showModal.mockResolvedValue(undefined);
		state().viewProfile('client-abc', 'client-abc/hard');
		render(ProfileBar, { targetId: 'client-abc' });

		await chooseAction('Export profile');
		await chooseAction('Import profile');

		expect(modal.showModal.mock.calls[0][0]).toBe(ExportProfileModal);
		expect(modal.showModal.mock.calls[0][1]).toEqual({
			targetId: 'client-abc',
			profileId: 'client-abc/hard',
			profileName: 'Hard'
		});
		expect(modal.showModal.mock.calls[1][0]).toBe(ImportProfileModal);
		expect(modal.showModal.mock.calls[1][1]).toEqual({ targetId: 'client-abc' });
		expect(send).not.toHaveBeenCalled();
	});

	it('hides Export in a remote session but keeps Import', async () => {
		remote.active = true;
		render(ProfileBar, { targetId: 'client-abc' });
		await openActions();
		expect(screen.queryByRole('menuitem', { name: 'Export profile' })).toBeNull();
		expect(screen.getByRole('menuitem', { name: 'Import profile' })).toBeTruthy();
	});
});

describe('ProfileNameModal', () => {
	it('returns the trimmed name and the copy choice', async () => {
		const closeModal = vi.fn();
		render(ProfileNameModal, {
			title: 'New profile',
			confirmText: 'Create',
			copyFrom: 'Default',
			closeModal
		});
		const create = screen.getByRole('button', { name: 'Create' }) as HTMLButtonElement;
		expect(create.disabled).toBe(true);

		await fireEvent.input(screen.getByLabelText('Name'), { target: { value: '  Brutal  ' } });
		await fireEvent.click(screen.getByRole('checkbox', { name: 'Start with the mods of Default' }));
		await fireEvent.click(create);

		expect(closeModal).toHaveBeenCalledWith({ name: 'Brutal', copy: true });
	});

	it('starts from the current name when renaming and offers no copy', async () => {
		const closeModal = vi.fn();
		render(ProfileNameModal, {
			title: 'Rename profile',
			confirmText: 'Rename',
			initialName: 'Hard',
			closeModal
		});

		expect((screen.getByLabelText('Name') as HTMLInputElement).value).toBe('Hard');
		expect(screen.queryByRole('checkbox')).toBeNull();

		await fireEvent.input(screen.getByLabelText('Name'), { target: { value: 'a'.repeat(65) } });
		expect((screen.getByRole('button', { name: 'Rename' }) as HTMLButtonElement).disabled).toBe(
			true
		);
		expect(screen.getByText('Use 1 to 64 characters.')).toBeTruthy();

		await fireEvent.click(screen.getByRole('button', { name: 'Cancel' }));
		expect(closeModal).toHaveBeenCalledWith(null);
	});
});
