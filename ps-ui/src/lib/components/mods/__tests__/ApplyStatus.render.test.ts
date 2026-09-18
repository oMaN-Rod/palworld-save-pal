// @vitest-environment jsdom
import type { PlanOp } from '$types';
import { MessageType } from '$types';
import { fireEvent, render, screen } from '@testing-library/svelte';
import { tick } from 'svelte';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import type { ModsTab } from '../TargetPanel.svelte';

const { holder, modal } = vi.hoisted(() => ({
	holder: { state: undefined as unknown },
	modal: { showModal: vi.fn(), showConfirmModal: vi.fn() }
}));

vi.mock('$lib/utils/websocketUtils', () => ({
	send: vi.fn(),
	sendAndWait: vi.fn(),
	sendBytes: vi.fn(),
	isReady: () => true,
	pushProgressMessage: vi.fn()
}));

vi.mock('$states', async () => {
	const { ModsState } = await import('$lib/states/modsState.svelte');
	holder.state = new ModsState();
	return { getModsState: () => holder.state, getModalState: () => modal };
});

import type { ModsState } from '$lib/states/modsState.svelte';
import ApplyResultModal from '../ApplyResultModal.svelte';
import ApplyStatus from '../ApplyStatus.svelte';
import { applyResult, noOps } from './fixtures';

const state = () => holder.state as ModsState;
const targetId = 'client-abc';

let onShowTab: ReturnType<typeof vi.fn<(tab: ModsTab) => void>>;

function setPlan(counts: Partial<Record<PlanOp, number>>) {
	state().plans = {
		...state().plans,
		[targetId]: { profile_id: 'p', counts: { ...noOps, ...counts }, entries: [] }
	};
}

function show() {
	return render(ApplyStatus, { props: { targetId, onShowTab } });
}

beforeEach(() => {
	modal.showModal.mockReset();
	onShowTab = vi.fn();
	state().reset();
});

describe('ApplyStatus', () => {
	it('says nothing before the plan arrives', () => {
		const { container } = show();
		expect(container.textContent?.trim()).toBe('');
	});

	it('says the target is up to date', () => {
		setPlan({ keep: 4, preserve: 1 });
		show();
		expect(screen.getByText('Up to date')).toBeTruthy();
	});

	it('counts pending changes', () => {
		setPlan({ add: 2, remove: 1 });
		show();
		expect(screen.getByText('3 pending')).toBeTruthy();
	});

	it('marks a change saved while the game ran as pending', async () => {
		setPlan({ keep: 1 });
		show();
		state().pending = { [targetId]: true };
		await tick();
		expect(screen.getByText('Pending')).toBeTruthy();
	});

	it('says an apply is running', async () => {
		setPlan({ add: 1 });
		show();
		state().applying = { [targetId]: true };
		await tick();
		expect(screen.getByText('Applying…')).toBeTruthy();
	});

	it('says a refused plan blocks applying', async () => {
		show();
		state().recordRefusal(MessageType.PROFILE_PLAN, { code: 'db', message: 'raw' }, targetId);
		await tick();
		expect(screen.getByText('Blocked')).toBeTruthy();
	});

	it('reopens a notable last outcome, and offers nothing for a plain success', async () => {
		setPlan({ keep: 1 });
		state().recordApply(applyResult({ target_id: targetId, counts: { ...noOps, add: 1 } }));
		const view = show();
		expect(screen.queryByRole('button', { name: 'Last apply result' })).toBeNull();
		view.unmount();

		const outcome = applyResult({ target_id: targetId, backup_dir: 'C:/backups/1' });
		state().recordApply(outcome);
		show();
		await fireEvent.click(screen.getByRole('button', { name: 'Last apply result' }));

		expect(modal.showModal).toHaveBeenCalledWith(ApplyResultModal, {
			result: outcome,
			targetId,
			onShowTab
		});
	});
});
