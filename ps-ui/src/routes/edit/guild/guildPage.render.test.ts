// @vitest-environment jsdom
import '$utils/__tests__/fixtures/animatePolyfill';
import { render, screen, waitFor } from '@testing-library/svelte';
import userEvent from '@testing-library/user-event';
import { beforeEach, describe, expect, it, vi } from 'vitest';

import { installViewportStub } from '$utils/__tests__/fixtures/viewportStub';

const { appState, palEditor, modalState, sent } = vi.hoisted(() => ({
	appState: {
		selectedPlayer: undefined as unknown,
		guilds: {} as Record<string, unknown>,
		loadingGuild: false,
		clipboardItem: null as unknown,
		settings: { debug_mode: false, new_pal_prefix: '', clone_prefix: '' }
	},
	palEditor: { open: vi.fn() },
	modalState: { showModal: vi.fn(), showConfirmModal: vi.fn() },
	sent: [] as unknown[][]
}));

vi.mock('$states', () => ({
	getAppState: () => appState,
	getModalState: () => modalState,
	getToastState: () => ({ add: vi.fn() }),
	getPalEditorState: () => palEditor
}));

vi.mock('$lib/utils/websocketUtils', () => ({
	send: (...args: unknown[]) => {
		sent.push(args);
	}
}));

vi.mock('$app/navigation', () => ({ goto: vi.fn() }));

vi.mock('$components/modals', () => ({
	PalSelectModal: {},
	NumberInputModal: {},
	PalPresetSelectModal: {},
	NumberSliderModal: {},
	TextInputModal: {}
}));

// Stubbed: each belongs to a tab this suite never opens.
vi.mock('$components/shared', () => ({ ItemBadge: {} }));
vi.mock('$components/presets', () => ({ StoragePresets: {} }));
vi.mock('$components/guilds', () => ({ LabResearchControls: {} }));
vi.mock('$components/guilds/LabResearch.svelte', () => ({ default: {} }));

vi.mock('$lib/data', () => ({
	palsData: {
		getByKey: (key: string) =>
			key && key !== 'None'
				? { localized_name: key, is_pal: true, pal_deck_index: 1, max_full_stomach: 150 }
				: undefined
	},
	buildingsData: { getByKey: () => undefined },
	itemsData: { getByKey: () => undefined },
	presetsData: { presetProfiles: {} }
}));

vi.mock('$utils', async (importOriginal) => {
	const actual = await importOriginal<Record<string, unknown>>();
	return {
		...actual,
		assetLoader: {
			loadImage: () => 'data:image/gif;base64,',
			loadMenuImage: () => 'data:image/gif;base64,'
		}
	};
});

const { setViewport } = installViewportStub();
const { default: GuildPage } = await import('./+page.svelte');

type TestPal = {
	instance_id: string;
	character_id: string;
	character_key: string;
	name: string;
	nickname: string;
	level: number;
	hp: number;
	max_hp: number;
	sanity: number;
	stomach: number;
	gender: string;
	is_sick: boolean;
	storage_slot: number;
	storage_id: string;
};

function makePal(baseIndex: number, slot: number, name: string): TestPal {
	return {
		instance_id: `pal-${baseIndex}-${slot}`,
		character_id: name,
		character_key: name,
		name,
		nickname: `${name} ${slot}`,
		level: 10,
		hp: 50,
		max_hp: 100,
		sanity: 40,
		stomach: 10,
		gender: 'Male',
		is_sick: false,
		storage_slot: slot,
		storage_id: `container-${baseIndex}`
	};
}

function makeBase(index: number, name: string, slotCount: number, pals: TestPal[]) {
	return {
		id: `base-${index}`,
		name,
		slot_count: slotCount,
		container_id: `container-${index}`,
		pals: Object.fromEntries(pals.map((pal) => [pal.instance_id, pal])),
		storage_containers: {}
	};
}

