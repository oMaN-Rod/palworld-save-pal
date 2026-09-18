// @vitest-environment jsdom
import { MessageType } from '$types';
import { fireEvent, render, screen } from '@testing-library/svelte';
import { tick } from 'svelte';
import { beforeEach, describe, expect, it, vi } from 'vitest';

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

vi.mock('$states', async () => {
	const { ModsState } = await import('$lib/states/modsState.svelte');
	holder.state = new ModsState();
	return {
		getModsState: () => holder.state,
		getModalState: () => modal
	};
});

import type { ModsState } from '$lib/states/modsState.svelte';
import FrameworksPanel from '../FrameworksPanel.svelte';
import { libraryMod } from './fixtures';

const state = () => holder.state as ModsState;
const targetId = 't1';

const status = {
	target_id: 't1',
	ue4ss_mode: 'standard',
	hazards: [],
	frameworks: [
		{
			key: 'ue4ss',
			name: 'UE4SS',
			installed: {
				present: true,
				version: '3.0.0',
				managed: true,
				mod_version_id: 'framework-ue4ss@3.0.0'
			},
			library: [],
			latest: { version: '3.0.1', display: '3.0.1', in_library: false },
			latest_error: null,
			update_available: true
		},
		{
			key: 'palschema',
			name: 'PalSchema',
			installed: { present: false, version: null, managed: false, mod_version_id: null },
			library: [
				{
					mod_version_id: 'framework-palschema@0.4.0',
					version: '0.4.0',
					display: '0.4.0',
					installed_at: '',
					is_current: true
				}
			],
			latest: null,
			latest_error: null,
			update_available: false
		},
		{
			key: 'amity',
			name: 'Amity',
			installed: { present: true, version: 'unknown', managed: false, mod_version_id: null },
			library: [],
			latest: null,
			latest_error: null,
			update_available: false
		}
	]
} as never;

beforeEach(() => {
	send.mockReset();
	modal.showConfirmModal.mockReset().mockResolvedValue(false);
	state().reset();
});

