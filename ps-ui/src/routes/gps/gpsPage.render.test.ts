// @vitest-environment jsdom
import '$utils/__tests__/fixtures/animatePolyfill';
import { render, screen, waitFor } from '@testing-library/svelte';
import userEvent from '@testing-library/user-event';
import { beforeEach, describe, expect, it, vi } from 'vitest';

import { installViewportStub } from '$utils/__tests__/fixtures/viewportStub';

const PALS_PER_PAGE = 30;
const TOTAL_SLOTS = 960;

const { appState, palEditor, modalState, upsState, sent } = vi.hoisted(() => ({
	appState: {
		gps: {} as Record<number, unknown>,
		loadingGps: false,
		gpsLoaded: false,
		hasGpsAvailable: false,
		selectedPlayer: undefined as unknown,
		selectedPal: undefined as unknown,
		settings: { new_pal_prefix: '', clone_prefix: '' },
		saveState: vi.fn()
	},
	palEditor: { open: vi.fn() },
	modalState: { showModal: vi.fn(), showConfirmModal: vi.fn() },
	upsState: { cloneToUps: vi.fn() },
	sent: [] as unknown[][]
}));

vi.mock('$states', () => ({
	getAppState: () => appState,
	getModalState: () => modalState,
	getToastState: () => ({ add: vi.fn() }),
	getUpsState: () => upsState,
	getPalEditorState: () => palEditor
}));

vi.mock('$lib/utils/websocketUtils', () => ({
	send: (...args: unknown[]) => {
		sent.push(args);
	}
}));

vi.mock('$components/modals', () => ({
	NumberInputModal: {},
	PalSelectModal: {},
	PalPresetSelectModal: {},
	FillPalsModal: {},
	CloneToUpsModal: {},
	ExportPalModal: {}
}));

