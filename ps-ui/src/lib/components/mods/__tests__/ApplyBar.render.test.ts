// @vitest-environment jsdom
import type { ApplyResult as ApplyReply, PlanOp } from '$types';
import { MessageType } from '$types';
import { fireEvent, render, screen } from '@testing-library/svelte';
import { tick } from 'svelte';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import type { ModsTab } from '../TargetPanel.svelte';

const { holder, modal, send, remote, toasts } = vi.hoisted(() => ({
	holder: { state: undefined as unknown },
	modal: { showModal: vi.fn(), showConfirmModal: vi.fn() },
	send: vi.fn(),
	remote: { active: false },
	toasts: { add: vi.fn() }
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
	return {
		getModsState: () => holder.state,
		getModalState: () => modal,
		getToastState: () => toasts
	};
});

import type { ModsState } from '$lib/states/modsState.svelte';
import ApplyBar from '../ApplyBar.svelte';
import ApplyResultModal from '../ApplyResultModal.svelte';

const state = () => holder.state as ModsState;
const targetId = 'client-abc';
const notice = 'The game is running. Your changes are saved and will be applied when it stops.';

const zero: Record<PlanOp, number> = {
	keep: 0,
	reattribute: 0,
	replace: 0,
	preserve: 0,
	add: 0,
	move: 0,
	remove: 0,
	remove_preserve: 0
};

function setPlan(counts: Partial<Record<PlanOp, number>>) {
	state().plans = {
		...state().plans,
		[targetId]: { profile_id: 'p', counts: { ...zero, ...counts }, entries: [] }
	};
}

function reply(overrides: Partial<ApplyReply> = {}): ApplyReply {
	return {
		target_id: targetId,
		request_id: 'req-1',
		mid_apply: true,
		failed: [],
		preserved: [],
		backup_dir: null,
		needs_attention: [],
		counts: zero,
		new_copies: [],
		skipped_new_copies: [],
		...overrides
	};
}

function applyButton(): HTMLButtonElement {
	return screen.getByRole('button', { name: /^(Loading )?Apply$/ }) as HTMLButtonElement;
}

let onShowTab: ReturnType<typeof vi.fn<(tab: ModsTab) => void>>;

function show() {
	return render(ApplyBar, { props: { targetId, onShowTab } });
}

beforeEach(() => {
	send.mockClear();
	modal.showConfirmModal.mockReset();
	modal.showModal.mockReset();
	toasts.add.mockReset();
	onShowTab = vi.fn();
	remote.active = false;
	state().reset();
});

describe('ApplyBar', () => {
	it('summarises pending changes by kind, leaving out kept files', () => {
		setPlan({ keep: 40, add: 3, replace: 1, remove: 2 });
		show();
		expect(screen.getByText('Pending changes: 6')).toBeTruthy();
		expect(screen.getByText('3 to add, 1 to update, 2 to remove')).toBeTruthy();
		expect(applyButton().disabled).toBe(false);
	});

	it('counts reattribute as update, and names moves and forgotten edited files', () => {
		setPlan({ reattribute: 2, move: 1, remove_preserve: 1 });
		show();
		expect(screen.getByText('Pending changes: 4')).toBeTruthy();
		expect(screen.getByText('2 to update, 1 to move, 1 kept because you edited them')).toBeTruthy();
		expect(applyButton().disabled).toBe(false);
	});

	it('stays out of the way while nothing is pending', () => {
		setPlan({ keep: 12 });
		show();
		expect(screen.queryByRole('region', { name: 'Apply changes' })).toBeNull();
		expect(screen.queryByRole('button', { name: 'Apply' })).toBeNull();
	});

	it('treats files kept for your edits as nothing to apply', () => {
		setPlan({ keep: 3, preserve: 2 });
		show();
		expect(screen.queryByRole('button', { name: 'Apply' })).toBeNull();
	});

	it('stays hidden until the plan arrives', () => {
		show();
		expect(screen.queryByRole('button', { name: 'Apply' })).toBeNull();
	});

	it('floats as a labelled region once changes are pending', () => {
		setPlan({ add: 1 });
		show();
		const region = screen.getByRole('region', { name: 'Apply changes' });
		expect(region.contains(applyButton())).toBe(true);
	});

	it('mentions files kept for your edits beside pending changes', () => {
		setPlan({ add: 1, preserve: 2 });
		show();
		expect(screen.getByText('Files kept because you edited them: 2')).toBeTruthy();
	});

	it('applies the target', async () => {
		setPlan({ add: 1 });
		show();
		await fireEvent.click(applyButton());
		expect(send).toHaveBeenCalledWith(MessageType.PROFILE_APPLY, { target_id: targetId });
	});

	it('shows progress with the stage in words, naming the progress bar', async () => {
		setPlan({ add: 1 });
		show();
		state().progress = {
			'req-1': { request_id: 'req-1', target_id: targetId, stage: 'checking', pct: 0, message: '' }
		};
		await tick();
		expect(screen.getByText('Checking whether the game is running…')).toBeTruthy();
		state().progress = {
			'req-1': { request_id: 'req-1', target_id: targetId, stage: 'applying', pct: 10, message: '' }
		};
		await tick();
		const stage = screen.getByText('Applying…');
		expect(stage.getAttribute('aria-live')).toBe('polite');
		const group = screen.getByRole('group', { name: 'Applying…' });
		expect((group.querySelector('progress') as HTMLProgressElement).value).toBe(10);
	});

	it('labels a downloading framework stage', async () => {
		setPlan({ add: 1 });
		show();
		state().progress = {
			'req-1': {
				request_id: 'req-1',
				target_id: targetId,
				stage: 'downloading',
				pct: 20,
				message: ''
			}
		};
		await tick();
		expect(screen.getByText('Downloading')).toBeTruthy();
	});

	it('stays hidden while a toggle applies in the background', async () => {
		setPlan({ keep: 3 });
		show();
		state().progress = {
			'req-3': { request_id: 'req-3', target_id: targetId, stage: 'applying', pct: 30, message: '' }
		};
		await tick();
		expect(screen.queryByRole('region', { name: 'Apply changes' })).toBeNull();
	});

	it('ignores progress for another target', async () => {
		setPlan({ add: 1 });
		show();
		state().progress = {
			'req-2': { request_id: 'req-2', target_id: 'other', stage: 'applying', pct: 50, message: '' }
		};
		await tick();
		expect(screen.queryByText('Applying…')).toBeNull();
		expect(applyButton().disabled).toBe(false);
	});

	it('renders a refused plan inline and still allows applying', async () => {
		show();
		state().recordRefusal(
			MessageType.PROFILE_PLAN,
			{ code: 'no_active_profile', message: 'raw' },
			targetId
		);
		await tick();
		expect(screen.getByText('This target has no active profile.')).toBeTruthy();
		expect(applyButton().disabled).toBe(false);
	});

	it('does not render another target’s plan refusal', async () => {
		show();
		state().recordRefusal(MessageType.PROFILE_PLAN, { code: 'db', message: 'elsewhere' }, 'other');
		await tick();
		expect(screen.queryByText('elsewhere')).toBeNull();
	});

	it("keeps this target's plan refusal when another target's plan is refused", async () => {
		show();
		state().recordRefusal(
			MessageType.PROFILE_PLAN,
			{ code: 'no_active_profile', message: 'raw' },
			targetId
		);
		state().recordRefusal(MessageType.PROFILE_PLAN, { code: 'db', message: 'elsewhere' }, 'other');
		await tick();

		expect(screen.getByText('This target has no active profile.')).toBeTruthy();
		expect(applyButton().disabled).toBe(false);
	});

	describe('a plan refused for unmanaged occupants', () => {
		const paths = ['C:/Palworld/Mods/Stray/main.lua'];

		function refusePlan() {
			state().recordRefusal(
				MessageType.PROFILE_PLAN,
				{ code: 'unmanaged_occupant', message: 'raw', paths },
				targetId
			);
		}

		it('lists the paths with Adopt and Replace', async () => {
			modal.showConfirmModal.mockResolvedValue(true);
			show();
			refusePlan();
			await tick();
			expect(
				screen.getByText('These files are in the way and were not installed by PalStudio:')
			).toBeTruthy();
			expect(screen.getByText(paths[0])).toBeTruthy();
			await fireEvent.click(screen.getByRole('button', { name: 'Adopt them' }));
			expect(onShowTab).toHaveBeenCalledWith('scan');
			await fireEvent.click(screen.getByRole('button', { name: 'Replace them' }));
			await vi.waitFor(() =>
				expect(send).toHaveBeenCalledWith(MessageType.PROFILE_APPLY, {
					target_id: targetId,
					replace_occupants: paths
				})
			);
		});

		it('keeps its own actions while the same outcome opens in a modal', async () => {
			show();
			refusePlan();
			state().recordApply(
				reply({
					mid_apply: false,
					error: { code: 'unmanaged_occupant', message: 'raw', paths }
				})
			);
			await tick();
			expect(screen.getAllByRole('button', { name: 'Adopt them' })).toHaveLength(1);
			expect(modal.showModal).toHaveBeenCalledOnce();
		});
	});

	it('shows the pending notice for a change saved while the game ran, with one Apply', async () => {
		show();
		expect(screen.queryByText(notice)).toBeNull();
		state().pending = { [targetId]: true };
		await tick();
		expect(screen.getByText(notice)).toBeTruthy();
		expect(screen.getAllByRole('button', { name: 'Apply' })).toHaveLength(1);
		expect(applyButton().disabled).toBe(false);
	});

	it('does not show another target’s pending notice', async () => {
		show();
		state().pending = { other: true };
		await tick();
		expect(screen.queryByText(notice)).toBeNull();
	});

	describe('the outcome of an apply', () => {
		it('opens a notable outcome in a modal once, when it arrives', async () => {
			setPlan({ add: 1 });
			show();
			const outcome = reply();
			state().recordApply(outcome);
			await tick();

			expect(modal.showModal).toHaveBeenCalledOnce();
			expect(modal.showModal.mock.calls[0][0]).toBe(ApplyResultModal);
			expect(modal.showModal.mock.calls[0][1]).toEqual({ result: outcome, targetId, onShowTab });

			setPlan({ add: 2 });
			await tick();
			expect(modal.showModal).toHaveBeenCalledOnce();
			expect(toasts.add).not.toHaveBeenCalled();
		});

		it('toasts a plain success of an apply started here instead of opening a modal', async () => {
			setPlan({ add: 2, remove: 1 });
			show();
			state().apply(targetId);
			await tick();
			state().recordApply(
				reply({ mid_apply: false, counts: { ...zero, keep: 5, add: 2, remove: 1 } })
			);
			await tick();

			expect(toasts.add).toHaveBeenCalledWith('Changes applied: 3', undefined, 'success');
			expect(modal.showModal).not.toHaveBeenCalled();
		});

		it('says nothing about a plain success a toggle applied in the background', async () => {
			show();
			state().recordApply(reply({ mid_apply: false, counts: { ...zero, add: 1 } }));
			await tick();

			expect(toasts.add).not.toHaveBeenCalled();
			expect(modal.showModal).not.toHaveBeenCalled();
		});

		it('waits for a running toggle before announcing the outcome', async () => {
			setPlan({ add: 1 });
			show();
			state().settingMod = { [targetId]: true };
			state().recordApply(reply());
			await tick();
			expect(modal.showModal).not.toHaveBeenCalled();

			state().settingMod = { [targetId]: false };
			await tick();
			expect(modal.showModal).toHaveBeenCalledOnce();
		});

		it('does not announce an outcome that arrived before it was shown', async () => {
			state().recordApply(reply());
			show();
			await tick();
			expect(modal.showModal).not.toHaveBeenCalled();
		});

		it('does not announce an older outcome of a target it switches to', async () => {
			state().recordApply(reply({ target_id: 'other', request_id: 'req-old' }));
			const view = show();
			await view.rerender({ targetId: 'other', onShowTab });
			await tick();
			expect(modal.showModal).not.toHaveBeenCalled();

			state().recordApply(reply({ target_id: 'other', request_id: 'req-new' }));
			await tick();
			expect(modal.showModal).toHaveBeenCalledOnce();
		});
	});

	it('disables Apply while a toggle applies', async () => {
		setPlan({ add: 1 });
		show();
		state().settingMod = { [targetId]: true };
		await tick();
		expect(applyButton().disabled).toBe(true);
	});

	it('disables Apply while progress runs for this target without a request of its own', async () => {
		setPlan({ add: 1 });
		show();
		state().progress = {
			'req-9': { request_id: 'req-9', target_id: targetId, stage: 'applying', pct: 40, message: '' }
		};
		await tick();
		expect(applyButton().disabled).toBe(true);
	});
});
