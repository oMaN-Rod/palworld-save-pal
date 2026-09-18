// @vitest-environment jsdom
import type { AdoptionCandidate, ModError, ScanReport } from '$types';
import { MessageType } from '$types';
import { fireEvent, render, screen, within } from '@testing-library/svelte';
import { tick } from 'svelte';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

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
import ScanPanel from '../ScanPanel.svelte';
import { modTarget } from './fixtures';

const state = () => holder.state as ModsState;
const targetId = 'client-abc';
const allManaged = 'Everything here is managed by PalStudio.';

const target = modTarget({ id: targetId, root_path: 'C:/Palworld' });

const localCandidate: AdoptionCandidate = {
	name: 'BetterUI',
	kind: 'ue4ss',
	root: 'C:/Palworld/Mods/BetterUI',
	files: ['C:/Palworld/Mods/BetterUI/main.lua', 'C:/Palworld/Mods/BetterUI/enabled.txt'],
	enabled: true,
	source: 'local'
};

const subscribedCandidate: AdoptionCandidate = {
	name: 'FastTravelAnywhere',
	kind: 'workshop',
	root: 'C:/Steam/workshop/content/1623730/123456',
	files: [],
	enabled: false,
	source: 'steam_subscribed'
};

const cleanFiles: ScanReport['files'] = [
	{ path: 'a.pak', kind: 'pak', state: 'managed_intact', mod_version_id: 'v1' }
];

function report(overrides: Partial<ScanReport> = {}): ScanReport {
	return {
		target_id: targetId,
		files: [],
		candidates: [],
		drifted: [],
		missing: [],
		unreadable: [],
		warnings: [],
		...overrides
	};
}

function setScan(scan: ScanReport) {
	state().recordScan(targetId, scan, true);
}

function quickScan(candidates: AdoptionCandidate[]) {
	state().recordScan(targetId, { candidates, warnings: [] }, false);
}

/** Mirrors the server, which echoes the candidate a refused adopt named. */
function refuseAdopt(candidate: AdoptionCandidate, error: ModError) {
	state().finishBusy('adopting', targetId);
	state().recordRefusal(MessageType.MOD_ADOPT, error, targetId, {
		subject: { candidate_name: candidate.name, source: candidate.source, root: candidate.root }
	});
}

function show() {
	return render(ScanPanel, { props: { targetId } });
}

function countOf(container: HTMLElement, fileState: string): string {
	return container.querySelector(`[data-state="${fileState}"] [data-count]`)?.textContent ?? '';
}

function scanSends(): number {
	return send.mock.calls.filter(([type]) => type === MessageType.MOD_TARGET_SCAN).length;
}

beforeEach(() => {
	send.mockClear();
	state().reset();
	state().targets = [target];
});

afterEach(() => {
	vi.restoreAllMocks();
});

