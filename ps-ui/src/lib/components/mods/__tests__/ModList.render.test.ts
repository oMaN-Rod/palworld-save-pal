// @vitest-environment jsdom
import { MessageType } from '$types';
import type { InstallManifest, LibraryMod, ModProfile, ModVersion, TargetLayoutJson } from '$types';
import { fireEvent, render, screen, within } from '@testing-library/svelte';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import '../../live/__tests__/fixtures/animatePolyfill';

const { holder, modal, remote, env } = vi.hoisted(() => ({
	holder: { state: undefined as unknown, nexus: undefined as unknown },
	modal: { showModal: vi.fn(), showConfirmModal: vi.fn() },
	remote: { active: false },
	env: { desktop: 'true' }
}));

vi.mock('$lib/signal/remoteMode.svelte', () => ({ getRemoteMode: () => remote }));

vi.mock('$env/static/public', () => ({
	get PUBLIC_DESKTOP_MODE() {
		return env.desktop;
	}
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
	const { NexusState } = await import('$lib/states/nexusState.svelte');
	holder.state = new ModsState();
	holder.nexus = new NexusState();
	return {
		getModsState: () => holder.state,
		getNexusState: () => holder.nexus,
		getModalState: () => modal,
		getServerState: () => ({ servers: [{ id: 7, name: 'Main World' }] }),
		getToastState: () => ({ add: vi.fn() }),
		downloadKey: (modId: number, fileId: number) => `${modId}:${fileId}`
	};
});

import { send } from '$lib/utils/websocketUtils';
import type { ModsState } from '$lib/states/modsState.svelte';
import type { NexusState } from '$lib/states/nexusState.svelte';
import {
	modIostoreConvertHandler,
	modReleaseProfilesHandler,
	modRemoveHandler,
	modVersionDeleteHandler,
	modVersionSetCurrentHandler,
	profileSetModHandler
} from '$lib/ws/handlers/modsHandler';
import { modUpdateCheckHandler } from '$lib/ws/handlers/nexusHandler';
import ModList from '../ModList.svelte';
import { modsViewMode } from '../modsView.svelte';
import { libraryMod, modVersion, modTarget as target } from './fixtures';

const context = { goto: vi.fn() };
const state = () => holder.state as ModsState;
const nexusState = () => holder.nexus as NexusState;

function version(overrides: Partial<ModVersion> = {}): ModVersion {
	return modVersion({ id: 'a-v1', size_bytes: 2048, ...overrides });
}

function mod(overrides: Partial<LibraryMod> = {}): LibraryMod {
	return libraryMod({
		author: 'Okaeri',
		versions: [version()],
		current_version_id: 'a-v1',
		...overrides
	});
}

function profile(targetId: string, enabled: string[]): ModProfile {
	return {
		id: `${targetId}-default`,
		target_id: targetId,
		name: 'Default',
		is_active: true,
		is_default: true,
		mods: enabled.map((modId, index) => ({
			profile_id: `${targetId}-default`,
			mod_id: modId,
			mod_version_id: null,
			enabled: true,
			load_order: index
		})),
		ue4ss_control_mode: 'managed',
		force_order_ue4ss: false,
		force_order_palschema: false,
		created_at: '2026-09-01T10:00:00Z',
		updated_at: '2026-09-01T10:00:00Z'
	};
}

const dockerLayout: TargetLayoutJson = {
	ue4ss_mods_dir: null,
	palschema_mods_dir: null,
	paks_mods_dir: '/palworld/paks',
	logicmods_dir: '/palworld/logicmods',
	nativemods_dir: '/palworld/nativemods',
	workshop_local_dir: null,
	mods_txt: null,
	palmodsettings_ini: null
};

const dockerTarget = target({
	id: 'server-7',
	kind: 'server',
	server_id: 7,
	name: 'old-name',
	root_path: '/palworld',
	platform: 'linux',
	layout: dockerLayout
});

const gamepassTarget = target({
	id: 'gamepass-1',
	kind: 'client',
	name: 'Game Pass Palworld',
	root_path: 'C:/XboxGames/Palworld',
	platform: 'wingdk'
});

const workshopManifest: InstallManifest = {
	folder_name: 'Pkg',
	display_name: 'Pkg',
	mod_type: 'workshop',
	version: '1',
	routes: [],
	decisions: [],
	platform_filtered: null,
	source: {}
};

function row(modId: string): HTMLElement {
	const found = document.querySelector(`[data-mod-id="${modId}"]`);
	expect(found).toBeTruthy();
	return found as HTMLElement;
}

function toggle(modId: string): HTMLButtonElement {
	return within(row(modId)).getByRole('switch') as HTMLButtonElement;
}

function sortCombobox(): HTMLElement {
	return screen.getAllByRole('combobox').find((el) => el.tagName !== 'SELECT') as HTMLElement;
}

function rowIds(): (string | null)[] {
	return [...document.querySelectorAll('[data-mod-id]')].map((el) =>
		el.getAttribute('data-mod-id')
	);
}

async function openMenu(modId: string) {
	await fireEvent.click(within(row(modId)).getByRole('button', { name: /Actions for/ }));
}

async function openDetails(modId: string) {
	await fireEvent.click(within(row(modId)).getByRole('button', { name: /^Show details for/ }));
}

function drawer(): HTMLElement {
	return screen.getByRole('complementary');
}

beforeEach(() => {
	vi.restoreAllMocks();
	modal.showConfirmModal.mockReset().mockResolvedValue(true);
	modal.showModal.mockReset();
	remote.active = false;
	env.desktop = 'true';
	const s = state();
	s.targets = [target(), dockerTarget];
	s.mods = [
		mod(),
		mod({
			id: 'mod-b',
			name: 'Bravo',
			author: 'Someone',
			mod_type: 'pak',
			versions: [version({ id: 'b-v1', mod_id: 'mod-b', size_bytes: 3 * 1024 * 1024 })],
			current_version_id: 'b-v1'
		})
	];
	s.profiles = { 'client-abc': [profile('client-abc', ['mod-a'])] };
	s.settingMod = {};
	s.pending = {};
	s.applying = {};
	s.releasing = {};
	s.lastRelease = {};
	s.lastError = {};
	modsViewMode.current = 'grid';
	nexusState().reset();
});

describe('ModList', () => {
	it('reflects the active profile in each toggle, labelled by the mod name', () => {
		render(ModList, { targetId: 'client-abc' });

		expect(toggle('mod-a').getAttribute('aria-checked')).toBe('true');
		expect(toggle('mod-b').getAttribute('aria-checked')).toBe('false');
		expect(screen.getByRole('switch', { name: 'Bravo' })).toBe(toggle('mod-b'));
	});

	it('enables a disabled mod through setMod', async () => {
		const setMod = vi.spyOn(state(), 'setMod');
		render(ModList, { targetId: 'client-abc' });

		await fireEvent.click(toggle('mod-b'));

		expect(setMod).toHaveBeenCalledWith('client-abc', 'mod-b', true, 'client-abc-default');
	});

	it('shows name, type, author, current version and total size', () => {
		render(ModList, { targetId: 'client-abc' });

		const b = within(row('mod-b'));
		expect(b.getByText('Bravo')).toBeTruthy();
		expect(b.getByText('Pak')).toBeTruthy();
		expect(b.getByText('by Someone')).toBeTruthy();
		expect(b.getByText('v1.0.0')).toBeTruthy();
		expect(b.getByText('3.0 MiB')).toBeTruthy();
	});

	it('disables the toggles only while this target has a set in flight', () => {
		state().settingMod = { 'server-7': true };
		const view = render(ModList, { targetId: 'client-abc' });
		expect(toggle('mod-b').disabled).toBe(false);
		view.unmount();

		state().settingMod = { 'client-abc': true };
		render(ModList, { targetId: 'client-abc' });
		expect(toggle('mod-a').disabled).toBe(true);
		expect(toggle('mod-b').disabled).toBe(true);
	});

	it('leaves the pending notice and its apply to the apply bar', () => {
		state().pending = { 'client-abc': true };
		render(ModList, { targetId: 'client-abc' });

		expect(screen.queryByText(/The game is running/)).toBeNull();
		expect(screen.queryByRole('button', { name: 'Apply now' })).toBeNull();
	});

	describe('the viewed profile', () => {
		function hardProfile() {
			return {
				...profile('client-abc', []),
				id: 'client-abc-hard',
				name: 'Hard',
				is_active: false,
				is_default: false,
				mods: [
					{
						profile_id: 'client-abc-hard',
						mod_id: 'mod-b',
						mod_version_id: null,
						enabled: true,
						load_order: 0
					}
				]
			};
		}

		beforeEach(() => {
			state().profiles = { 'client-abc': [profile('client-abc', ['mod-a']), hardProfile()] };
		});

		it('says nothing about activation while the active profile is viewed', () => {
			render(ModList, { targetId: 'client-abc' });
			expect(screen.queryByText(/isn't active/)).toBeNull();
		});

		it("shows a profile that isn't active, and says its changes wait for activation", () => {
			state().viewProfile('client-abc', 'client-abc-hard');
			render(ModList, { targetId: 'client-abc' });

			expect(toggle('mod-a').getAttribute('aria-checked')).toBe('false');
			expect(toggle('mod-b').getAttribute('aria-checked')).toBe('true');
			expect(
				screen.getByText(
					"You're editing Hard, which isn't active. These changes apply when it is active."
				)
			).toBeTruthy();
		});

		it('toggles a mod in the viewed profile', async () => {
			const setMod = vi.spyOn(state(), 'setMod');
			state().viewProfile('client-abc', 'client-abc-hard');
			render(ModList, { targetId: 'client-abc' });

			await fireEvent.click(toggle('mod-a'));

			expect(setMod).toHaveBeenCalledWith('client-abc', 'mod-a', true, 'client-abc-hard');
		});
	});

	describe('version pins', () => {
		beforeEach(() => {
			state().mods = [
				mod({
					versions: [
						version({ id: 'a-v1', version: '1.0.0', installed_at: '2026-09-01T10:00:00Z' }),
						version({ id: 'a-v2', version: '2.0.0', installed_at: '2026-09-05T10:00:00Z' })
					],
					current_version_id: 'a-v2'
				})
			];
		});

		async function pin(modId: string): Promise<HTMLSelectElement> {
			await openDetails(modId);
			return within(drawer()).getByLabelText('Version') as HTMLSelectElement;
		}

		it('follows the current version until a version is pinned', async () => {
			render(ModList, { targetId: 'client-abc' });

			const select = await pin('mod-a');
			expect(select.value).toBe('');
			expect([...select.options].map((option) => option.textContent?.trim())).toEqual([
				'Follow current (v2.0.0)',
				'Always v2.0.0',
				'Always v1.0.0'
			]);
		});

		it('shows a stored pin as selected', async () => {
			state().profiles['client-abc'][0].mods[0].mod_version_id = 'a-v1';
			render(ModList, { targetId: 'client-abc' });
			expect((await pin('mod-a')).value).toBe('a-v1');
		});

		it('pins a version and returns to following current', async () => {
			const setModVersion = vi.spyOn(state(), 'setModVersion');
			render(ModList, { targetId: 'client-abc' });

			await fireEvent.change(await pin('mod-a'), { target: { value: 'a-v1' } });
			await fireEvent.change(await pin('mod-a'), { target: { value: '' } });

			expect(setModVersion.mock.calls).toEqual([
				['client-abc', 'client-abc-default', 'mod-a', true, 'a-v1'],
				['client-abc', 'client-abc-default', 'mod-a', true, null]
			]);
		});

		it('offers no pin for a Steam-subscribed package', async () => {
			state().mods = [
				mod({
					source_kind: 'workshop',
					source_ref: JSON.stringify({ workshop_id: '3300000001' })
				})
			];
			render(ModList, { targetId: 'client-abc' });
			await openDetails('mod-a');
			expect(within(drawer()).queryByLabelText('Version')).toBeNull();
		});

		it('disables the pin while a selection change is in flight', async () => {
			state().settingMod = { 'client-abc': true };
			render(ModList, { targetId: 'client-abc' });
			expect((await pin('mod-a')).disabled).toBe(true);
		});
	});

	it('renders the targets and profiles of a removal refused as in use', async () => {
		const removeMod = vi.spyOn(state(), 'removeMod');
		render(ModList, { targetId: 'client-abc' });

		await openMenu('mod-a');
		await fireEvent.click(screen.getByRole('menuitem', { name: 'Remove from library' }));
		await vi.waitFor(() => expect(removeMod).toHaveBeenCalledWith('mod-a'));

		await modRemoveHandler.handle(
			{
				mod_id: 'mod-a',
				error: {
					code: 'version_in_use',
					message: 'mod mod-a is in use',
					targets: ['client-abc'],
					profiles: ['client-abc-default']
				}
			},
			context as never
		);

		expect(
			await within(row('mod-a')).findByText('In use by targets: Palworld and profiles: Default')
		).toBeTruthy();
		expect(within(row('mod-b')).queryByText(/In use by/)).toBeNull();
	});

	it('does not remove when the confirmation is declined', async () => {
		modal.showConfirmModal.mockResolvedValue(false);
		const removeMod = vi.spyOn(state(), 'removeMod');
		render(ModList, { targetId: 'client-abc' });

		await openMenu('mod-a');
		await fireEvent.click(screen.getByRole('menuitem', { name: 'Remove from library' }));
		await Promise.resolve();

		expect(modal.showConfirmModal).toHaveBeenCalled();
		expect(removeMod).not.toHaveBeenCalled();
	});

	it('renders a removal refused while an apply runs', async () => {
		render(ModList, { targetId: 'client-abc' });

		await modRemoveHandler.handle(
			{
				mod_id: 'mod-a',
				error: { code: 'apply_in_progress', message: 'busy', target_id: 'server-7' }
			},
			context as never
		);

		expect(await within(row('mod-a')).findByText(/Main World is applying mods/)).toBeTruthy();
	});

	it('loads the profiles of unloaded targets when a refusal names a profile it cannot resolve', () => {
		const loadProfiles = vi.spyOn(state(), 'loadProfiles').mockImplementation(() => {});
		state().recordRefusal(
			'mod_remove',
			{ code: 'version_in_use', message: 'in use', targets: [], profiles: ['server-7-default'] },
			undefined,
			{ subject: { mod_id: 'mod-a' } }
		);
		render(ModList, { targetId: 'client-abc' });

		expect(loadProfiles).toHaveBeenCalledTimes(1);
		expect(loadProfiles).toHaveBeenCalledWith('server-7');
	});

	describe('remove from profile', () => {
		it('is offered only for a mod the viewed profile holds, and sends the viewed profile', async () => {
			const removeFromProfile = vi.spyOn(state(), 'removeFromProfile').mockImplementation(() => {});
			render(ModList, { targetId: 'client-abc' });

			await openMenu('mod-b');
			expect(screen.queryByRole('menuitem', { name: 'Remove from profile' })).toBeNull();
			await openMenu('mod-a');
			await fireEvent.click(screen.getByRole('menuitem', { name: 'Remove from profile' }));

			expect(removeFromProfile).toHaveBeenCalledWith('client-abc', 'mod-a', 'client-abc-default');
		});

		it('is disabled while a toggle for the target is in flight', async () => {
			state().settingMod = { 'client-abc': true };
			render(ModList, { targetId: 'client-abc' });

			await openMenu('mod-a');
			expect(
				(screen.getByRole('menuitem', { name: 'Remove from profile' }) as HTMLButtonElement)
					.disabled
			).toBe(true);
		});

		it('renders a refusal under the row', async () => {
			render(ModList, { targetId: 'client-abc' });
			state().recordRefusal(
				'profile_remove_mod',
				{ code: 'mod_not_in_profile', message: 'gone' },
				'client-abc',
				{ subject: { mod_id: 'mod-a' } }
			);

			expect(
				await within(row('mod-a')).findByText('This mod is no longer in the profile.')
			).toBeTruthy();
		});
	});

	describe('in-use refusal actions', () => {
		const inUse = {
			code: 'version_in_use',
			message: 'mod mod-a is in use',
			targets: ['client-abc', 'server-7'],
			profiles: ['client-abc-default', 'server-7-default'],
			holders: [
				{
					target_id: 'client-abc',
					target_name: 'Palworld',
					profile_id: 'client-abc-default',
					profile_name: 'Default',
					enabled: true
				},
				{
					target_id: 'server-7',
					target_name: 'old-name',
					profile_id: 'server-7-default',
					profile_name: 'Default',
					enabled: false
				}
			],
			deployed: [{ target_id: 'server-7', target_name: 'old-name' }]
		};

		async function refuse(error: typeof inUse & { frameworks?: unknown[] } = inUse) {
			await modRemoveHandler.handle({ mod_id: 'mod-a', error }, context as never);
		}

		it('lists each holder and deployment by name', async () => {
			render(ModList, { targetId: 'client-abc' });
			await refuse();

			const card = within(row('mod-a'));
			expect(await card.findByText('Palworld / Default (enabled)')).toBeTruthy();
			expect(card.getByText('Main World / Default')).toBeTruthy();
			expect(card.getByText('Its files are still on: Main World')).toBeTruthy();
			expect(card.getByText('Turn it off in the enabled profiles first.')).toBeTruthy();
			expect(card.queryByText(/In use by targets/)).toBeNull();
		});

		it('explains a mod held only by a framework slot', async () => {
			render(ModList, { targetId: 'client-abc' });
			await refuse({
				...inUse,
				targets: ['client-abc'],
				profiles: [],
				holders: [],
				deployed: [],
				frameworks: [{ target_id: 'client-abc', target_name: 'old-name' }]
			});

			const card = within(row('mod-a'));
			expect(await card.findByText('Installed as a framework on: Palworld')).toBeTruthy();
			expect(card.queryByText(/Its files are still on/)).toBeNull();
			expect(card.queryByRole('button', { name: /^Remove from disabled profiles/ })).toBeNull();
			expect(card.getByRole('button', { name: 'Delete again: Alpha' })).toBeTruthy();
		});

		it('releases disabled profiles and applies a deployed target with no enabled holder', async () => {
			const releaseProfiles = vi.spyOn(state(), 'releaseProfiles').mockImplementation(() => {});
			const apply = vi.spyOn(state(), 'apply').mockImplementation(() => {});
			render(ModList, { targetId: 'client-abc' });
			await refuse({
				...inUse,
				deployed: [{ target_id: 'client-abc', target_name: 'Palworld' }, ...inUse.deployed]
			});

			const card = within(row('mod-a'));
			expect(await card.findByText('Its files are still on: Palworld, Main World')).toBeTruthy();
			await fireEvent.click(
				card.getByRole('button', { name: 'Remove from disabled profiles: Alpha' })
			);
			expect(releaseProfiles).toHaveBeenCalledWith('mod-a');
			await fireEvent.click(
				card.getByRole('button', { name: 'Apply Main World to remove the files of Alpha' })
			);
			expect(apply).toHaveBeenCalledWith('server-7');
			expect(card.queryByRole('button', { name: /^Apply Palworld/ })).toBeNull();
		});

		it('drops released holders and reports the release', async () => {
			render(ModList, { targetId: 'client-abc' });
			await refuse();
			await modReleaseProfilesHandler.handle(
				{
					mod_id: 'mod-a',
					removed: [{ target_id: 'server-7', profile_id: 'server-7-default' }],
					enabled: [{ target_id: 'client-abc', profile_id: 'client-abc-default' }]
				},
				context as never
			);

			const card = within(row('mod-a'));
			expect(await card.findByText('Disabled profiles removed: 1')).toBeTruthy();
			expect(card.queryByText('Main World / Default')).toBeNull();
			expect(
				card.queryByRole('button', { name: 'Remove from disabled profiles: Alpha' })
			).toBeNull();
			expect(
				card.getByRole('button', { name: 'Apply Main World to remove the files of Alpha' })
			).toBeTruthy();
		});

		it('disables Apply while that target applies, and retries the delete after confirming', async () => {
			const removeMod = vi.spyOn(state(), 'removeMod').mockImplementation(() => {});
			state().applying = { 'server-7': true };
			render(ModList, { targetId: 'client-abc' });
			await refuse();

			const card = within(row('mod-a'));
			expect(
				(
					(await card.findByRole('button', {
						name: 'Apply Main World to remove the files of Alpha'
					})) as HTMLButtonElement
				).disabled
			).toBe(true);
			await fireEvent.click(card.getByRole('button', { name: 'Delete again: Alpha' }));
			await vi.waitFor(() => expect(removeMod).toHaveBeenCalledWith('mod-a'));
			expect(modal.showConfirmModal).toHaveBeenCalled();
		});
	});

	describe('refusal attribution', () => {
		const refusedToggle = (modId: string, message: string) =>
			profileSetModHandler.handle(
				{
					target_id: 'client-abc',
					mod_id: modId,
					enabled: true,
					error: { code: 'db', message }
				},
				context as never
			);

		it('shows a toggle refusal that names no mod above the list', async () => {
			render(ModList, { targetId: 'client-abc' });

			await profileSetModHandler.handle(
				{
					target_id: 'client-abc',
					error: { code: 'target_not_found', message: 'no mod target client-abc' }
				} as never,
				context as never
			);

			const alert = await screen.findByRole('alert');
			expect(alert.textContent).toContain(
				'This game install is no longer managed by PalStudio, so the change was not saved.'
			);
			expect(within(row('mod-a')).queryByRole('alert')).toBeNull();
		});

		it('renders a refusal under the row it names, even after acting on another row', async () => {
			render(ModList, { targetId: 'client-abc' });
			state().setMod('client-abc', 'mod-a', false);
			await fireEvent.click(toggle('mod-b'));

			await refusedToggle('mod-a', 'disk full');

			expect(await within(row('mod-a')).findByText('disk full')).toBeTruthy();
			expect(within(row('mod-b')).queryByText('disk full')).toBeNull();
		});

		it("keeps each row's refusal when the next refusal of that type names another row", async () => {
			render(ModList, { targetId: 'client-abc' });

			await modVersionSetCurrentHandler.handle(
				{ mod_id: 'mod-a', version_id: 'a-v1', error: { code: 'db', message: 'first' } },
				context as never
			);
			expect(await within(row('mod-a')).findByText('first')).toBeTruthy();

			await modVersionSetCurrentHandler.handle(
				{ mod_id: 'mod-b', version_id: 'b-v1', error: { code: 'db', message: 'second' } },
				context as never
			);
			expect(await within(row('mod-b')).findByText('second')).toBeTruthy();
			expect(within(row('mod-a')).getByText('first')).toBeTruthy();
		});

		it('keeps a refusal through an unmount and a remount', async () => {
			const view = render(ModList, { targetId: 'client-abc' });
			await refusedToggle('mod-b', 'kept across tabs');
			view.unmount();

			render(ModList, { targetId: 'client-abc' });

			expect(within(row('mod-b')).getByText('kept across tabs')).toBeTruthy();
		});

		it("keeps one row's refusal when the same request succeeds for another row", async () => {
			render(ModList, { targetId: 'client-abc' });
			await modRemoveHandler.handle(
				{ mod_id: 'mod-a', error: { code: 'remove_failed', message: 'locked file' } },
				context as never
			);

			await modRemoveHandler.handle({ mod_id: 'mod-b', removed: true }, context as never);

			expect(await within(row('mod-a')).findByText('locked file')).toBeTruthy();
		});

		it('renders a version delete refusal under the mod that owns the version', async () => {
			render(ModList, { targetId: 'client-abc' });

			await modVersionDeleteHandler.handle(
				{
					version_id: 'a-v1',
					error: {
						code: 'version_in_use',
						message: 'mod version a-v1 is in use',
						is_current: true,
						targets: [],
						profiles: [],
						frameworks: [],
						open_applies: []
					}
				},
				context as never
			);

			expect(await within(row('mod-a')).findByText(/This is the current version/)).toBeTruthy();
			expect(within(row('mod-b')).queryByRole('alert')).toBeNull();
		});

		it('renders a refused make-current under its mod', async () => {
			render(ModList, { targetId: 'client-abc' });

			await modVersionSetCurrentHandler.handle(
				{
					mod_id: 'mod-b',
					version_id: 'b-v1',
					error: { code: 'subscribed_package', message: 'Steam owns its versions' }
				},
				context as never
			);

			expect(await within(row('mod-b')).findByText('Steam owns its versions')).toBeTruthy();
		});

		it('names the kind a target has no place for', async () => {
			render(ModList, { targetId: 'client-abc' });

			await profileSetModHandler.handle(
				{
					target_id: 'client-abc',
					mod_id: 'mod-b',
					enabled: true,
					error: { code: 'not_supported_on_target', message: 'no place', kind: 'pak' }
				},
				context as never
			);

			expect((await within(row('mod-b')).findByRole('alert')).textContent?.trim()).toBe(
				'This game install has no place for Pak files'
			);
		});
	});

	describe('versions', () => {
		beforeEach(() => {
			state().mods = [
				mod({
					versions: [
						version(),
						version({
							id: 'a-v2',
							version: '2.0.0',
							is_current: false,
							size_bytes: null,
							installed_at: '2026-09-05T10:00:00Z'
						})
					]
				})
			];
		});

		async function openVersions() {
			render(ModList, { targetId: 'client-abc' });
			await openMenu('mod-a');
			await fireEvent.click(screen.getByRole('menuitem', { name: 'Details…' }));
		}

		const versionRow = (id: string) =>
			within(drawer().querySelector(`[data-version-id="${id}"]`) as HTMLElement);

		it('lists versions with the current marker and no actions on the current one', async () => {
			await openVersions();

			expect(versionRow('a-v2').getByText('2.0.0')).toBeTruthy();
			expect(versionRow('a-v2').getByText('—')).toBeTruthy();
			expect(versionRow('a-v1').getByText('Current')).toBeTruthy();
			expect(versionRow('a-v1').queryByRole('button', { name: 'Make current' })).toBeNull();
			expect(versionRow('a-v1').queryByRole('button', { name: 'Delete' })).toBeNull();
		});

		it('makes another version current', async () => {
			const setCurrent = vi.spyOn(state(), 'setCurrentVersion').mockImplementation(() => {});
			await openVersions();

			await fireEvent.click(versionRow('a-v2').getByRole('button', { name: 'Make current' }));

			expect(setCurrent).toHaveBeenCalledWith('mod-a', 'a-v2');
		});

		it('deletes a version only after confirming', async () => {
			const deleteVersion = vi.spyOn(state(), 'deleteVersion').mockImplementation(() => {});
			await openVersions();

			await fireEvent.click(versionRow('a-v2').getByRole('button', { name: 'Delete' }));

			await vi.waitFor(() => expect(deleteVersion).toHaveBeenCalledWith('a-v2'));
			expect(modal.showConfirmModal).toHaveBeenCalledTimes(1);
			expect(modal.showConfirmModal.mock.invocationCallOrder[0]).toBeLessThan(
				deleteVersion.mock.invocationCallOrder[0]
			);
		});

		it('keeps a version when the confirmation is declined', async () => {
			modal.showConfirmModal.mockResolvedValue(false);
			const deleteVersion = vi.spyOn(state(), 'deleteVersion').mockImplementation(() => {});
			await openVersions();

			await fireEvent.click(versionRow('a-v2').getByRole('button', { name: 'Delete' }));
			await Promise.resolve();

			expect(modal.showConfirmModal).toHaveBeenCalled();
			expect(deleteVersion).not.toHaveBeenCalled();
		});
	});

	describe('row menu', () => {
		it('moves focus into the menu and back to its trigger on Escape', async () => {
			render(ModList, { targetId: 'client-abc' });
			const trigger = within(row('mod-a')).getByRole('button', { name: /Actions for/ });
			expect(trigger.getAttribute('aria-haspopup')).toBe('menu');

			await openMenu('mod-a');
			expect(document.activeElement).toBe(screen.getByRole('menuitem', { name: 'Details…' }));

			await fireEvent.keyDown(screen.getByRole('menu'), { key: 'Escape' });
			expect(document.activeElement).toBe(trigger);
		});
	});

	describe('install', () => {
		it('offers Install a mod in the empty state, opening the install modal for this target', async () => {
			state().mods = [];
			render(ModList, { targetId: 'client-abc' });

			expect(screen.getByText('No mods in the library')).toBeTruthy();
			await fireEvent.click(screen.getByRole('button', { name: 'Install a mod' }));

			expect(modal.showModal).toHaveBeenCalledTimes(1);
			expect(modal.showModal.mock.calls[0][1]).toEqual({ targetId: 'client-abc' });
		});

		it('offers Install a mod beside the filter when the library has mods', async () => {
			render(ModList, { targetId: 'server-7' });

			await fireEvent.click(screen.getByRole('button', { name: 'Install a mod' }));

			expect(modal.showModal.mock.calls[0][1]).toEqual({ targetId: 'server-7' });
		});

		it('offers Install a mod in a remote session, since archives can be uploaded', () => {
			remote.active = true;
			const view = render(ModList, { targetId: 'client-abc' });
			expect(screen.getByRole('button', { name: 'Install a mod' })).toBeTruthy();
			view.unmount();

			state().mods = [];
			render(ModList, { targetId: 'client-abc' });
			expect(screen.getByRole('button', { name: 'Install a mod' })).toBeTruthy();
			expect(screen.getByText('Install a mod archive to add it to the library.')).toBeTruthy();
		});

		it('keeps the install hint in the empty state outside a remote session', () => {
			state().mods = [];
			render(ModList, { targetId: 'client-abc' });

			expect(screen.getByText('Install a mod archive to add it to the library.')).toBeTruthy();
			expect(screen.queryByText(/computer running PalStudio/)).toBeNull();
		});
	});

	it('filters by author', async () => {
		render(ModList, { targetId: 'client-abc' });

		await fireEvent.input(screen.getByPlaceholderText('Filter by name or author'), {
			target: { value: 'someone' }
		});

		expect(rowIds()).toEqual(['mod-b']);
	});

	it('sorts by name by default, then by type and by size', async () => {
		state().mods = [
			...state().mods,
			mod({
				id: 'mod-c',
				name: 'Charlie',
				mod_type: 'logicmods',
				versions: [version({ id: 'c-v1', mod_id: 'mod-c', size_bytes: 10 })],
				current_version_id: 'c-v1'
			})
		];
		render(ModList, { targetId: 'client-abc' });
		expect(rowIds()).toEqual(['mod-a', 'mod-b', 'mod-c']);

		await fireEvent.click(sortCombobox());
		await fireEvent.click(screen.getByRole('option', { name: 'Type' }));
		await vi.waitFor(() => expect(rowIds()).toEqual(['mod-c', 'mod-b', 'mod-a']));

		await fireEvent.click(sortCombobox());
		await fireEvent.click(screen.getByRole('option', { name: 'Size' }));
		await vi.waitFor(() => expect(rowIds()).toEqual(['mod-b', 'mod-a', 'mod-c']));
	});

	describe('route kinds a target cannot hold', () => {
		function subscribed(overrides: Partial<LibraryMod> = {}): LibraryMod {
			return mod({
				id: 'workshop-3301',
				name: 'Subscribed',
				mod_type: 'workshop',
				source_kind: 'workshop',
				source_ref: '{"workshop_id":"3301","package":"Pkg"}',
				versions: [version({ id: 'w-v1', mod_id: 'workshop-3301', manifest: workshopManifest })],
				current_version_id: 'w-v1',
				...overrides
			});
		}

		it('marks a subscribed mod and offers no version actions', async () => {
			state().mods = [subscribed()];
			render(ModList, { targetId: 'client-abc' });

			expect(within(row('workshop-3301')).getByText('Steam Workshop')).toBeTruthy();
			await openMenu('workshop-3301');
			expect(screen.getByRole('menuitem', { name: 'Remove from library' })).toBeTruthy();
			await fireEvent.click(screen.getByRole('menuitem', { name: 'Details…' }));
			expect(drawer().querySelector('[data-version-id]')).toBeNull();
			expect(within(drawer()).queryByLabelText('Version')).toBeNull();
		});

		it('warns on a server target when the package has no server install rule', () => {
			state().mods = [
				subscribed({
					versions: [
						version({
							id: 'w-v1',
							mod_id: 'workshop-3301',
							manifest: { ...workshopManifest, source: { server_capable: false } }
						})
					]
				}),
				mod({ versions: [version({ manifest: { ...workshopManifest, mod_type: 'ue4ss' } })] })
			];
			render(ModList, { targetId: 'server-7' });

			expect(within(row('workshop-3301')).getByText(/Client only/)).toBeTruthy();
			expect(within(row('mod-a')).queryByText(/Client only/)).toBeNull();
		});

		it('blocks enabling on a Docker server but still allows turning one off', () => {
			state().mods = [subscribed(), subscribed({ id: 'workshop-9', name: 'Already on' })];
			state().profiles = { 'server-7': [profile('server-7', ['workshop-9'])] };
			render(ModList, { targetId: 'server-7' });

			const blocked = toggle('workshop-3301');
			expect(blocked.disabled).toBe(true);
			const reason = document.getElementById(blocked.getAttribute('aria-describedby') ?? '');
			expect(reason?.textContent?.trim()).toBe("Docker servers can't load Workshop packages");
			expect(toggle('workshop-9').disabled).toBe(false);
			expect(toggle('workshop-9').getAttribute('aria-describedby')).toBeNull();
		});

		it('blocks enabling a native DLL mod on a game client', () => {
			state().targets = [target({ layout: { ...dockerLayout, nativemods_dir: null } })];
			state().mods = [
				mod({
					id: 'native',
					name: 'Native',
					mod_type: 'nativedll',
					versions: [
						version({
							id: 'n-v1',
							mod_id: 'native',
							manifest: {
								...workshopManifest,
								mod_type: 'nativedll',
								routes: [{ archive_path: 'a.dll', rel_path: 'a.dll', kind: 'nativedll' }]
							}
						})
					],
					current_version_id: 'n-v1'
				})
			];
			render(ModList, { targetId: 'client-abc' });

			expect(toggle('native').disabled).toBe(true);
			expect(
				within(row('native')).getByText("Game clients can't load native DLL mods")
			).toBeTruthy();
		});

		it('renders a subscription missing on the target under the toggled row', async () => {
			state().mods = [subscribed(), mod()];
			render(ModList, { targetId: 'client-abc' });

			await fireEvent.click(toggle('workshop-3301'));
			await profileSetModHandler.handle(
				{
					target_id: 'client-abc',
					mod_id: 'workshop-3301',
					enabled: true,
					error: {
						code: 'not_subscribed_on_target',
						message: 'Steam Workshop item 3301 is not in the directory',
						workshop_id: '3301'
					}
				},
				context as never
			);

			expect(
				await within(row('workshop-3301')).findByText(
					"This server's Steam Workshop folder does not have this mod. Subscribe to it there first."
				)
			).toBeTruthy();
			expect(within(row('mod-a')).queryByText(/Subscribe to it/)).toBeNull();
		});

		it('shows no verification pill without a stored entry', () => {
			render(ModList, { targetId: 'client-abc' });
			expect(within(row('mod-a')).queryByText('Verified')).toBeNull();
		});

		it('renders a Docker refusal of a Workshop package in words', async () => {
			state().mods = [subscribed()];
			state().profiles = { 'server-7': [profile('server-7', ['workshop-3301'])] };
			render(ModList, { targetId: 'server-7' });

			await profileSetModHandler.handle(
				{
					target_id: 'server-7',
					mod_id: 'workshop-3301',
					enabled: true,
					error: { code: 'not_supported_on_target', message: 'no place', kind: 'workshop' }
				},
				context as never
			);

			expect((await within(row('workshop-3301')).findByRole('alert')).textContent?.trim()).toBe(
				"Docker servers can't load Workshop packages"
			);
		});
	});

	describe('verification pills', () => {
		function verify(status: 'verified' | 'missing' | 'unknown', live = true) {
			state().verification = {
				'client-abc': {
					target_id: 'client-abc',
					instance_id: 'auto:1',
					live,
					checked_at: '2026-09-01T10:00:00Z',
					build_info: null,
					resolution: { complete: true, missing: [] },
					status: [{ mod_id: 'mod-a', name: 'Alpha', kind: 'ue4ss', status }]
				}
			};
		}

		it('shows a Verified pill on an enabled mod that verified', () => {
			verify('verified');
			render(ModList, { targetId: 'client-abc' });
			expect(within(row('mod-a')).getByText('Verified')).toBeTruthy();
		});

		it('shows a Not loaded pill for a missing mod', () => {
			verify('missing');
			render(ModList, { targetId: 'client-abc' });
			expect(within(row('mod-a')).getByText('Not loaded')).toBeTruthy();
		});

		it('shows no pill on a disabled mod even with a stored entry', () => {
			state().verification = {
				'client-abc': {
					target_id: 'client-abc',
					instance_id: 'auto:1',
					live: true,
					checked_at: '2026-09-01T10:00:00Z',
					build_info: null,
					resolution: { complete: true, missing: [] },
					status: [{ mod_id: 'mod-b', name: 'Bravo', kind: 'pak', status: 'verified' }]
				}
			};
			render(ModList, { targetId: 'client-abc' });
			expect(within(row('mod-b')).queryByText('Verified')).toBeNull();
		});

		it('carries a last-seen title when the target is not live', () => {
			verify('verified', false);
			render(ModList, { targetId: 'client-abc' });
			expect(within(row('mod-a')).getByText('Verified').getAttribute('title')).toContain(
				'Last checked'
			);
		});
	});

	describe('Game Pass conversion', () => {
		beforeEach(() => {
			state().targets = [target(), dockerTarget, gamepassTarget];
			state().profiles = {
				'client-abc': [profile('client-abc', ['mod-a'])],
				'gamepass-1': [profile('gamepass-1', ['mod-b'])]
			};
		});

		it('offers Convert for Game Pass on a pak row for a wingdk client target in desktop mode, and sends it', async () => {
			const convertIostore = vi.spyOn(state(), 'convertIostore');
			render(ModList, { targetId: 'gamepass-1' });

			await openMenu('mod-b');
			await fireEvent.click(screen.getByRole('menuitem', { name: 'Convert for Game Pass' }));

			expect(convertIostore).toHaveBeenCalledWith('gamepass-1', 'mod-b');
		});

		it('offers no conversion on a non-wingdk target', async () => {
			render(ModList, { targetId: 'client-abc' });
			await openMenu('mod-b');
			expect(screen.queryByRole('menuitem', { name: 'Convert for Game Pass' })).toBeNull();
		});

		it('offers no conversion in a remote session', async () => {
			remote.active = true;
			render(ModList, { targetId: 'gamepass-1' });
			await openMenu('mod-b');
			expect(screen.queryByRole('menuitem', { name: 'Convert for Game Pass' })).toBeNull();
		});

		it('renders a conversion refusal under the row it names', async () => {
			render(ModList, { targetId: 'gamepass-1' });

			await modIostoreConvertHandler.handle(
				{
					target_id: 'gamepass-1',
					mod_id: 'mod-b',
					error: { code: 'not_convertible', message: 'no pak files' }
				},
				context as never
			);

			expect(
				await within(row('mod-b')).findByText('This mod has no pak files to convert.')
			).toBeTruthy();
		});

		it('shows a Game Pass pill on a converted pin, and a note when a newer version is installed', () => {
			state().mods = [
				mod({
					id: 'mod-b',
					name: 'Bravo',
					mod_type: 'pak',
					versions: [
						version({ id: 'b-v1', mod_id: 'mod-b', version: '1.0.0+iostore' }),
						version({ id: 'b-v2', mod_id: 'mod-b', version: '1.1.0' })
					],
					current_version_id: 'b-v2'
				})
			];
			state().profiles['gamepass-1'][0].mods[0].mod_version_id = 'b-v1';
			render(ModList, { targetId: 'gamepass-1' });

			expect(within(row('mod-b')).getByText('Game Pass')).toBeTruthy();
			expect(
				within(row('mod-b')).getByText('Converted from 1.0.0; a newer version is installed.')
			).toBeTruthy();
		});

		it('shows the pill but no note when the current version itself is the converted build', () => {
			state().mods = [
				mod({
					id: 'mod-b',
					name: 'Bravo',
					mod_type: 'pak',
					versions: [version({ id: 'b-v1', mod_id: 'mod-b', version: '1.0.0+iostore' })],
					current_version_id: 'b-v1'
				})
			];
			render(ModList, { targetId: 'gamepass-1' });

			expect(within(row('mod-b')).getByText('Game Pass')).toBeTruthy();
			expect(within(row('mod-b')).queryByText(/a newer version is installed/)).toBeNull();
		});

		it('judges Convert against the active profile, not an inactive viewed one', async () => {
			const legacyManifest: InstallManifest = {
				folder_name: 'Mod',
				display_name: 'Mod',
				mod_type: 'pak',
				version: '1.0.0',
				routes: [{ archive_path: 'Cool_P.pak', rel_path: 'Cool_P.pak', kind: 'pak' }],
				decisions: [],
				platform_filtered: null,
				source: {}
			};
			const convertedManifest: InstallManifest = {
				...legacyManifest,
				routes: [
					...legacyManifest.routes,
					{ archive_path: 'Cool_P.utoc', rel_path: 'Cool_P.utoc', kind: 'pak' },
					{ archive_path: 'Cool_P.ucas', rel_path: 'Cool_P.ucas', kind: 'pak' }
				]
			};
			state().mods = [
				mod({
					id: 'mod-b',
					name: 'Bravo',
					mod_type: 'pak',
					versions: [
						version({ id: 'b-v1', mod_id: 'mod-b', version: '1.0.0', manifest: legacyManifest }),
						version({
							id: 'b-v2',
							mod_id: 'mod-b',
							version: '1.0.0+iostore',
							manifest: convertedManifest
						})
					],
					current_version_id: 'b-v2'
				})
			];

			function pinnedProfile(id: string, versionId: string, active: boolean): ModProfile {
				const base = profile('gamepass-1', ['mod-b']);
				return {
					...base,
					id,
					is_active: active,
					mods: base.mods.map((entry) => ({ ...entry, profile_id: id, mod_version_id: versionId }))
				};
			}

			state().profiles = {
				'gamepass-1': [
					pinnedProfile('gamepass-1-default', 'b-v2', true),
					pinnedProfile('gamepass-1-hard', 'b-v1', false)
				]
			};
			state().viewProfile('gamepass-1', 'gamepass-1-hard');

			render(ModList, { targetId: 'gamepass-1' });
			await openMenu('mod-b');
			expect(screen.queryByRole('menuitem', { name: 'Convert for Game Pass' })).toBeNull();
		});

		it('offers Convert when the active profile pins a legacy version, even though an inactive viewed one pins the converted build', async () => {
			const legacyManifest: InstallManifest = {
				folder_name: 'Mod',
				display_name: 'Mod',
				mod_type: 'pak',
				version: '1.0.0',
				routes: [{ archive_path: 'Cool_P.pak', rel_path: 'Cool_P.pak', kind: 'pak' }],
				decisions: [],
				platform_filtered: null,
				source: {}
			};
			const convertedManifest: InstallManifest = {
				...legacyManifest,
				routes: [
					...legacyManifest.routes,
					{ archive_path: 'Cool_P.utoc', rel_path: 'Cool_P.utoc', kind: 'pak' },
					{ archive_path: 'Cool_P.ucas', rel_path: 'Cool_P.ucas', kind: 'pak' }
				]
			};
			state().mods = [
				mod({
					id: 'mod-b',
					name: 'Bravo',
					mod_type: 'pak',
					versions: [
						version({ id: 'b-v1', mod_id: 'mod-b', version: '1.0.0', manifest: legacyManifest }),
						version({
							id: 'b-v2',
							mod_id: 'mod-b',
							version: '1.0.0+iostore',
							manifest: convertedManifest
						})
					],
					current_version_id: 'b-v2'
				})
			];

			function pinnedProfile(id: string, versionId: string, active: boolean): ModProfile {
				const base = profile('gamepass-1', ['mod-b']);
				return {
					...base,
					id,
					is_active: active,
					mods: base.mods.map((entry) => ({ ...entry, profile_id: id, mod_version_id: versionId }))
				};
			}

			state().profiles = {
				'gamepass-1': [
					pinnedProfile('gamepass-1-default', 'b-v1', true),
					pinnedProfile('gamepass-1-hard', 'b-v2', false)
				]
			};
			state().viewProfile('gamepass-1', 'gamepass-1-hard');

			render(ModList, { targetId: 'gamepass-1' });
			await openMenu('mod-b');
			expect(screen.getByRole('menuitem', { name: 'Convert for Game Pass' })).toBeTruthy();
		});

		it('shows no note when the pinned converted build matches the base current version', () => {
			state().mods = [
				mod({
					id: 'mod-b',
					name: 'Bravo',
					mod_type: 'pak',
					versions: [
						version({ id: 'b-v1', mod_id: 'mod-b', version: '1.0.0+iostore' }),
						version({ id: 'b-v2', mod_id: 'mod-b', version: '1.0.0' })
					],
					current_version_id: 'b-v2'
				})
			];
			state().profiles['gamepass-1'][0].mods[0].mod_version_id = 'b-v1';
			render(ModList, { targetId: 'gamepass-1' });

			expect(within(row('mod-b')).getByText('Game Pass')).toBeTruthy();
			expect(within(row('mod-b')).queryByText(/a newer version is installed/)).toBeNull();
		});
	});
	describe('layout', () => {
		it('shows cards in a grid by default and remembers a switch to the list', async () => {
			render(ModList, { targetId: 'client-abc' });
			const grid = screen.getByRole('button', { name: 'Grid view' });
			const list = screen.getByRole('button', { name: 'List view' });
			expect(grid.getAttribute('aria-pressed')).toBe('true');
			expect(document.querySelector('[data-view="grid"]')).toBeTruthy();

			await fireEvent.click(list);

			expect(list.getAttribute('aria-pressed')).toBe('true');
			expect(modsViewMode.current).toBe('list');
			expect(document.querySelector('[data-view="list"]')).toBeTruthy();
			expect(rowIds()).toEqual(['mod-a', 'mod-b']);
		});

		it('stands in the initials of a mod that has no thumbnail', () => {
			state().mods = [mod({ name: 'Better Pal Stats' })];
			render(ModList, { targetId: 'client-abc' });
			expect(within(row('mod-a')).getByText('BP')).toBeTruthy();
		});

		it('opens the details of the card clicked, and closes them', async () => {
			render(ModList, { targetId: 'client-abc' });
			expect(screen.queryByRole('complementary')).toBeNull();

			await openDetails('mod-b');
			expect(screen.getByRole('complementary', { name: 'Bravo' })).toBeTruthy();

			await fireEvent.click(within(drawer()).getByRole('button', { name: 'Close details' }));
			expect(screen.queryByRole('complementary')).toBeNull();
		});

		it('closes the details once their mod leaves the library', async () => {
			render(ModList, { targetId: 'client-abc' });
			await openDetails('mod-b');

			state().mods = state().mods.filter((entry) => entry.id !== 'mod-b');
			await vi.waitFor(() => expect(screen.queryByRole('complementary')).toBeNull());
		});

		it('filters to the enabled or disabled mods of the viewed profile', async () => {
			render(ModList, { targetId: 'client-abc' });

			await fireEvent.click(screen.getByRole('button', { name: 'Enabled' }));
			expect(rowIds()).toEqual(['mod-a']);

			await fireEvent.click(screen.getByRole('button', { name: 'Disabled' }));
			expect(rowIds()).toEqual(['mod-b']);

			await fireEvent.click(screen.getByRole('button', { name: 'All' }));
			expect(rowIds()).toEqual(['mod-a', 'mod-b']);
		});

		it('filters by the mod types in the library', async () => {
			render(ModList, { targetId: 'client-abc' });
			const types = screen.getByRole('group', { name: 'Type' });
			expect(
				within(types)
					.getAllByRole('button')
					.map((button) => button.textContent?.trim())
			).toEqual(['All types', 'Pak', 'UE4SS']);

			await fireEvent.click(within(types).getByRole('button', { name: 'Pak' }));
			expect(rowIds()).toEqual(['mod-b']);
		});
	});

	describe('Nexus update checks', () => {
		it('checks updates once on open, and not again on a re-render', async () => {
			const checkUpdates = vi.spyOn(nexusState(), 'checkUpdates');
			render(ModList, { targetId: 'client-abc' });

			expect(checkUpdates).toHaveBeenCalledTimes(1);
			expect(checkUpdates).toHaveBeenCalledWith('client-abc');

			state().mods = [...state().mods];
			await fireEvent.input(screen.getByPlaceholderText('Filter by name or author'), {
				target: { value: 'a' }
			});

			expect(checkUpdates).toHaveBeenCalledTimes(1);
		});

		it('forces a second check with "Check for updates"', async () => {
			const checkUpdates = vi.spyOn(nexusState(), 'checkUpdates');
			render(ModList, { targetId: 'client-abc' });
			checkUpdates.mockClear();

			await fireEvent.click(screen.getByRole('button', { name: 'Check for updates' }));

			expect(checkUpdates).toHaveBeenCalledWith('client-abc', { force: true });
		});

		it('renders the truncated sentence when the last run for this target said so', async () => {
			render(ModList, { targetId: 'client-abc' });

			await modUpdateCheckHandler.handle(
				{ target_id: 'client-abc', checked: 50, truncated: true, updates: [] },
				context as never
			);

			expect(
				await screen.findByText('Only the first 50 mods were checked.')
			).toBeTruthy();
		});

		it('renders a check refusal once above the list, and a retry is possible', async () => {
			const checkUpdates = vi.spyOn(nexusState(), 'checkUpdates');
			render(ModList, { targetId: 'client-abc' });
			checkUpdates.mockClear();

			await modUpdateCheckHandler.handle(
				{
					target_id: 'client-abc',
					error: { code: 'no_active_profile', message: 'client-abc has no active profile' }
				},
				context as never
			);

			expect(await screen.findByText('client-abc has no active profile')).toBeTruthy();

			await fireEvent.click(screen.getByRole('button', { name: 'Check for updates' }));

			expect(checkUpdates).toHaveBeenCalledWith('client-abc', { force: true });
		});

		it('sends no automatic check outside the desktop app', () => {
			vi.mocked(send).mockClear();
			env.desktop = 'false';
			render(ModList, { targetId: 'client-abc' });

			expect(
				vi.mocked(send).mock.calls.some((call) => call[0] === MessageType.MOD_UPDATE_CHECK)
			).toBe(false);
		});

		it('sends no automatic check in a remote session', () => {
			vi.mocked(send).mockClear();
			remote.active = true;
			render(ModList, { targetId: 'client-abc' });

			expect(
				vi.mocked(send).mock.calls.some((call) => call[0] === MessageType.MOD_UPDATE_CHECK)
			).toBe(false);
		});
	});
});
