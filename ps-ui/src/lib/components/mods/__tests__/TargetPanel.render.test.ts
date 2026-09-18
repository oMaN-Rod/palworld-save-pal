// @vitest-environment jsdom
import type { ModTarget, TargetLayoutJson } from '$types';
import { MessageType } from '$types';
import { fireEvent, render, screen } from '@testing-library/svelte';
import { tick } from 'svelte';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import '../../live/__tests__/fixtures/animatePolyfill';

const { env, holder, modal, remote, remoteMock, send } = vi.hoisted(() => {
	const remote = { active: false };
	return {
		env: { desktop: 'false' },
		holder: { state: undefined as unknown },
		modal: { showModal: vi.fn(), showConfirmModal: vi.fn() },
		remote,
		remoteMock: vi.fn(() => remote),
		send: vi.fn()
	};
});

vi.mock('$lib/utils/websocketUtils', () => ({
	send: (type: unknown, data?: unknown) => send(type, data),
	sendAndWait: vi.fn(),
	sendBytes: vi.fn(),
	isReady: () => true,
	pushProgressMessage: vi.fn()
}));

vi.mock('../ModList.svelte', () => ({ default: () => {} }));
vi.mock('../LoadOrderPanel.svelte', () => ({ default: () => {} }));
vi.mock('../ScanPanel.svelte', () => ({ default: () => {} }));
vi.mock('../FrameworksPanel.svelte', () => ({ default: () => {} }));
vi.mock('../LivePanel.svelte', () => ({ default: () => {} }));
vi.mock('../DiscoverPanel.svelte', () => ({ default: () => {} }));
vi.mock('$env/static/public', () => ({
	get PUBLIC_DESKTOP_MODE() {
		return env.desktop;
	},
	PUBLIC_WS_URL: 'localhost:0/ws'
}));
vi.mock('$lib/signal/remoteMode.svelte', async (importOriginal) => {
	const actual = await importOriginal<typeof import('$lib/signal/remoteMode.svelte')>();
	return { ...actual, getRemoteMode: remoteMock };
});
vi.mock('../WorldsPanel.svelte', () => ({ default: () => {} }));

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
import { RemoteModeState } from '$lib/signal/remoteMode.svelte';
import { modTarget } from './fixtures';
import TargetPanelHarness from './TargetPanelHarness.svelte';

const layout: TargetLayoutJson = {
	ue4ss_mods_dir: null,
	palschema_mods_dir: null,
	paks_mods_dir: 'C:/Palworld/Pal/Content/Paks/~mods',
	logicmods_dir: 'C:/Palworld/Pal/Content/Paks/LogicMods',
	nativemods_dir: null,
	workshop_local_dir: null,
	mods_txt: null,
	palmodsettings_ini: null
};

function target(overrides: Partial<ModTarget>): ModTarget {
	return modTarget({ root_path: 'C:/Palworld', layout, ...overrides });
}

const state = () => holder.state as ModsState;

beforeEach(() => {
	env.desktop = 'false';
	remote.active = false;
	send.mockReset();
	modal.showConfirmModal.mockReset().mockResolvedValue(false);
	state().reset();
	state().targets = [
		target({}),
		target({ id: 'server-7', kind: 'server', server_id: 7, name: 'Main World' })
	];
});