vi.mock('$lib/data', () => ({
	palsData: {
		getByKey: (key: string) =>
			key && key !== 'None'
				? { localized_name: key, is_pal: true, pal_deck_index: 1, max_full_stomach: 150 }
				: undefined
	},
	elementsData: { elements: {}, getByKey: () => undefined },
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
const { default: GpsPage } = await import('./+page.svelte');

type TestPal = {
	instance_id: string;
	character_id: string;
	character_key: string;
	name: string;
	nickname?: string;
	level: number;
	hp: number;
	max_hp: number;
	storage_slot: number;
};

function makePal(index: number, name: string): TestPal {
	return {
		instance_id: `pal-${index}`,
		character_id: name,
		character_key: name,
		name,
		nickname: `${name} ${index}`,
		level: 10,
		hp: 100,
		max_hp: 100,
		storage_slot: index
	};
}

/** Keyed by slot index: the delete path turns the keys into `pal_indexes`. */
function loadStorage(count: number): void {
	const gps: Record<number, TestPal> = {};
	for (let index = 0; index < count; index++) {
		gps[index] = makePal(index, 'Blaze');
	}
	gps[count] = makePal(count, 'Splash');

	appState.gps = gps;
	appState.hasGpsAvailable = true;
	appState.gpsLoaded = true;
}

function pageButtons(): HTMLElement[] {
	return screen.queryAllByRole('button', { name: /^Page \d+$/ });
}

function badges(): HTMLElement[] {
	return screen.queryAllByTestId('pal-container-badge');
}

function badgeButton(index: number): HTMLElement {
	const all = badges();
	const button = all[index]?.querySelector('button');
	expect(button, `slot ${index} has no badge button`).toBeTruthy();
	return button as HTMLElement;
}

function renderPage() {
	return render(GpsPage, { props: {} });
}

beforeEach(() => {
	setViewport(1440);
	sent.length = 0;
	palEditor.open.mockClear();
	modalState.showModal.mockReset();
	modalState.showModal.mockResolvedValue(undefined);
	modalState.showConfirmModal.mockReset();
	modalState.showConfirmModal.mockResolvedValue(false);
	upsState.cloneToUps.mockReset();
	upsState.cloneToUps.mockResolvedValue(undefined);
	appState.gps = {};
	appState.loadingGps = false;
	appState.gpsLoaded = false;
	appState.hasGpsAvailable = false;
	appState.selectedPlayer = undefined;
	localStorage.clear();
});

describe('gps page', () => {
	it('says the storage is unavailable before it renders a container', () => {
		renderPage();
		expect(screen.getByText('Global Pal Storage is not available.')).toBeTruthy();
		expect(pageButtons()).toHaveLength(0);
		expect(badges()).toHaveLength(0);
	});

	it('waits on the fetch rather than showing an empty container', () => {
		appState.loadingGps = true;
		renderPage();
		expect(screen.getByText('Loading GPS')).toBeTruthy();
		expect(pageButtons()).toHaveLength(0);
	});

	it('pages the whole storage when nothing narrows it', () => {
		loadStorage(45);
		renderPage();
		// The pager windows at most 16 pages.
		expect(pageButtons()).toHaveLength(16);
		expect(
			screen.getByRole('navigation', { name: `Page 1 of ${TOTAL_SLOTS / PALS_PER_PAGE}` })
		).toBeTruthy();
	});

	it('counts whole pages for a filtered list that is not a multiple of the page size', async () => {
		const user = userEvent.setup();
		loadStorage(45);
		renderPage();

		await user.click(screen.getByRole('button', { name: 'Filter' }));
		await user.type(screen.getByPlaceholderText(/Search by name/i), 'Blaze');

		// 45 of 46 pals match, at 30 a page: two pages, not `Math.ceil(45) / 30`.
		const pages = pageButtons();
		expect(pages).toHaveLength(2);
		expect(pages.map((button) => button.textContent?.trim())).toEqual(['1', '2']);
	});

	it('drops the pager entirely once one page holds the matches', async () => {
		const user = userEvent.setup();
		loadStorage(45);
		renderPage();

		await user.click(screen.getByRole('button', { name: 'Filter' }));
		await user.type(screen.getByPlaceholderText(/Search by name/i), 'Splash');

		expect(pageButtons()).toHaveLength(0);
	});

	it('offers the always-available operations with nothing selected', () => {
		loadStorage(3);
		renderPage();

		const rail = document.querySelector('#gps-actions');
		expect(rail).toBeTruthy();
		const labels = Array.from(rail?.querySelectorAll('button') ?? []).map((button) =>
			button.getAttribute('aria-label')
		);
		expect(labels).toEqual(['Add all Pals to GPS', 'Select all in GPS']);
	});

	it('hands the action surface to the view once something is selected', async () => {
		const user = userEvent.setup();
		loadStorage(3);
		renderPage();

		await user.click(screen.getByLabelText('Select all in GPS'));

		expect(document.querySelector('#gps-actions')).toBeNull();
		expect(screen.getByText('4 Pals selected')).toBeTruthy();
		expect(screen.getByLabelText('Delete selected Pals')).toBeTruthy();
		expect(screen.getByLabelText('Clone 4 Pals to Universal Pal Storage')).toBeTruthy();
		expect(screen.getByLabelText('Clone 4 Pals to Player')).toBeTruthy();
	});

	it('selects every stored pal, not every slot in the storage', async () => {
		const user = userEvent.setup();
		loadStorage(45);
		renderPage();

		await user.click(screen.getByLabelText('Select all in GPS'));

		expect(screen.getByText('46 Pals selected')).toBeTruthy();
	});

	it('clears a complete selection when select-all runs again', async () => {
		const user = userEvent.setup();
		loadStorage(3);
		renderPage();

		await user.click(screen.getByLabelText('Select all in GPS'));
		await user.click(screen.getByLabelText('Select all in GPS'));

		expect(screen.queryByText(/Pals selected/)).toBeNull();
		expect(document.querySelector('#gps-actions')).toBeTruthy();
	});

	it('toggles one pal with a Ctrl-click on its grid badge', async () => {
		const user = userEvent.setup();
		loadStorage(3);
		renderPage();

		await user.keyboard('{Control>}');
		await user.click(badges()[0]);
		await user.keyboard('{/Control}');

		expect(screen.getByText('1 Pal selected')).toBeTruthy();
		expect(palEditor.open).not.toHaveBeenCalled();
	});

	it('will not select an empty slot', async () => {
		const user = userEvent.setup();
		loadStorage(1);
		renderPage();

		await user.keyboard('{Control>}');
		await user.click(badges()[5]);
		await user.keyboard('{/Control}');

		expect(screen.queryByText(/Pals? selected/)).toBeNull();
	});

	it('clears the selection from its own action', async () => {
		const user = userEvent.setup();
		loadStorage(3);
		renderPage();

		await user.click(screen.getByLabelText('Select all in GPS'));
		await user.click(screen.getByLabelText('Clear selected Pals'));

		expect(screen.queryByText(/Pals selected/)).toBeNull();
	});

	it('fills the storage from the add-all action', async () => {
		const user = userEvent.setup();
		loadStorage(3);
		renderPage();

		await user.click(screen.getByLabelText('Add all Pals to GPS'));

		await waitFor(() => expect(modalState.showModal).toHaveBeenCalledTimes(1));
		expect(modalState.showModal.mock.calls[0][1]).toMatchObject({ target: 'gps' });
	});

	it('offers the selection to the preset modal', async () => {
		const user = userEvent.setup();
		loadStorage(3);
		renderPage();

		await user.click(screen.getByLabelText('Select all in GPS'));
		await user.click(screen.getByLabelText('Apply preset to selected Pals'));

		await waitFor(() => expect(modalState.showModal).toHaveBeenCalledTimes(1));
		expect(modalState.showModal.mock.calls[0][1].selectedPals).toEqual([
			{ character_id: 'Blaze', character_key: 'Blaze' },
			{ character_id: 'Blaze', character_key: 'Blaze' },
			{ character_id: 'Blaze', character_key: 'Blaze' },
			{ character_id: 'Splash', character_key: 'Splash' }
		]);
	});

	it('clones the selection to the universal storage, with no owning player', async () => {
		const user = userEvent.setup();
		loadStorage(3);
		renderPage();
		modalState.showModal.mockResolvedValue({ collectionId: 'collection-1', tags: [], notes: '' });

		await user.click(screen.getByLabelText('Select all in GPS'));
		await user.click(screen.getByLabelText('Clone 4 Pals to Universal Pal Storage'));

		await waitFor(() => expect(upsState.cloneToUps).toHaveBeenCalledTimes(1));
		expect(upsState.cloneToUps).toHaveBeenCalledWith(
			['pal-0', 'pal-1', 'pal-2', 'pal-3'],
			'gps',
			undefined,
			'collection-1',
			undefined,
			undefined
		);
	});

	it('clones the selection to the player the export modal named', async () => {
		const user = userEvent.setup();
		loadStorage(3);
		renderPage();
		modalState.showModal.mockResolvedValue({ target: 'palbox', playerId: 'player-9' });

		await user.click(screen.getByLabelText('Select all in GPS'));
		await user.click(screen.getByLabelText('Clone 4 Pals to Player'));

		await waitFor(() => expect(sent).toHaveLength(1));
		const [type, payload] = sent[0] as [string, Record<string, unknown>];
		expect(String(type)).toBe('clone_gps_pal_to_player');
		expect(payload).toEqual({
			pal_ids: ['pal-0', 'pal-1', 'pal-2', 'pal-3'],
			destination_type: 'palbox',
			destination_player_uid: 'player-9'
		});
		await waitFor(() => expect(screen.queryByText(/Pals selected/)).toBeNull());
	});

	it('sends nothing when the export modal comes back pointing at the gps itself', async () => {
		const user = userEvent.setup();
		loadStorage(3);
		renderPage();
		modalState.showModal.mockResolvedValue({ target: 'gps', playerId: 'player-9' });

		await user.click(screen.getByLabelText('Select all in GPS'));
		await user.click(screen.getByLabelText('Clone 4 Pals to Player'));

		await waitFor(() => expect(modalState.showModal).toHaveBeenCalledTimes(1));
		expect(sent).toHaveLength(0);
		expect(screen.getByText('4 Pals selected')).toBeTruthy();
	});

	it('deletes the selection by slot index, as numbers', async () => {
		const user = userEvent.setup();
		loadStorage(3);
		renderPage();
		modalState.showConfirmModal.mockResolvedValue(true);

		await user.click(screen.getByLabelText('Select all in GPS'));
		await user.click(screen.getByLabelText('Delete selected Pals'));

		await waitFor(() => expect(sent).toHaveLength(1));
		const [type, payload] = sent[0] as [string, { pal_indexes: number[] }];
		expect(String(type)).toBe('delete_gps_pals');
		expect(payload).toEqual({ pal_indexes: [0, 1, 2, 3] });
		expect(payload.pal_indexes.every((index) => typeof index === 'number')).toBe(true);
		await waitFor(() => expect(screen.queryByText(/Pals selected/)).toBeNull());
	});

	it('leaves the storage alone when the delete is not confirmed', async () => {
		const user = userEvent.setup();
		loadStorage(3);
		renderPage();

		await user.click(screen.getByLabelText('Select all in GPS'));
		await user.click(screen.getByLabelText('Delete selected Pals'));

		// The selection clearing marks the handler as finished.
		await waitFor(() => expect(screen.queryByText(/Pals selected/)).toBeNull());
		expect(sent).toHaveLength(0);
	});

	it('adds a pal to the empty slot that was tapped, keyed by its number', async () => {
		const user = userEvent.setup();
		loadStorage(3);
		renderPage();
		modalState.showModal.mockResolvedValue(['Blaze', 'Nickname']);

		await user.click(badgeButton(5));

		await waitFor(() => expect(sent).toHaveLength(1));
		const [type, payload] = sent[0] as [string, { storage_slot: number }];
		expect(String(type)).toBe('add_gps_pal');
		expect(payload).toMatchObject({
			character_id: 'Blaze',
			nickname: 'Nickname',
			storage_slot: 5
		});
		expect(typeof payload.storage_slot).toBe('number');
	});

	it('opens a stored pal in the editor when the view asks for it', async () => {
		const user = userEvent.setup();
		loadStorage(3);
		renderPage();

		await user.click(screen.getByLabelText('List View'));
		// The portrait renders its own button, but never under this exact name.
		const rows = screen.queryAllByRole('button', { name: 'Blaze 0' });
		expect(rows).toHaveLength(1);
		await user.click(rows[0]);

		expect(palEditor.open).toHaveBeenCalledTimes(1);
	});
});