/** Base 1: six slots, three "Blaze". Base 2: four slots, one "Splash". Base 3: four empty slots. */
function loadGuild(): void {
	const bases = {
		'base-1': makeBase(1, 'Home', 6, [
			makePal(1, 0, 'Blaze'),
			makePal(1, 1, 'Blaze'),
			makePal(1, 2, 'Blaze')
		]),
		'base-2': makeBase(2, 'Outpost', 4, [makePal(2, 0, 'Splash')]),
		'base-3': makeBase(3, 'Quarry', 4, [])
	};

	appState.guilds = {
		'guild-1': { id: 'guild-1', name: 'Testers', base_camp_level: 5, bases, state: 0 }
	};
	appState.selectedPlayer = { guild_id: 'guild-1', level: 50 };
}

function loadWideBase(): void {
	const pals = Array.from({ length: 40 }, (_, slot) => makePal(1, slot, 'Blaze'));
	appState.guilds = {
		'guild-1': {
			id: 'guild-1',
			name: 'Testers',
			base_camp_level: 5,
			bases: { 'base-1': makeBase(1, 'Home', 45, pals) },
			state: 0
		}
	};
	appState.selectedPlayer = { guild_id: 'guild-1', level: 50 };
}

function badges(): HTMLElement[] {
	return screen.queryAllByTestId('pal-container-badge');
}

function badgeButton(index: number): HTMLElement {
	const button = badges()[index]?.querySelector('button');
	expect(button, `slot ${index} has no badge button`).toBeTruthy();
	return button as HTMLElement;
}

function palPageButtons(): HTMLElement[] {
	return screen.queryAllByRole('button', { name: /^Page \d+$/ });
}

function baseBubbles(): HTMLElement[] {
	return Array.from(document.querySelectorAll('#guild-pager button')).filter((button) =>
		/^\d+$/.test(button.textContent?.trim() ?? '')
	) as HTMLElement[];
}

function currentBaseName(): string {
	return document.querySelector('#guild-base-name')?.textContent?.trim() ?? '';
}

function railLabels(id: string): (string | null)[] {
	const rail = document.querySelector(id);
	return Array.from(rail?.querySelectorAll('button') ?? []).map((button) =>
		button.getAttribute('aria-label')
	);
}

function renderPage() {
	return render(GuildPage, { props: {} });
}

async function openFilters(user: ReturnType<typeof userEvent.setup>) {
	await user.click(screen.getByRole('button', { name: 'Filter' }));
	return screen.getByPlaceholderText('Search by name or nickname');
}

beforeEach(() => {
	setViewport(1440);
	sent.length = 0;
	palEditor.open.mockClear();
	modalState.showModal.mockReset();
	modalState.showModal.mockResolvedValue(undefined);
	modalState.showConfirmModal.mockReset();
	modalState.showConfirmModal.mockResolvedValue(false);
	appState.guilds = {};
	appState.selectedPlayer = undefined;
	appState.loadingGuild = false;
	localStorage.clear();
});