describe('ScanPanel', () => {
	it("shows no scan time from the target record, which detection stamps and scans don't", () => {
		state().targets = [{ ...target, last_scanned_at: '2026-09-13 14:22:33' }];
		const { container } = show();

		expect(container.textContent).not.toContain(
			new Date(Date.UTC(2026, 8, 13, 14, 22, 33)).toLocaleString()
		);
		expect(container.textContent).not.toContain('2026-09-13 14:22:33');
	});

	it('scans candidates once per session, not on every mount', async () => {
		const { unmount } = show();
		unmount();
		show();
		await tick();

		expect(scanSends()).toBe(1);
		expect(send).toHaveBeenCalledWith(MessageType.MOD_TARGET_SCAN, {
			target_id: targetId,
			candidates_only: true
		});
	});

	it('runs a full scan from the Scan button', async () => {
		show();
		send.mockClear();
		await fireEvent.click(screen.getByRole('button', { name: 'Scan now' }));
		expect(send).toHaveBeenCalledWith(MessageType.MOD_TARGET_SCAN, {
			target_id: targetId,
			candidates_only: false
		});
	});

	it('shows when the last full scan arrived', async () => {
		const arrived = Date.UTC(2026, 8, 13, 14, 22, 33);
		vi.spyOn(Date, 'now').mockReturnValue(arrived);
		const { container } = show();
		expect(container.textContent).not.toContain(new Date(arrived).toLocaleString());

		setScan(report({ files: cleanFiles }));
		await tick();

		expect(container.textContent).toContain(new Date(arrived).toLocaleString());
	});

	it('counts files per state and lists the problem paths with what happens next', () => {
		setScan(
			report({
				files: [
					{ path: 'a.pak', kind: 'pak', state: 'managed_intact', mod_version_id: 'v1' },
					{ path: 'b.pak', kind: 'pak', state: 'managed_intact', mod_version_id: 'v1' },
					{ path: 'c.lua', kind: 'ue4ss', state: 'managed_drifted', mod_version_id: 'v2' },
					{ path: 'd.pak', kind: 'pak', state: 'managed_missing', mod_version_id: 'v3' },
					{ path: 'e.txt', kind: null, state: 'unmanaged', mod_version_id: null },
					{ path: 'f.txt', kind: null, state: 'unmanaged', mod_version_id: null },
					{ path: 'g.txt', kind: null, state: 'unmanaged', mod_version_id: null }
				],
				drifted: ['c.lua'],
				missing: ['d.pak']
			})
		);
		const { container } = show();

		expect(countOf(container, 'managed_intact')).toBe('2');
		expect(countOf(container, 'managed_drifted')).toBe('1');
		expect(countOf(container, 'managed_missing')).toBe('1');
		expect(countOf(container, 'managed_unreadable')).toBe('0');
		expect(countOf(container, 'unmanaged')).toBe('3');
		expect(container.querySelector('[data-state="managed_drifted"]')?.textContent).toContain(
			'Edited'
		);
		expect(container.querySelector('[data-state="unmanaged"]')?.textContent).toContain(
			'Not managed'
		);

		expect(
			screen.getByText('Edited since PalStudio installed it — your edit is kept on the next apply')
		).toBeTruthy();
		expect(screen.getByText('c.lua')).toBeTruthy();
		expect(screen.getByText('Missing — the next apply puts it back')).toBeTruthy();
		expect(screen.getByText('d.pak')).toBeTruthy();
		expect(screen.queryByText('Unreadable — check permissions')).toBeNull();
		expect(screen.queryByText(allManaged)).toBeNull();
	});

	it('offers Adopt for a local candidate with its kind, file count and state', async () => {
		setScan(report({ candidates: [localCandidate] }));
		show();

		const row = screen.getByText('BetterUI').closest('li') as HTMLElement;
		expect(within(row).getByText('UE4SS')).toBeTruthy();
		expect(within(row).getByText('Files: 2')).toBeTruthy();
		expect(within(row).getByText('Enabled')).toBeTruthy();
		expect(within(row).queryByRole('button', { name: 'Manage in PalStudio' })).toBeNull();
		expect(
			screen.getByText(
				'Adopting records these files as a PalStudio mod without changing anything the game sees.'
			)
		).toBeTruthy();

		send.mockClear();
		await fireEvent.click(within(row).getByRole('button', { name: 'Adopt' }));
		expect(send).toHaveBeenCalledWith(MessageType.MOD_ADOPT, {
			target_id: targetId,
			candidate_name: 'BetterUI',
			source: 'local',
			root: 'C:/Palworld/Mods/BetterUI'
		});
	});

	it('offers Manage in PalStudio for a subscribed candidate, without a file count', async () => {
		setScan(report({ candidates: [subscribedCandidate] }));
		show();

		const row = screen.getByText('FastTravelAnywhere').closest('li') as HTMLElement;
		expect(within(row).getByText('Subscribed on Steam Workshop')).toBeTruthy();
		expect(within(row).getByText('Disabled')).toBeTruthy();
		expect(within(row).queryByText(/^Files:/)).toBeNull();
		expect(within(row).queryByRole('button', { name: 'Adopt' })).toBeNull();

		send.mockClear();
		await fireEvent.click(within(row).getByRole('button', { name: 'Manage in PalStudio' }));
		expect(send).toHaveBeenCalledWith(MessageType.MOD_ADOPT, {
			target_id: targetId,
			candidate_name: 'FastTravelAnywhere',
			source: 'steam_subscribed',
			root: 'C:/Steam/workshop/content/1623730/123456'
		});
	});

	it('shows the unreadable PalModSettings.ini notice', async () => {
		show();
		setScan(report({ files: cleanFiles, warnings: ['palmodsettings_unreadable'] }));
		await tick();

		expect(
			screen.getByText(
				'PalModSettings.ini could not be read, so Steam Workshop mods cannot be listed.'
			)
		).toBeTruthy();
		expect(screen.queryByText(allManaged)).toBeNull();
	});

	it('explains an already managed Workshop package under the candidate, with the rename hint', async () => {
		setScan(report({ candidates: [localCandidate, subscribedCandidate] }));
		show();

		const row = screen.getByText('FastTravelAnywhere').closest('li') as HTMLElement;
		await fireEvent.click(within(row).getByRole('button', { name: 'Manage in PalStudio' }));
		refuseAdopt(subscribedCandidate, {
			code: 'already_managed',
			message: 'FastTravelAnywhere is already managed'
		});
		await tick();

		expect(
			within(row).getByText('PalStudio already manages a Workshop mod with this package name.')
		).toBeTruthy();
		expect(
			within(row).getByText(
				'If this Steam item was renamed, remove the old entry from the library and manage it again.'
			)
		).toBeTruthy();
		expect(screen.queryByText('FastTravelAnywhere is already managed')).toBeNull();
		const other = screen.getByText('BetterUI').closest('li') as HTMLElement;
		expect(within(other).queryByText(/already manages/)).toBeNull();
	});

	it('leaves out the rename hint for a local candidate', async () => {
		setScan(report({ candidates: [localCandidate] }));
		show();

		await fireEvent.click(screen.getByRole('button', { name: 'Adopt' }));
		refuseAdopt(localCandidate, { code: 'already_managed', message: 'raw' });
		await tick();

		expect(
			screen.getByText('PalStudio already manages a Workshop mod with this package name.')
		).toBeTruthy();
		expect(screen.queryByText(/was renamed/)).toBeNull();
	});

	it('tells the user to scan again when the candidate is gone', async () => {
		setScan(report({ candidates: [subscribedCandidate] }));
		show();

		await fireEvent.click(screen.getByRole('button', { name: 'Manage in PalStudio' }));
		refuseAdopt(subscribedCandidate, {
			code: 'not_a_candidate',
			message: 'FastTravelAnywhere is not an adoption candidate on client-abc'
		});
		await tick();

		expect(screen.getByRole('alert').textContent).toContain(
			'This mod is no longer here. Scan again to refresh the list.'
		);
		expect(screen.queryByText(/not an adoption candidate/)).toBeNull();
	});

	it('shows an adopt refusal only under the same-named candidate at the root it named', async () => {
		const paks: AdoptionCandidate = {
			name: 'Foo.pak',
			kind: 'pak',
			root: 'C:/Palworld/Pal/Content/Paks/~mods/Foo.pak',
			files: ['C:/Palworld/Pal/Content/Paks/~mods/Foo.pak'],
			enabled: true,
			source: 'local'
		};
		const logic: AdoptionCandidate = {
			...paks,
			kind: 'logicmods',
			root: 'C:/Palworld/Pal/Content/Paks/LogicMods/Foo.pak',
			files: ['C:/Palworld/Pal/Content/Paks/LogicMods/Foo.pak']
		};
		setScan(report({ candidates: [paks, logic] }));
		show();

		refuseAdopt(logic, { code: 'not_a_candidate', message: 'raw' });
		await tick();

		expect(screen.getAllByRole('alert')).toHaveLength(1);
		const logicRow = screen.getByTitle(logic.root).closest('li') as HTMLElement;
		const paksRow = screen.getByTitle(paks.root).closest('li') as HTMLElement;
		expect(within(logicRow).getByRole('alert')).toBeTruthy();
		expect(within(paksRow).queryByRole('alert')).toBeNull();
	});

	it('shows an adopt refusal above the lists once its candidate is no longer listed', async () => {
		setScan(report({ candidates: [localCandidate] }));
		show();

		refuseAdopt(subscribedCandidate, { code: 'not_a_candidate', message: 'raw' });
		await tick();

		const alert = screen.getByRole('alert');
		expect(alert.textContent).toContain('This mod is no longer here.');
		expect(alert.closest('li')).toBeNull();
	});

	it('keeps the refusals of two candidates refused one after the other', async () => {
		setScan(report({ candidates: [localCandidate, subscribedCandidate] }));
		show();

		refuseAdopt(localCandidate, { code: 'already_managed', message: 'raw' });
		refuseAdopt(subscribedCandidate, { code: 'not_a_candidate', message: 'raw' });
		await tick();

		expect(screen.getAllByRole('alert')).toHaveLength(2);
		const localRow = screen.getByText('BetterUI').closest('li') as HTMLElement;
		const subscribedRow = screen.getByText('FastTravelAnywhere').closest('li') as HTMLElement;
		expect(within(localRow).getByRole('alert').textContent).toContain(
			'PalStudio already manages a Workshop mod with this package name.'
		);
		expect(within(subscribedRow).getByRole('alert').textContent).toContain(
			'This mod is no longer here.'
		);
	});

	it('clears an adopt refusal once a scan reply no longer lists its candidate', async () => {
		setScan(report({ candidates: [localCandidate] }));
		show();
		refuseAdopt(localCandidate, { code: 'not_a_candidate', message: 'raw' });
		await tick();
		expect(screen.getByRole('alert')).toBeTruthy();

		quickScan([localCandidate]);
		await tick();
		expect(screen.getByRole('alert')).toBeTruthy();

		setScan(report({ files: cleanFiles }));
		await tick();
		expect(screen.queryByRole('alert')).toBeNull();
		expect(screen.getByText(allManaged)).toBeTruthy();
	});

	it('says everything is managed when a clean full scan arrives', async () => {
		show();
		setScan(report({ files: cleanFiles }));
		await tick();

		expect(screen.getByText(allManaged)).toBeTruthy();
	});

	it('withdraws the claim when a candidates-only scan replies after the full one', async () => {
		show();
		setScan(report({ files: cleanFiles }));
		await tick();
		expect(screen.getByText(allManaged)).toBeTruthy();

		quickScan([]);
		await tick();

		expect(screen.queryByText(allManaged)).toBeNull();
		expect(screen.getByText('Intact')).toBeTruthy();
	});

	it('withdraws the claim when a later scan is refused', async () => {
		show();
		setScan(report({ files: cleanFiles }));
		await tick();
		expect(screen.getByText(allManaged)).toBeTruthy();

		state().refuseScan(targetId);
		state().recordRefusal(
			MessageType.MOD_TARGET_SCAN,
			{ code: 'scan_failed', message: 'layout could not be resolved' },
			targetId
		);
		await tick();

		expect(screen.getByText('layout could not be resolved')).toBeTruthy();
		expect(screen.queryByText(allManaged)).toBeNull();
	});

	it('keeps the claim across a remount when nothing replied in between', async () => {
		const { unmount } = show();
		setScan(report({ files: cleanFiles }));
		await tick();
		unmount();

		show();
		await tick();

		expect(screen.getByText(allManaged)).toBeTruthy();
	});

	it('claims everything is managed for a clean full reply that arrived before it was shown', () => {
		setScan(report({ files: cleanFiles }));
		show();

		expect(screen.getByText(allManaged)).toBeTruthy();
	});

	it('does not claim everything is managed while unmanaged files remain', async () => {
		show();
		setScan(
			report({
				files: [
					...cleanFiles,
					{ path: 'stray.txt', kind: null, state: 'unmanaged', mod_version_id: null }
				]
			})
		);
		await tick();

		expect(screen.queryByText(allManaged)).toBeNull();
	});

	it('does not claim everything is managed before a full scan', () => {
		quickScan([]);
		show();

		expect(screen.queryByText(allManaged)).toBeNull();
	});
});
