// @vitest-environment jsdom
import { render } from '@testing-library/svelte';
import { describe, expect, it, vi } from 'vitest';

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

vi.mock('../ModList.svelte', () => ({ default: () => {} }));

vi.mock('$states', async () => {
	const { ModsState } = await import('$lib/states/modsState.svelte');
	holder.state = new ModsState();
	return {
		getModsState: () => holder.state,
		getModalState: () => modal,
		getServerState: () => ({ servers: [] }),
		getToastState: () => ({ add: vi.fn() })
	};
});

import type { ModsState } from '$lib/states/modsState.svelte';
import { modTarget } from './fixtures';
import TargetPanelHarness from './TargetPanelHarness.svelte';

const state = () => holder.state as ModsState;

describe('TargetPanel with the scan tab open', () => {
	it('sends a single scan while it and the scan panel both ask for one', () => {
		state().targets = [
			modTarget({
				id: 'server-7',
				kind: 'server',
				server_id: 7,
				name: 'Main World',
				root_path: '/srv/main',
				platform: 'linux',
				layout: {
					ue4ss_mods_dir: null,
					palschema_mods_dir: null,
					paks_mods_dir: '/srv/main/Pal/Content/Paks/~mods',
					logicmods_dir: '/srv/main/Pal/Content/Paks/LogicMods',
					nativemods_dir: null,
					workshop_local_dir: null,
					mods_txt: null,
					palmodsettings_ini: null
				}
			})
		];

		render(TargetPanelHarness, { targetId: 'server-7' });

		const scans = send.mock.calls.filter(([type]) => type === 'mod_target_scan');
		expect(scans).toEqual([['mod_target_scan', { target_id: 'server-7', candidates_only: true }]]);
	});
});
