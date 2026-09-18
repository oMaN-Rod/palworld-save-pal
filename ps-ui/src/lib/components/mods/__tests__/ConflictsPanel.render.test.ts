// @vitest-environment jsdom
import { MessageType } from '$types';
import { fireEvent, render, screen } from '@testing-library/svelte';
import { tick } from 'svelte';
import { beforeEach, describe, expect, it, vi } from 'vitest';

const { holder, modal, send, remote, env } = vi.hoisted(() => ({
	holder: { state: undefined as unknown },
	modal: { showModal: vi.fn(), showConfirmModal: vi.fn() },
	send: vi.fn(),
	remote: { active: false },
	env: { desktop: 'true' }
}));

vi.mock('$lib/utils/websocketUtils', () => ({
	send: (type: unknown, data?: unknown) => send(type, data),
	sendAndWait: vi.fn(),
	sendBytes: vi.fn(),
	isReady: () => true,
	pushProgressMessage: vi.fn()
}));

vi.mock('$lib/signal/remoteMode.svelte', () => ({ getRemoteMode: () => remote }));

vi.mock('$env/static/public', () => ({
	get PUBLIC_DESKTOP_MODE() {
		return env.desktop;
	}
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
import type { InstallManifest, ModProfile } from '$types';
import ConflictsPanel from '../ConflictsPanel.svelte';
import { libraryMod, modTarget, modVersion } from './fixtures';

const state = () => holder.state as ModsState;
const targetId = 't1';
const gamepassTarget = modTarget({
	id: 't1',
	kind: 'client',
	platform: 'wingdk',
	root_path: 'C:/XboxGames/Palworld'
});

const report = {
	target_id: 't1',
	profile_id: 'p1',
	platform: 'wingdk',
	conflicts: [
		{
			kind: 'missing_dependency' as const,
			mod_id: 'mod-a',
			dependency: 'UE4SS',
			source: 'mod_type' as const
		},
		{
			kind: 'pak_overlap' as const,
			paks: [
				{ mod_id: 'mod-a', file: 'a.pak', source: 'library' as const, path: null },
				{ mod_id: 'mod-b', file: 'b.pak', source: 'library' as const, path: null }
			],
			winner: { mod_id: 'mod-b', file: 'b.pak', source: 'library' as const, path: null },
			assets: ['/Game/x', '/Game/y'],
			asset_count: 2
		}
	],
	unreadable: [
		{ mod_id: 'mod-a', file: 'c.pak', reason: 'iostore', source: 'library' as const, path: null }
	]
};

beforeEach(() => {
	send.mockReset();
	remote.active = false;
	env.desktop = 'true';
	state().reset();
	state().mods = [
		libraryMod({ id: 'mod-a', name: 'Alpha' }),
		libraryMod({ id: 'mod-b', name: 'Beta' })
	];
});

describe('ConflictsPanel', () => {
	it('asks for conflicts on open when none stored', () => {
		render(ConflictsPanel, { targetId });
		expect(send).toHaveBeenCalledWith('mod_conflicts', { target_id: 't1' });
	});

	it('does not re-request on a plain re-render when a report is already stored', async () => {
		state().conflicts = { t1: report };
		render(ConflictsPanel, { targetId });
		expect(send).not.toHaveBeenCalledWith('mod_conflicts', expect.anything());
		send.mockReset();

		state().mods = [...state().mods];
		await tick();
		expect(send).not.toHaveBeenCalledWith('mod_conflicts', expect.anything());
	});

	it('requests again when the selected target changes', async () => {
		state().conflicts = { t1: report };
		const { rerender } = render(ConflictsPanel, { targetId: 't1' });
		send.mockReset();

		await rerender({ targetId: 't2' });
		expect(send).toHaveBeenCalledWith('mod_conflicts', { target_id: 't2' });
	});

	it('requests again after a reset even with a report already stored', async () => {
		state().conflicts = { t1: report };
		render(ConflictsPanel, { targetId });
		send.mockReset();

		state().resets += 1;
		await tick();

		expect(send).toHaveBeenCalledWith('mod_conflicts', { target_id: 't1' });
	});

	it('renders one card per conflict under the right heading, naming the winner', async () => {
		state().conflicts = { t1: report };
		render(ConflictsPanel, { targetId });
		await tick();

		expect(screen.getByText('Missing requirements (1)')).toBeTruthy();
		expect(screen.getByText('Overlapping pak files (1)')).toBeTruthy();
		expect(screen.getByText(/Alpha needs UE4SS/)).toBeTruthy();
		expect(screen.getByText(/Beta loads last and wins/)).toBeTruthy();
	});

	it('names the winner in each overlap summary, so two overlaps with the same asset count stay distinguishable', async () => {
		state().conflicts = {
			t1: {
				...report,
				conflicts: [
					{
						kind: 'pak_overlap',
						paks: [
							{ mod_id: 'mod-a', file: 'a.pak', source: 'library', path: null },
							{ mod_id: 'mod-b', file: 'b.pak', source: 'library', path: null }
						],
						winner: { mod_id: 'mod-b', file: 'b.pak', source: 'library', path: null },
						assets: ['/Game/x'],
						asset_count: 2
					},
					{
						kind: 'pak_overlap',
						paks: [
							{ mod_id: 'mod-a', file: 'a2.pak', source: 'library', path: null },
							{ mod_id: 'mod-b', file: 'b2.pak', source: 'library', path: null }
						],
						winner: { mod_id: 'mod-a', file: 'a2.pak', source: 'library', path: null },
						assets: ['/Game/y'],
						asset_count: 2
					}
				]
			}
		};
		render(ConflictsPanel, { targetId });
		await tick();

		expect(screen.getByText('Show the 2 assets Beta overrides')).toBeTruthy();
		expect(screen.getByText('Show the 2 assets Alpha overrides')).toBeTruthy();
	});

	it('counts the unreadable files in the collapsed section', async () => {
		state().conflicts = { t1: report };
		render(ConflictsPanel, { targetId });
		await tick();

		expect(screen.getByText('Files that could not be checked (1)')).toBeTruthy();
	});

	it('disables Check again while checking', async () => {
		state().conflicts = { t1: report };
		render(ConflictsPanel, { targetId });
		state().loadConflicts(targetId);
		await tick();

		expect(
			(screen.getByRole('button', { name: 'Check again' }) as HTMLButtonElement).disabled
		).toBe(true);
	});

	it('shows a no_active_profile refusal as an alert', async () => {
		render(ConflictsPanel, { targetId });
		state().finishBusy('checkingConflicts', targetId);
		state().recordRefusal(
			MessageType.MOD_CONFLICTS,
			{ code: 'no_active_profile', message: 'none' },
			targetId
		);
		await tick();

		expect(screen.getByRole('alert').textContent).toContain(
			'This target has no active profile to check.'
		);
	});

	it('shows no conflicts message when the report is empty', async () => {
		state().conflicts = {
			t1: { target_id: 't1', profile_id: 'p1', platform: 'win64', conflicts: [], unreadable: [] }
		};
		render(ConflictsPanel, { targetId });
		await tick();

		expect(screen.getByText('No conflicts found in the active profile.')).toBeTruthy();
	});

	describe('Game Pass conversion', () => {
		beforeEach(() => {
			state().mods = [
				libraryMod({ id: 'mod-a', name: 'Alpha' }),
				libraryMod({ id: 'mod-b', name: 'Beta', mod_type: 'pak' })
			];
		});

		const gamepassReport = {
			target_id: 't1',
			profile_id: 'p1',
			platform: 'wingdk',
			conflicts: [
				{ kind: 'gamepass_pak_incompatible' as const, mod_id: 'mod-b', files: ['b.pak'] }
			],
			unreadable: []
		};

		it('judges conversion by the pinned version the report scanned, not the current one', async () => {
			const manifest = (routes: { rel_path: string; kind: string }[]) =>
				({
					routes: routes.map((route) => ({ archive_path: route.rel_path, ...route }))
				}) as unknown as InstallManifest;
			const legacy = modVersion({
				id: 'v-old',
				mod_id: 'mod-b',
				version: '1.0.0',
				is_current: false,
				manifest: manifest([{ rel_path: 'B_P.pak', kind: 'pak' }])
			});
			const converted = modVersion({
				id: 'v-new',
				mod_id: 'mod-b',
				version: '1.1.0+iostore',
				is_current: true,
				manifest: manifest([
					{ rel_path: 'B_P.pak', kind: 'pak' },
					{ rel_path: 'B_P.utoc', kind: 'pak' },
					{ rel_path: 'B_P.ucas', kind: 'pak' }
				])
			});
			const pinnedTo = (versionId: string | null) =>
				({
					id: 'p1',
					target_id: 't1',
					mods: [
						{
							profile_id: 'p1',
							mod_id: 'mod-b',
							mod_version_id: versionId,
							enabled: true,
							load_order: 0
						}
					]
				}) as unknown as ModProfile;
			state().mods = [
				libraryMod({
					id: 'mod-b',
					name: 'Beta',
					mod_type: 'pak',
					versions: [legacy, converted],
					current_version_id: 'v-new'
				})
			];
			state().targets = [gamepassTarget];
			state().conflicts = { t1: gamepassReport };

			state().profiles = { t1: [pinnedTo('v-old')] };
			const view = render(ConflictsPanel, { targetId });
			await tick();
			expect(screen.getByRole('button', { name: 'Convert Beta for Game Pass' })).toBeTruthy();
			view.unmount();

			state().profiles = { t1: [pinnedTo(null)] };
			render(ConflictsPanel, { targetId });
			await tick();
			expect(screen.queryByRole('button', { name: 'Convert Beta for Game Pass' })).toBeNull();
		});

		it('offers Convert for Game Pass on a local Game Pass install and sends it', async () => {
			state().targets = [gamepassTarget];
			state().conflicts = { t1: gamepassReport };
			render(ConflictsPanel, { targetId });
			await tick();

			const convertIostore = vi.spyOn(state(), 'convertIostore');
			await fireEvent.click(screen.getByRole('button', { name: 'Convert Beta for Game Pass' }));

			expect(convertIostore).toHaveBeenCalledWith('t1', 'mod-b');
		});

		it('hides the button on a non-wingdk target', async () => {
			state().targets = [{ ...gamepassTarget, platform: 'win64' }];
			state().conflicts = { t1: gamepassReport };
			render(ConflictsPanel, { targetId });
			await tick();

			expect(screen.queryByRole('button', { name: /Convert .* for Game Pass/ })).toBeNull();
		});

		it('hides the button in a remote session', async () => {
			remote.active = true;
			state().targets = [gamepassTarget];
			state().conflicts = { t1: gamepassReport };
			render(ConflictsPanel, { targetId });
			await tick();

			expect(screen.queryByRole('button', { name: /Convert .* for Game Pass/ })).toBeNull();
		});

		it('disables the button while converting', async () => {
			state().targets = [gamepassTarget];
			state().conflicts = { t1: gamepassReport };
			state().converting = { t1: true };
			render(ConflictsPanel, { targetId });
			await tick();

			expect(
				(screen.getByRole('button', { name: 'Convert Beta for Game Pass' }) as HTMLButtonElement)
					.disabled
			).toBe(true);
		});

		it('hides Convert once a successful conversion updates the pin and report', async () => {
			state().targets = [gamepassTarget];
			state().conflicts = { t1: gamepassReport };
			state().profiles = {
				t1: [
					{
						id: 'p1',
						target_id: 't1',
						mods: [
							{
								profile_id: 'p1',
								mod_id: 'mod-b',
								mod_version_id: null,
								enabled: true,
								load_order: 0
							}
						]
					} as unknown as ModProfile
				]
			};
			render(ConflictsPanel, { targetId });
			await tick();
			expect(screen.getByRole('button', { name: 'Convert Beta for Game Pass' })).toBeTruthy();

			state().profiles = {
				t1: [
					{
						id: 'p1',
						target_id: 't1',
						mods: [
							{
								profile_id: 'p1',
								mod_id: 'mod-b',
								mod_version_id: 'v-new',
								enabled: true,
								load_order: 0
							}
						]
					} as unknown as ModProfile
				]
			};
			state().conflicts = { t1: { ...gamepassReport, conflicts: [] } };
			await tick();

			expect(screen.queryByRole('button', { name: 'Convert Beta for Game Pass' })).toBeNull();
		});

		it('renders a stored conversion refusal under its card', async () => {
			state().targets = [gamepassTarget];
			state().conflicts = { t1: gamepassReport };
			state().recordRefusal(
				MessageType.MOD_IOSTORE_CONVERT,
				{ code: 'conversion_failed', message: 'iostoretoc failed' },
				targetId,
				{ subject: { mod_id: 'mod-b' } }
			);
			render(ConflictsPanel, { targetId });
			await tick();

			expect(screen.getByRole('alert').textContent).toContain('iostoretoc failed');
		});
	});

	it('labels untracked paks, explains them, and lists untracked unreadable files by name', async () => {
		state().conflicts = {
			t1: {
				...report,
				conflicts: [
					{
						kind: 'pak_overlap',
						paks: [
							{ mod_id: 'mod-a', file: 'a.pak', source: 'library', path: null },
							{ mod_id: 'mod-b', file: 'b.pak', source: 'disk', path: '~WorkshopMods/B/b.pak' },
							{ mod_id: null, file: 'Stray_P.pak', source: 'disk', path: 'LogicMods/Stray_P.pak' }
						],
						winner: {
							mod_id: null,
							file: 'Stray_P.pak',
							source: 'disk',
							path: 'LogicMods/Stray_P.pak'
						},
						assets: ['/Game/x'],
						asset_count: 1
					}
				],
				unreadable: [
					{
						mod_id: null,
						file: 'Broken.pak',
						reason: 'not_a_pak',
						source: 'disk',
						path: '~mods/Broken.pak'
					},
					{ mod_id: 'mod-a', file: 'c.pak', reason: 'iostore', source: 'library', path: null }
				]
			}
		};
		render(ConflictsPanel, { targetId });
		await tick();

		expect(screen.getByText(/Alpha, Beta, Stray_P\.pak \(untracked\)/)).toBeTruthy();
		expect(screen.getByText('Show the 1 assets Stray_P.pak (untracked) overrides')).toBeTruthy();
		expect(
			screen.getByText("Untracked paks are in the game's pak folders but not managed by PalStudio.")
		).toBeTruthy();
		expect(screen.getByText('Broken.pak (untracked): The file is not a pak')).toBeTruthy();
		expect(
			screen.getByText(
				'Alpha — c.pak: Already converted for Game Pass; its contents are not checked'
			)
		).toBeTruthy();
	});
});