describe('guild base pals', () => {
	it('asks for a player before it renders a container', () => {
		renderPage();
		expect(screen.getByText('Select a Player to view Guild')).toBeTruthy();
		expect(badges()).toHaveLength(0);
	});

	it('waits on the fetch rather than showing an empty base', () => {
		appState.loadingGuild = true;
		appState.selectedPlayer = { guild_id: 'guild-1', level: 50 };
		renderPage();
		expect(badges()).toHaveLength(0);
		expect(document.querySelector('#guild-pager')).toBeNull();
	});

	it('shows every slot of the current base, placeholders and all', () => {
		loadGuild();
		renderPage();
		expect(badges()).toHaveLength(6);
	});

	it('never renders a pal pager, however wide the base', () => {
		loadWideBase();
		renderPage();

		expect(palPageButtons()).toHaveLength(0);
		expect(screen.queryByRole('navigation', { name: /^Page \d+ of \d+$/ })).toBeNull();
		expect(badges()).toHaveLength(45);
	});

	it('keeps the base selector, one bubble per base', () => {
		loadGuild();
		renderPage();

		expect(document.querySelector('#guild-pager')).toBeTruthy();
		expect(baseBubbles().map((button) => button.textContent?.trim())).toEqual(['1', '2', '3']);
		expect(currentBaseName()).toBe('Home');
	});

	it('moves between bases from the selector', async () => {
		const user = userEvent.setup();
		loadGuild();
		renderPage();

		await user.click(screen.getByRole('button', { name: 'Next' }));
		expect(currentBaseName()).toBe('Outpost');
		expect(badges()).toHaveLength(4);

		await user.click(screen.getByRole('button', { name: 'Previous' }));
		expect(currentBaseName()).toBe('Home');
	});

	it('jumps straight to a base from its bubble', async () => {
		const user = userEvent.setup();
		loadGuild();
		renderPage();

		await user.click(baseBubbles()[2]);
		expect(currentBaseName()).toBe('Quarry');
	});

	it('wraps backwards off the first base onto the last', async () => {
		const user = userEvent.setup();
		loadGuild();
		renderPage();

		await user.click(screen.getByRole('button', { name: 'Previous' }));
		expect(currentBaseName()).toBe('Quarry');
	});

	it('wraps forwards off the last base onto the first', async () => {
		const user = userEvent.setup();
		loadGuild();
		renderPage();

		await user.click(baseBubbles()[2]);
		await user.click(screen.getByRole('button', { name: 'Next' }));
		expect(currentBaseName()).toBe('Home');
	});

	it('still pages the bases from the keyboard', async () => {
		const user = userEvent.setup();
		loadGuild();
		renderPage();

		await user.keyboard('e');
		expect(currentBaseName()).toBe('Outpost');

		await user.keyboard('q');
		expect(currentBaseName()).toBe('Home');
	});

	it('offers only the always-available operations with nothing selected', () => {
		loadGuild();
		renderPage();

		expect(railLabels('#guild-pals-actions')).toEqual([
			'Add a new Pal to your Base',
			'Select all in current base',
			'Heal all in Base'
		]);
	});

	it('hands the action surface to the view once something is selected', async () => {
		const user = userEvent.setup();
		loadGuild();
		renderPage();

		await user.click(screen.getByLabelText('Select all in current base'));

		expect(document.querySelector('#guild-pals-actions')).toBeNull();
		expect(screen.getByText('3 Pals selected')).toBeTruthy();
		expect(screen.getByLabelText('Apply preset to selected Pals')).toBeTruthy();
		expect(screen.getByLabelText('Heal selected Pals')).toBeTruthy();
		expect(screen.getByLabelText('Delete selected Pals')).toBeTruthy();
		expect(screen.getByLabelText('Clear selected Pals')).toBeTruthy();
	});

	it('selects the pals of the current base, not its empty slots', async () => {
		const user = userEvent.setup();
		loadGuild();
		renderPage();

		await user.click(screen.getByLabelText('Select all in current base'));

		expect(screen.getByText('3 Pals selected')).toBeTruthy();
	});

	it('clears a complete selection when select-all runs again', async () => {
		const user = userEvent.setup();
		loadGuild();
		renderPage();

		await user.click(screen.getByLabelText('Select all in current base'));
		await user.click(screen.getByLabelText('Select all in current base'));

		expect(screen.queryByText(/Pals? selected/)).toBeNull();
		expect(document.querySelector('#guild-pals-actions')).toBeTruthy();
	});

	it('toggles one pal with a Ctrl-click on its grid badge', async () => {
		const user = userEvent.setup();
		loadGuild();
		renderPage();

		await user.keyboard('{Control>}');
		await user.click(badges()[0]);
		await user.keyboard('{/Control}');

		expect(screen.getByText('1 Pal selected')).toBeTruthy();
		expect(palEditor.open).not.toHaveBeenCalled();
	});

	it('will not select an empty slot', async () => {
		const user = userEvent.setup();
		loadGuild();
		renderPage();

		await user.keyboard('{Control>}');
		await user.click(badges()[4]);
		await user.keyboard('{/Control}');

		expect(screen.queryByText(/Pals? selected/)).toBeNull();
	});

	it('clears the selection from its own action', async () => {
		const user = userEvent.setup();
		loadGuild();
		renderPage();

		await user.click(screen.getByLabelText('Select all in current base'));
		await user.click(screen.getByLabelText('Clear selected Pals'));

		expect(screen.queryByText(/Pals? selected/)).toBeNull();
	});

	it('adds a pal to the base on screen', async () => {
		const user = userEvent.setup();
		loadGuild();
		renderPage();
		modalState.showModal.mockResolvedValue(['Blaze', 'Nickname']);

		await user.click(screen.getByLabelText('Add a new Pal to your Base'));

		await waitFor(() => expect(sent).toHaveLength(1));
		const [type, payload] = sent[0] as [string, Record<string, unknown>];
		expect(String(type)).toBe('add_pal');
		expect(payload).toMatchObject({
			guild_id: 'guild-1',
			base_id: 'base-1',
			character_id: 'Blaze',
			nickname: 'Nickname',
			container_id: 'container-1'
		});
		expect(payload.storage_slot).toBeUndefined();
	});

	it('adds to the base the selector moved to, not the first one', async () => {
		const user = userEvent.setup();
		loadGuild();
		renderPage();
		modalState.showModal.mockResolvedValue(['Splash', 'Nickname']);

		await user.click(screen.getByRole('button', { name: 'Next' }));
		await user.click(screen.getByLabelText('Add a new Pal to your Base'));

		await waitFor(() => expect(sent).toHaveLength(1));
		const [, payload] = sent[0] as [string, Record<string, unknown>];
		expect(payload).toMatchObject({ base_id: 'base-2', container_id: 'container-2' });
	});

	it('adds a pal to the empty slot that was tapped', async () => {
		const user = userEvent.setup();
		loadGuild();
		renderPage();
		modalState.showModal.mockResolvedValue(['Blaze', 'Nickname']);

		await user.click(badgeButton(4));

		await waitFor(() => expect(sent).toHaveLength(1));
		const [type, payload] = sent[0] as [string, Record<string, unknown>];
		expect(String(type)).toBe('add_pal');
		expect(payload).toMatchObject({ base_id: 'base-1', storage_slot: 4 });
	});

	it('heals the whole base from its own row', async () => {
		const user = userEvent.setup();
		loadGuild();
		renderPage();

		await user.click(screen.getByLabelText('Heal all in Base'));

		await waitFor(() => expect(sent).toHaveLength(1));
		const [type, payload] = sent[0] as [string, Record<string, unknown>];
		expect(String(type)).toBe('heal_all_pals');
		expect(payload).toEqual({ guild_id: 'guild-1', base_id: 'base-1' });
	});

	it('heals the selection by id, and drops it afterwards', async () => {
		const user = userEvent.setup();
		loadGuild();
		renderPage();

		await user.click(screen.getByLabelText('Select all in current base'));
		await user.click(screen.getByLabelText('Heal selected Pals'));

		await waitFor(() => expect(sent).toHaveLength(1));
		const [type, ids] = sent[0] as [string, string[]];
		expect(String(type)).toBe('heal_pals');
		expect(ids).toEqual(['pal-1-0', 'pal-1-1', 'pal-1-2']);
		await waitFor(() => expect(screen.queryByText(/Pals? selected/)).toBeNull());
	});

	it('offers the selection to the preset modal', async () => {
		const user = userEvent.setup();
		loadGuild();
		renderPage();

		await user.click(screen.getByLabelText('Select all in current base'));
		await user.click(screen.getByLabelText('Apply preset to selected Pals'));

		await waitFor(() => expect(modalState.showModal).toHaveBeenCalledTimes(1));
		expect(modalState.showModal.mock.calls[0][1].selectedPals).toEqual([
			{ character_id: 'Blaze', character_key: 'Blaze' },
			{ character_id: 'Blaze', character_key: 'Blaze' },
			{ character_id: 'Blaze', character_key: 'Blaze' }
		]);
	});

	it('deletes the selection from the base it belongs to', async () => {
		const user = userEvent.setup();
		loadGuild();
		renderPage();
		modalState.showConfirmModal.mockResolvedValue(true);

		await user.click(screen.getByLabelText('Select all in current base'));
		await user.click(screen.getByLabelText('Delete selected Pals'));

		await waitFor(() => expect(sent).toHaveLength(1));
		const [type, payload] = sent[0] as [string, Record<string, unknown>];
		expect(String(type)).toBe('delete_pals');
		expect(payload).toEqual({
			guild_id: 'guild-1',
			base_id: 'base-1',
			pal_ids: ['pal-1-0', 'pal-1-1', 'pal-1-2']
		});
		await waitFor(() => expect(screen.queryByText(/Pals? selected/)).toBeNull());
	});

	it('leaves the base alone when the delete is not confirmed', async () => {
		const user = userEvent.setup();
		loadGuild();
		renderPage();

		await user.click(screen.getByLabelText('Select all in current base'));
		await user.click(screen.getByLabelText('Delete selected Pals'));

		// The selection drops once the handler finishes, sent or not.
		await waitFor(() => expect(screen.queryByText(/Pals? selected/)).toBeNull());
		expect(sent).toHaveLength(0);
	});

	it('opens a stored pal in the editor from the list view', async () => {
		const user = userEvent.setup();
		loadGuild();
		renderPage();

		await user.click(screen.getByLabelText('List View'));
		// Only the row's overlay button is named exactly by the nickname.
		const rows = screen.queryAllByRole('button', { name: 'Blaze 0' });
		expect(rows).toHaveLength(1);
		await user.click(rows[0]);

		expect(palEditor.open).toHaveBeenCalledTimes(1);
	});
});

