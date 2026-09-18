// @vitest-environment jsdom
import { MessageType } from '$types';
import { fireEvent, render, screen } from '@testing-library/svelte';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import type { ModsTab } from '../TargetPanel.svelte';

const { holder, modal, send } = vi.hoisted(() => ({
	holder: { state: undefined as unknown },
	modal: { showModal: vi.fn(), showConfirmModal: vi.fn() },
	send: vi.fn()
}));

vi.mock('$lib/utils/websocketUtils', () => ({
	send: (type: unknown, data?: unknown) => send(type, data),
	sendAndWait: vi.fn(),
	sendBytes: vi.fn(),
	isReady: () => true,
	pushProgressMessage: vi.fn()
}));

vi.mock('$lib/signal/remoteMode.svelte', () => ({ getRemoteMode: () => ({ active: false }) }));

vi.mock('$states', async () => {
	const { ModsState } = await import('$lib/states/modsState.svelte');
	holder.state = new ModsState();
	return { getModsState: () => holder.state, getModalState: () => modal };
});

import type { ModsState } from '$lib/states/modsState.svelte';
import type { ApplyResult as ApplyReply } from '$types';
import ApplyResultModal from '../ApplyResultModal.svelte';
import { applyResult, noOps } from './fixtures';

const state = () => holder.state as ModsState;
const targetId = 'client-abc';

let onShowTab: ReturnType<typeof vi.fn<(tab: ModsTab) => void>>;
let closeModal: ReturnType<typeof vi.fn>;

function show(overrides: Partial<ApplyReply>) {
	return render(ApplyResultModal, {
		props: {
			result: applyResult({ target_id: targetId, ...overrides }),
			targetId,
			onShowTab,
			closeModal
		}
	});
}

beforeEach(() => {
	send.mockClear();
	modal.showConfirmModal.mockReset();
	onShowTab = vi.fn();
	closeModal = vi.fn();
	state().reset();
});

describe('ApplyResultModal', () => {
	it('titles an apply that finished with notes and counts what it changed', () => {
		show({ counts: { ...noOps, add: 4, remove: 1 }, preserved: ['C:/Mods/A/config.lua'] });

		expect(screen.getByRole('heading', { name: 'Applied with notes' })).toBeTruthy();
		expect(screen.getByText('Added').nextElementSibling?.textContent).toBe('4');
		expect(screen.getByText('Removed').nextElementSibling?.textContent).toBe('1');
		expect(screen.getByText('C:/Mods/A/config.lua')).toBeTruthy();
	});

	it('titles a refused apply without counts', () => {
		show({ error: { code: 'target_locked', message: 'raw' }, counts: { ...noOps, add: 2 } });

		expect(screen.getByRole('heading', { name: 'Apply stopped' })).toBeTruthy();
		expect(screen.queryByText('Added')).toBeNull();
	});

	it('titles an apply that stopped midway', () => {
		show({ mid_apply: true });
		expect(screen.getByRole('heading', { name: "Apply didn't finish" })).toBeTruthy();
	});

	it('closes on Done', async () => {
		show({ backup_dir: 'C:/backups/1' });
		await fireEvent.click(screen.getByRole('button', { name: 'Done' }));
		expect(closeModal).toHaveBeenCalledOnce();
	});

	it('closes before switching to the tab an action asks for', async () => {
		show({ backup_dir: 'C:/backups/1' });
		await fireEvent.click(screen.getByRole('button', { name: 'View backups' }));

		expect(closeModal).toHaveBeenCalledOnce();
		expect(onShowTab).toHaveBeenCalledWith('backups');
	});

	it('closes when an action applies again', async () => {
		show({ mid_apply: true });
		await fireEvent.click(screen.getByRole('button', { name: 'Apply again' }));

		expect(send).toHaveBeenCalledWith(MessageType.PROFILE_APPLY, { target_id: targetId });
		expect(closeModal).toHaveBeenCalledOnce();
	});

	it('offers no inline Dismiss inside the modal', () => {
		show({ mid_apply: true });
		expect(screen.queryByRole('button', { name: 'Dismiss' })).toBeNull();
	});
});