describe('FrameworksPanel', () => {
	it('asks for status on open and renders one row per framework', async () => {
		render(FrameworksPanel, { targetId });
		expect(send).toHaveBeenCalledWith('framework_status', { target_id: 't1' });
		state().frameworks = { t1: status };
		await tick();
		expect(screen.getByRole('button', { name: 'Update UE4SS' })).toBeTruthy();
		expect(screen.getByRole('button', { name: 'Install PalSchema' })).toBeTruthy();
		expect(screen.queryByRole('button', { name: 'Remove Amity' })).toBeNull();
		expect(screen.queryByRole('button', { name: 'Update Amity' })).toBeNull();
		expect(screen.getByText('Not installed by PalStudio')).toBeTruthy();
	});

	it('checks GitHub only on request', async () => {
		render(FrameworksPanel, { targetId });
		await fireEvent.click(screen.getByRole('button', { name: 'Check for updates' }));
		expect(send).toHaveBeenCalledWith('framework_status', { target_id: 't1', check_latest: true });
	});

	it('confirms removal, and offers a forced removal after framework_required', async () => {
		modal.showConfirmModal.mockResolvedValue(true);
		state().frameworks = { t1: status };
		render(FrameworksPanel, { targetId });
		await fireEvent.click(screen.getByRole('button', { name: 'Remove UE4SS' }));
		await tick();
		expect(send).toHaveBeenCalledWith('framework_remove', { target_id: 't1', key: 'ue4ss' });
		state().finishBusy('frameworkBusy', 't1');
		state().recordRefusal(
			MessageType.FRAMEWORK_REMOVE,
			{
				code: 'framework_required',
				message: 'm',
				framework: 'ue4ss',
				dependents: ['CoolMod'],
				key: 'ue4ss'
			},
			't1'
		);
		await tick();
		expect(screen.getByRole('alert').textContent).toContain('CoolMod');
		await fireEvent.click(screen.getByRole('button', { name: 'Remove UE4SS anyway' }));
		await tick();
		expect(send).toHaveBeenCalledWith('framework_remove', {
			target_id: 't1',
			key: 'ue4ss',
			force: true
		});
	});

	it('clears a stale install refusal on a new remove attempt, revealing the real one', async () => {
		modal.showConfirmModal.mockResolvedValue(true);
		state().frameworks = { t1: status };
		state().recordRefusal(
			MessageType.FRAMEWORK_INSTALL,
			{ code: 'network', message: 'stale install failure', key: 'ue4ss' },
			't1'
		);
		state().recordRefusal(
			MessageType.FRAMEWORK_REMOVE,
			{ code: 'framework_conflict', message: 'stale remove failure', key: 'ue4ss' },
			't1'
		);
		render(FrameworksPanel, { targetId });

		await fireEvent.click(screen.getByRole('button', { name: 'Remove UE4SS' }));
		await tick();
		state().finishBusy('frameworkBusy', 't1');
		state().recordRefusal(
			MessageType.FRAMEWORK_REMOVE,
			{
				code: 'framework_required',
				message: 'm',
				framework: 'ue4ss',
				dependents: ['CoolMod'],
				key: 'ue4ss'
			},
			't1'
		);
		await tick();

		expect(screen.getByRole('button', { name: 'Remove UE4SS anyway' })).toBeTruthy();
	});

	it('disables every framework action while one runs, and renders a Docker refusal', async () => {
		state().frameworks = { t1: status };
		state().installFramework('t1', 'palschema');
		render(FrameworksPanel, { targetId });
		for (const name of ['Update UE4SS', 'Remove UE4SS', 'Check for updates']) {
			expect((screen.getByRole('button', { name }) as HTMLButtonElement).disabled).toBe(true);
		}
		state().recordRefusal(
			MessageType.FRAMEWORK_STATUS,
			{ code: 'not_supported_on_target', message: 'd' },
			't1'
		);
		await tick();
		expect(
			screen.getByText('Frameworks for Docker servers are managed by the server image.')
		).toBeTruthy();
	});

	it('reloads status when the selected target changes, and again on a reset', async () => {
		const { rerender } = render(FrameworksPanel, { targetId: 't1' });
		expect(send).toHaveBeenCalledWith('framework_status', { target_id: 't1' });
		send.mockReset();

		await rerender({ targetId: 't2' });
		expect(send).toHaveBeenCalledWith('framework_status', { target_id: 't2' });
		send.mockReset();

		state().reset();
		await tick();
		expect(send).toHaveBeenCalledWith('framework_status', { target_id: 't2' });
	});

	it("shows Remove UE4SS anyway under UE4SS only, matched by the refusal's echoed key", async () => {
		state().frameworks = { t1: status };
		state().recordRefusal(
			MessageType.FRAMEWORK_REMOVE,
			{
				code: 'framework_required',
				message: 'm',
				framework: 'ue4ss',
				dependents: ['CoolMod'],
				key: 'ue4ss'
			},
			't1'
		);
		render(FrameworksPanel, { targetId });

		expect(screen.getByRole('button', { name: 'Remove UE4SS anyway' })).toBeTruthy();
		expect(screen.queryByRole('button', { name: 'Remove PalSchema anyway' })).toBeNull();
	});

	it('keeps a stored framework_required refusal visible after remounting the panel', async () => {
		state().frameworks = { t1: status };
		state().recordRefusal(
			MessageType.FRAMEWORK_REMOVE,
			{
				code: 'framework_required',
				message: 'm',
				framework: 'ue4ss',
				dependents: ['CoolMod'],
				key: 'ue4ss'
			},
			't1'
		);
		const first = render(FrameworksPanel, { targetId });
		expect(screen.getByRole('button', { name: 'Remove UE4SS anyway' })).toBeTruthy();
		first.unmount();

		render(FrameworksPanel, { targetId });
		expect(screen.getByRole('button', { name: 'Remove UE4SS anyway' })).toBeTruthy();
	});

	it('shows an install-side framework_required refusal without a Remove anyway', async () => {
		state().frameworks = { t1: status };
		state().recordRefusal(
			MessageType.FRAMEWORK_INSTALL,
			{ code: 'framework_required', message: 'm', framework: 'ue4ss', key: 'palschema' },
			't1'
		);
		render(FrameworksPanel, { targetId });

		const card = screen.getByText('PalSchema').closest('[data-framework="palschema"]');
		expect(card?.textContent).toContain('Install UE4SS first.');
		expect(screen.queryByRole('button', { name: 'Remove PalSchema anyway' })).toBeNull();
	});

	it('resolves dependent ids to their display names', async () => {
		state().frameworks = { t1: status };
		state().mods = [libraryMod({ id: 'cool-mod', name: 'Cool Mod' })];
		state().recordRefusal(
			MessageType.FRAMEWORK_REMOVE,
			{
				code: 'framework_required',
				message: 'm',
				framework: 'ue4ss',
				dependents: ['palschema', 'cool-mod'],
				key: 'ue4ss'
			},
			't1'
		);
		render(FrameworksPanel, { targetId });

		expect(screen.getByRole('alert').textContent).toContain('PalSchema, Cool Mod');
	});

	it('installs a chosen library version', async () => {
		state().frameworks = { t1: status };
		render(FrameworksPanel, { targetId });
		await fireEvent.change(screen.getByLabelText('PalSchema version to install'), {
			target: { value: 'framework-palschema@0.4.0' }
		});
		await fireEvent.click(screen.getByRole('button', { name: 'Install PalSchema' }));
		expect(send).toHaveBeenCalledWith('framework_install', {
			target_id: 't1',
			key: 'palschema',
			mod_version_id: 'framework-palschema@0.4.0'
		});
	});
});