describe('guild base pal search', () => {
	it('narrows to the matches, dropping the empty slots', async () => {
		const user = userEvent.setup();
		loadGuild();
		renderPage();

		const search = await openFilters(user);
		await user.type(search, 'Blaze');

		await waitFor(() => expect(badges()).toHaveLength(3));
	});

	it('reaches pals in bases other than the one on screen', async () => {
		const user = userEvent.setup();
		loadGuild();
		renderPage();

		const search = await openFilters(user);
		await user.type(search, 'Splash');

		await waitFor(() => expect(badges()).toHaveLength(1));
		expect(currentBaseName()).toBe('Home');
	});

	it('goes back to the whole base when the query is cleared', async () => {
		const user = userEvent.setup();
		loadGuild();
		renderPage();

		const search = await openFilters(user);
		await user.type(search, 'Splash');
		await waitFor(() => expect(badges()).toHaveLength(1));

		await user.clear(search);
		await waitFor(() => expect(badges()).toHaveLength(6));
	});

	it('matches nothing rather than falling back to the base', async () => {
		const user = userEvent.setup();
		loadGuild();
		renderPage();

		const search = await openFilters(user);
		await user.type(search, 'Nothing');

		await waitFor(() => expect(badges()).toHaveLength(0));
	});
});

describe('guild tabs outside the pals section', () => {
	it('keeps the guild, base and delete controls beside the container', () => {
		loadGuild();
		renderPage();

		expect(document.querySelector('#guild-name')).toBeTruthy();
		expect(document.querySelector('#guild-level')).toBeTruthy();
		expect(document.querySelector('#guild-delete')).toBeTruthy();
		expect(document.querySelector('#guild-base-name')).toBeTruthy();
		expect(document.querySelector('#guild-tabs')).toBeTruthy();
	});

	it('swaps the container out for the storage tab, and back', async () => {
		const user = userEvent.setup();
		loadGuild();
		renderPage();

		await user.click(document.querySelector('#guild-tab-storage') as HTMLElement);
		expect(badges()).toHaveLength(0);
		expect(screen.getByText('No Storage Containers')).toBeTruthy();
		expect(document.querySelector('#guild-pals-actions')).toBeNull();
		expect(document.querySelector('#guild-pager')).toBeTruthy();

		await user.click(document.querySelector('#guild-tab-pals') as HTMLElement);
		expect(badges()).toHaveLength(6);
		expect(document.querySelector('#guild-pals-actions')).toBeTruthy();
	});

	it('swaps the container out for the chest tab', async () => {
		const user = userEvent.setup();
		loadGuild();
		renderPage();

		await user.click(document.querySelector('#guild-tab-chest') as HTMLElement);
		expect(badges()).toHaveLength(0);
		expect(document.querySelector('#guild-pager')).toBeTruthy();
	});
});