describe('TargetPanel', () => {
	it('loads the profiles and plan of its target', () => {
		render(TargetPanelHarness, { targetId: 'client-abc' });

		expect(send).toHaveBeenCalledWith('profile_list', { target_id: 'client-abc' });
		expect(send).toHaveBeenCalledWith('profile_plan', { target_id: 'client-abc' });
	});

	it('shows the profile bar once the target has profiles', async () => {
		render(TargetPanelHarness, { targetId: 'client-abc' });
		expect(screen.queryByLabelText('Profile')).toBeNull();

		state().profiles = {
			'client-abc': [
				{
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
					updated_at: ''
				}
			]
		};
		await tick();

		expect(screen.getByLabelText('Profile')).toBeTruthy();
	});

	it("keeps the profile bar's action names distinct from a backup set's Delete", async () => {
		render(TargetPanelHarness, { targetId: 'client-abc' });
		state().profiles = {
			'client-abc': [
				{
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
					updated_at: ''
				},
				{
					id: 'client-abc/hard',
					target_id: 'client-abc',
					name: 'Hard',
					is_active: false,
					is_default: false,
					mods: [],
					ue4ss_control_mode: 'enabled_txt',
					force_order_ue4ss: false,
					force_order_palschema: false,
					created_at: '',
					updated_at: ''
				}
			]
		};
		state().viewProfile('client-abc', 'client-abc/hard');
		state().backups = {
			'client-abc': [{ name: '20260101-000000', size_bytes: 0, entries: [] }]
		};
		await fireEvent.click(screen.getByRole('button', { name: 'Open backups from outside' }));
		await tick();
		await fireEvent.click(screen.getByRole('button', { name: 'Profile actions' }));

		expect(screen.getAllByRole('menuitem', { name: 'Delete profile' })).toHaveLength(1);
		expect(screen.getAllByRole('button', { name: 'Delete' })).toHaveLength(1);
		expect(screen.getAllByRole('menuitem', { name: 'New profile' })).toHaveLength(1);
		expect(screen.getAllByRole('menuitem', { name: 'Rename profile' })).toHaveLength(1);
		expect(screen.getAllByRole('button', { name: 'Activate profile' })).toHaveLength(1);
	});

	it('offers Launch on a game install in the desktop app only', async () => {
		env.desktop = 'true';
		const client = render(TargetPanelHarness, { targetId: 'client-abc' });
		await fireEvent.click(screen.getByRole('button', { name: 'Launch' }));
		expect(send).toHaveBeenCalledWith('game_launch', { target_id: 'client-abc' });
		client.unmount();

		const server = render(TargetPanelHarness, { targetId: 'server-7' });
		expect(screen.queryByRole('button', { name: 'Launch' })).toBeNull();
		server.unmount();

		remote.active = true;
		const remoteView = render(TargetPanelHarness, { targetId: 'client-abc' });
		expect(screen.queryByRole('button', { name: 'Launch' })).toBeNull();
		remoteView.unmount();

		remote.active = false;
		env.desktop = 'false';
		render(TargetPanelHarness, { targetId: 'client-abc' });
		expect(screen.queryByRole('button', { name: 'Launch' })).toBeNull();
	});

	it('shows the Worlds tab for a game install in the desktop app or a remote session', () => {
		const plain = render(TargetPanelHarness, { targetId: 'client-abc' });
		expect(screen.queryByRole('tab', { name: 'Worlds' })).toBeNull();
		plain.unmount();

		env.desktop = 'true';
		const desktop = render(TargetPanelHarness, { targetId: 'client-abc' });
		expect(screen.getByRole('tab', { name: 'Worlds' })).toBeTruthy();
		desktop.unmount();

		const server = render(TargetPanelHarness, { targetId: 'server-7' });
		expect(screen.queryByRole('tab', { name: 'Worlds' })).toBeNull();
		server.unmount();

		env.desktop = 'false';
		remote.active = true;
		render(TargetPanelHarness, { targetId: 'client-abc' });
		expect(screen.getByRole('tab', { name: 'Worlds' })).toBeTruthy();
	});

	it('shows the Discover tab only in the desktop app outside a remote session', () => {
		const plain = render(TargetPanelHarness, { targetId: 'client-abc' });
		expect(screen.queryByRole('tab', { name: 'Discover' })).toBeNull();
		plain.unmount();

		remote.active = true;
		const remoteView = render(TargetPanelHarness, { targetId: 'client-abc' });
		expect(screen.queryByRole('tab', { name: 'Discover' })).toBeNull();
		remoteView.unmount();

		remote.active = false;
		env.desktop = 'true';
		render(TargetPanelHarness, { targetId: 'client-abc' });
		expect(screen.getByRole('tab', { name: 'Discover' })).toBeTruthy();
	});

	it('snaps back to the Mods tab when the Discover tab loses its gate', async () => {
		const session = { connected: true };
		const remoteInstance = new RemoteModeState({
			getSession: () => session as never,
			createTransport: () => ({ dispose: vi.fn() }) as never,
			setTransportDelegate: vi.fn(),
			resetTransportDelegate: vi.fn()
		});
		remoteMock.mockReturnValueOnce(remoteInstance);

		env.desktop = 'true';
		render(TargetPanelHarness, { targetId: 'client-abc' });
		await fireEvent.click(screen.getByRole('tab', { name: 'Discover' }));
		await tick();
		expect(screen.getByTestId('tab').textContent).toBe('discover');

		remoteInstance.enter();
		await tick();

		expect(screen.getByTestId('tab').textContent).toBe('mods');
	});

	it('lists the Frameworks, Conflicts and Live tabs for a client target', () => {
		render(TargetPanelHarness, { targetId: 'client-abc' });

		expect(screen.getByRole('tab', { name: 'Frameworks' })).toBeTruthy();
		expect(screen.getByRole('tab', { name: 'Conflicts' })).toBeTruthy();
		expect(screen.getByRole('tab', { name: 'Live' })).toBeTruthy();
	});

	it('lists no Live tab for a server target', () => {
		render(TargetPanelHarness, { targetId: 'server-7' });

		expect(screen.getByRole('tab', { name: 'Frameworks' })).toBeTruthy();
		expect(screen.getByRole('tab', { name: 'Conflicts' })).toBeTruthy();
		expect(screen.queryByRole('tab', { name: 'Live' })).toBeNull();
	});

	it('shows a move-to-backup button for a hazard with a fix, and sends the request', async () => {
		state().targets = [target({ detected: { hazards: ['workshop_proxy_dll'] } })];
		render(TargetPanelHarness, { targetId: 'client-abc' });

		const move = screen.getByRole('button', {
			name: 'Move to backup: A dwmapi.dll or xinput1_3.dll sits beside the Workshop UE4SS install and would start UE4SS twice.'
		});
		await fireEvent.click(move);

		expect(send).toHaveBeenCalledWith('framework_hazard_remove', {
			target_id: 'client-abc',
			hazard: 'workshop_proxy_dll'
		});
	});

	it("keeps the hazard outcome visible once the moved hazard was the target's only one", async () => {
		state().targets = [target({ detected: { hazards: ['workshop_proxy_dll'] } })];
		render(TargetPanelHarness, { targetId: 'client-abc' });
		state().lastHazardRemove = {
			'client-abc': {
				target_id: 'client-abc',
				hazard: 'workshop_proxy_dll',
				moved: ['dwmapi.dll'],
				backup_dir: 'C:/backups/hazards'
			}
		};
		await tick();
		expect(screen.getByText('Moved 1 files to C:/backups/hazards.')).toBeTruthy();

		state().targets = [target({ detected: { hazards: [] } })];
		await tick();

		expect(screen.getByText('Moved 1 files to C:/backups/hazards.')).toBeTruthy();
	});

	it('shows the apply status in the header and the apply bar while changes are pending', async () => {
		render(TargetPanelHarness, { targetId: 'client-abc' });
		expect(screen.queryByRole('region', { name: 'Apply changes' })).toBeNull();

		state().plans = {
			'client-abc': {
				profile_id: 'client-abc/default',
				counts: {
					keep: 0,
					reattribute: 0,
					replace: 0,
					preserve: 0,
					add: 2,
					move: 0,
					remove: 0,
					remove_preserve: 0
				},
				entries: []
			}
		};
		await tick();

		expect(screen.getByText('2 pending')).toBeTruthy();
		expect(screen.getByRole('region', { name: 'Apply changes' })).toBeTruthy();
		expect(screen.getAllByRole('button', { name: 'Apply' })).toHaveLength(1);
	});

	it('names the kind of install beside its name', () => {
		const view = render(TargetPanelHarness, { targetId: 'client-abc' });
		expect(screen.getByRole('heading', { name: 'Palworld Client' })).toBeTruthy();
		view.unmount();

		render(TargetPanelHarness, { targetId: 'server-7' });
		expect(screen.getByRole('heading', { name: 'Main World Server' })).toBeTruthy();
	});

	it('offers Remove behind a confirmation on a client target only', async () => {
		const { unmount } = render(TargetPanelHarness, { targetId: 'client-abc' });

		await fireEvent.click(screen.getByRole('button', { name: 'Install actions' }));
		await fireEvent.click(screen.getByRole('menuitem', { name: 'Remove' }));
		await tick();
		expect(modal.showConfirmModal).toHaveBeenCalledTimes(1);
		expect(send).not.toHaveBeenCalledWith('mod_target_remove', expect.anything());
		unmount();

		render(TargetPanelHarness, { targetId: 'server-7' });
		expect(screen.queryByRole('button', { name: 'Install actions' })).toBeNull();
	});

	it('lists only the layout directories that exist', async () => {
		render(TargetPanelHarness, { targetId: 'client-abc' });
		expect(screen.queryByText('Pak mods')).toBeNull();

		await fireEvent.click(screen.getByRole('button', { name: 'Where mods go' }));

		expect(screen.getByRole('heading', { name: 'Where mods go' })).toBeTruthy();
		expect(screen.getByText('Pak mods')).toBeTruthy();
		expect(screen.getByText('C:/Palworld/Pal/Content/Paks/LogicMods')).toBeTruthy();
		expect(screen.queryByText('UE4SS mods')).toBeNull();
		expect(screen.queryByText('PalModSettings.ini')).toBeNull();
	});

	it('shows hazards in words in a warning card', () => {
		state().targets = [target({ detected: { hazards: ['ue4ss_dual_instance', 'new_hazard'] } })];
		render(TargetPanelHarness, { targetId: 'client-abc' });

		expect(screen.getByText('Problems with this install')).toBeTruthy();
		expect(screen.getByText(/Running both crashes the game/)).toBeTruthy();
		expect(screen.getByText('new_hazard')).toBeTruthy();
	});

	it('shows no hazard card without hazards', () => {
		render(TargetPanelHarness, { targetId: 'client-abc' });

		expect(screen.queryByText('Problems with this install')).toBeNull();
	});

	it('binds the active tab both ways', async () => {
		render(TargetPanelHarness, { targetId: 'client-abc' });
		const output = screen.getByTestId('tab');
		expect(output.textContent).toBe('scan');

		await fireEvent.click(screen.getByRole('tab', { name: 'Mods' }));
		await tick();
		expect(output.textContent).toBe('mods');

		await fireEvent.click(screen.getByRole('button', { name: 'Open backups from outside' }));
		await tick();
		expect(screen.getByRole('tab', { name: 'Backups' }).className).not.toContain(
			'text-surface-400'
		);
		expect(screen.getByRole('tab', { name: 'Mods' }).className).toContain('text-surface-400');
	});

	it('switches to the tab the apply bar asks for', async () => {
		state().recordRefusal(
			MessageType.PROFILE_PLAN,
			{ code: 'unmanaged_occupant', message: 'raw', paths: ['Pal/Content/Paks/~mods/a.pak'] },
			'client-abc'
		);
		render(TargetPanelHarness, { targetId: 'client-abc' });
		await fireEvent.click(screen.getByRole('tab', { name: 'Mods' }));
		await tick();
		const output = screen.getByTestId('tab');
		expect(output.textContent).toBe('mods');

		await fireEvent.click(screen.getByRole('button', { name: 'Adopt them' }));
		await tick();

		expect(output.textContent).toBe('scan');
		expect(screen.getByRole('tab', { name: 'Scan' }).className).not.toContain('text-surface-400');
	});

	it('scans its target once on mount', () => {
		render(TargetPanelHarness, { targetId: 'server-7' });

		const scans = send.mock.calls.filter(([type]) => type === 'mod_target_scan');
		expect(scans).toEqual([['mod_target_scan', { target_id: 'server-7', candidates_only: true }]]);
	});

	it('counts subscribed Workshop mods on the scan tab', async () => {
		render(TargetPanelHarness, { targetId: 'server-7' });
		expect(screen.getByRole('tab', { name: 'Scan' })).toBeTruthy();

		state().candidates = {
			'server-7': [
				{ name: 'A', root: 'a', source: 'steam_subscribed' },
				{ name: 'B', root: 'b', source: 'steam_subscribed' },
				{ name: 'C', root: 'c', source: 'loose' }
			] as never
		};
		await tick();

		expect(screen.getByRole('tab', { name: 'Scan (2)' })).toBeTruthy();
	});

	it('loads targets and the library while its target is missing', async () => {
		state().targets = [];
		render(TargetPanelHarness, { targetId: 'server-9' });

		expect(screen.getByText("Preparing this server's mods…")).toBeTruthy();
		expect(send).toHaveBeenCalledWith('mod_target_list', undefined);
		expect(send).toHaveBeenCalledWith('mod_list', undefined);

		state().targets = [target({ id: 'server-9', kind: 'server', server_id: 9 })];
		await tick();
		expect(screen.queryByText("Preparing this server's mods…")).toBeNull();
	});

	it('does not reload targets when its target is present', () => {
		render(TargetPanelHarness, { targetId: 'client-abc' });

		expect(send).not.toHaveBeenCalledWith('mod_target_list', undefined);
	});

	it('does not reload targets again while the target stays missing', async () => {
		state().targets = [];
		render(TargetPanelHarness, { targetId: 'server-9' });
		state().targets = [target({})];
		state().targetsLoaded = true;
		await tick();

		const lists = send.mock.calls.filter(([type]) => type === 'mod_target_list');
		expect(lists).toHaveLength(1);
	});

	it('says the server has no mods target once a requested list arrives without it', async () => {
		state().targets = [];
		render(TargetPanelHarness, { targetId: 'server-9' });
		expect(screen.getByText("Preparing this server's mods…")).toBeTruthy();

		state().targets = [target({})];
		state().targetsLoaded = true;
		await tick();

		expect(screen.getByText('This server has no mods target yet.')).toBeTruthy();
		expect(screen.queryByText("Preparing this server's mods…")).toBeNull();
	});

	it('keeps preparing while only a list from before its request is loaded', () => {
		state().targets = [target({})];
		state().targetsLoaded = true;
		render(TargetPanelHarness, { targetId: 'server-9' });

		expect(screen.getByText("Preparing this server's mods…")).toBeTruthy();
		expect(screen.queryByText('This server has no mods target yet.')).toBeNull();
	});

	it('warns about an unreadable PalModSettings.ini outside the scan tab', async () => {
		const unreadable =
			'PalModSettings.ini could not be read, so Steam Workshop mods cannot be listed.';
		state().scanWarnings = { 'server-7': ['palmodsettings_unreadable'] };
		render(TargetPanelHarness, { targetId: 'server-7' });
		expect(screen.queryByText(unreadable)).toBeNull();

		await fireEvent.click(screen.getByRole('tab', { name: 'Mods' }));
		await tick();

		expect(screen.getByText(unreadable)).toBeTruthy();
	});

	it('marks its tabs as a tab list with the open one selected', async () => {
		render(TargetPanelHarness, { targetId: 'client-abc' });

		expect(screen.getByRole('tablist')).toBeTruthy();
		const scan = screen.getByRole('tab', { name: 'Scan' });
		expect(scan.getAttribute('aria-selected')).toBe('true');
		expect(screen.getByRole('tab', { name: 'Mods' }).getAttribute('aria-selected')).toBe('false');
		const panel = screen.getByRole('tabpanel');
		expect(scan.getAttribute('aria-controls')).toBe(panel.id);
		expect(panel.getAttribute('aria-labelledby')).toBe(scan.id);
		expect(panel.getAttribute('tabindex')).toBe('0');

		await fireEvent.click(screen.getByRole('button', { name: 'Open backups from outside' }));
		await tick();

		const backups = screen.getByRole('tab', { name: 'Backups' });
		expect(backups.getAttribute('aria-selected')).toBe('true');
		expect(screen.getByRole('tabpanel').getAttribute('aria-labelledby')).toBe(backups.id);
	});

	it('moves between tabs with the arrow keys', async () => {
		render(TargetPanelHarness, { targetId: 'client-abc' });

		await fireEvent.keyDown(screen.getByRole('tab', { name: 'Scan' }), { key: 'ArrowLeft' });
		await tick();

		const live = screen.getByRole('tab', { name: 'Live' });
		expect(screen.getByTestId('tab').textContent).toBe('live');
		expect(live.getAttribute('tabindex')).toBe('0');
		expect(document.activeElement).toBe(live);
		expect(screen.getByRole('tab', { name: 'Scan' }).getAttribute('tabindex')).toBe('-1');
	});

	it('shows a scan refusal above the tabs while the scan tab is closed', async () => {
		state().recordRefusal(
			MessageType.MOD_TARGET_SCAN,
			{ code: 'scan_failed', message: 'layout broke' },
			'client-abc'
		);
		render(TargetPanelHarness, { targetId: 'client-abc' });
		expect(screen.queryByText(/layout broke/)).toBeNull();

		await fireEvent.click(screen.getByRole('tab', { name: 'Mods' }));
		await tick();

		expect(screen.getByRole('alert').textContent).toContain('layout broke');
	});

	describe('after a reconnect', () => {
		const sentOf = (type: string) => send.mock.calls.filter(([sent]) => sent === type);

		it('sends each load once on first mount', async () => {
			render(TargetPanelHarness, { targetId: 'server-7' });
			state().targets = [...state().targets];
			state().targetsLoaded = true;
			await tick();

			expect(sentOf('profile_list')).toHaveLength(1);
			expect(sentOf('profile_plan')).toHaveLength(1);
			expect(sentOf('mod_target_scan')).toHaveLength(1);
		});

		it('sends its profiles, plan and unanswered scan again after a remote reconnect', async () => {
			state().connectionChanged(true, 'remote');
			render(TargetPanelHarness, { targetId: 'server-7' });
			state().connectionChanged(false, 'remote');
			send.mockReset();

			state().connectionChanged(true, 'remote');
			await tick();

			expect(send.mock.calls).toEqual([
				['profile_list', { target_id: 'server-7' }],
				['profile_plan', { target_id: 'server-7' }],
				['mod_target_scan', { target_id: 'server-7', candidates_only: true }]
			]);
		});

		it('repeats a full scan as a full scan', async () => {
			state().connectionChanged(true);
			render(TargetPanelHarness, { targetId: 'server-7' });
			state().completeScan('server-7');
			state().scan('server-7', { full: true });
			state().completeScan('server-7');
			state().recordScan('server-7', { files: [] }, true);
			state().connectionChanged(false);
			state().connectionChanged(true);
			send.mockReset();
			await tick();

			expect(sentOf('mod_target_scan')).toEqual([
				['mod_target_scan', { target_id: 'server-7', candidates_only: false }]
			]);
		});

		it('waits for a target the reset removed before loading it again', async () => {
			render(TargetPanelHarness, { targetId: 'server-7' });
			send.mockReset();

			state().reset();
			await tick();
			expect(sentOf('profile_list')).toEqual([]);

			state().targets = [target({ id: 'server-7', kind: 'server', server_id: 7 })];
			await tick();
			expect(sentOf('profile_list')).toEqual([['profile_list', { target_id: 'server-7' }]]);
			expect(sentOf('profile_plan')).toHaveLength(1);
			expect(sentOf('mod_target_scan')).toHaveLength(1);
		});
	});
});
