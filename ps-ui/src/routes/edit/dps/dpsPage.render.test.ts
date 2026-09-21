// @vitest-environment jsdom
import '$utils/__tests__/fixtures/animatePolyfill';
import { render, screen, waitFor } from '@testing-library/svelte';
import userEvent from '@testing-library/user-event';
import { beforeEach, describe, expect, it, vi } from 'vitest';

import { installViewportStub } from '$utils/__tests__/fixtures/viewportStub';

const PALS_PER_PAGE = 30;
const TOTAL_SLOTS = 9600;

const { appState, palEditor, modalState, upsState, sent } = vi.hoisted(() => ({
	appState: {
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
	CloneToUpsModal: {}
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
const { default: DpsPage } = await import('./+page.svelte');

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

/** Keyed by slot index: the delete path turns those keys into `pal_indexes`. */
function selectPlayer(count: number): void {
	const dps: Record<number, TestPal> = {};
	for (let index = 0; index < count; index++) {
		dps[index] = makePal(index, 'Blaze');
	}
	dps[count] = makePal(count, 'Splash');

	appState.selectedPlayer = {
		uid: 'player-1',
		level: 50,
		dps
	};
}

function pageButtons(): HTMLElement[] {
	return screen.queryAllByRole('button', { name: /^Page \d+$/ });
}

function badgeButton(index: number): HTMLElement {
	const badges = screen.getAllByTestId('pal-container-badge');
	const button = badges[index].querySelector('button');
	expect(button, `slot ${index} has no badge button`).toBeTruthy();
	return button as HTMLElement;
}

function renderPage() {
	return render(DpsPage, { props: {} });
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
	appState.selectedPlayer = undefined;
	localStorage.clear();
});

describe('dps page', () => {
	it('asks for a player before it renders a container', () => {
		renderPage();
		expect(screen.getByRole('heading', { name: /Select a Player/i })).toBeTruthy();
		expect(pageButtons()).toHaveLength(0);
	});

	it('pages the whole storage when nothing narrows it', () => {
		selectPlayer(45);
		renderPage();
		expect(pageButtons()).toHaveLength(16);
		expect(
			screen.getByRole('navigation', { name: `Page 1 of ${TOTAL_SLOTS / PALS_PER_PAGE}` })
		).toBeTruthy();
	});

	it('counts whole pages for a filtered list that is not a multiple of the page size', async () => {
		const user = userEvent.setup();
		selectPlayer(45);
		renderPage();

		await user.click(screen.getByRole('button', { name: 'Filter' }));
		await user.type(screen.getByPlaceholderText(/Search by name/i), 'Blaze');

		const pages = pageButtons();
		expect(pages).toHaveLength(2);
		expect(pages.map((button) => button.textContent?.trim())).toEqual(['1', '2']);
	});

	it('drops the pager entirely once one page holds the matches', async () => {
		const user = userEvent.setup();
		selectPlayer(45);
		renderPage();

		await user.click(screen.getByRole('button', { name: 'Filter' }));
		await user.type(screen.getByPlaceholderText(/Search by name/i), 'Splash');

		expect(pageButtons()).toHaveLength(0);
	});

	it('offers the always-available operations with nothing selected', () => {
		selectPlayer(3);
		renderPage();

		const rail = document.querySelector('#dps-actions');
		expect(rail).toBeTruthy();
		const labels = Array.from(rail?.querySelectorAll('button') ?? []).map((button) =>
			button.getAttribute('aria-label')
		);
		expect(labels).toEqual(['Add all Pals to DPS', 'Select all in DPS']);
	});

	it('hands the action surface to the view once something is selected', async () => {
		const user = userEvent.setup();
		selectPlayer(3);
		renderPage();

		await user.click(screen.getByLabelText('Select all in DPS'));

		expect(document.querySelector('#dps-actions')).toBeNull();
		expect(screen.getByText('4 Pals selected')).toBeTruthy();
		expect(screen.getByLabelText('Delete selected Pals')).toBeTruthy();
	});

	it('selects every stored pal, not every slot in the storage', async () => {
		const user = userEvent.setup();
		selectPlayer(45);
		renderPage();

		await user.click(screen.getByLabelText('Select all in DPS'));

		expect(screen.getByText('46 Pals selected')).toBeTruthy();
	});

	it('clears a complete selection when select-all runs again', async () => {
		const user = userEvent.setup();
		selectPlayer(3);
		renderPage();

		await user.click(screen.getByLabelText('Select all in DPS'));
		await user.click(screen.getByLabelText('Select all in DPS'));

		expect(screen.queryByText(/Pals selected/)).toBeNull();
		expect(document.querySelector('#dps-actions')).toBeTruthy();
	});

	it('toggles one pal with a Ctrl-click on its grid badge', async () => {
		const user = userEvent.setup();
		selectPlayer(3);
		renderPage();

		const badges = screen.getAllByTestId('pal-container-badge');
		await user.keyboard('{Control>}');
		await user.click(badges[0]);
		await user.keyboard('{/Control}');

		expect(screen.getByText('1 Pal selected')).toBeTruthy();
		expect(palEditor.open).not.toHaveBeenCalled();
	});

	it('will not select an empty slot', async () => {
		const user = userEvent.setup();
		selectPlayer(1);
		renderPage();

		const badges = screen.getAllByTestId('pal-container-badge');
		await user.keyboard('{Control>}');
		await user.click(badges[5]);
		await user.keyboard('{/Control}');

		expect(screen.queryByText(/Pals? selected/)).toBeNull();
	});

	it('clears the selection from its own action', async () => {
		const user = userEvent.setup();
		selectPlayer(3);
		renderPage();

		await user.click(screen.getByLabelText('Select all in DPS'));
		await user.click(screen.getByLabelText('Clear selected Pals'));

		expect(screen.queryByText(/Pals selected/)).toBeNull();
	});

	it('fills the storage from the add-all action', async () => {
		const user = userEvent.setup();
		selectPlayer(3);
		renderPage();

		await user.click(screen.getByLabelText('Add all Pals to DPS'));

		await waitFor(() => expect(modalState.showModal).toHaveBeenCalledTimes(1));
		expect(modalState.showModal.mock.calls[0][1]).toMatchObject({ target: 'dps' });
	});

	it('offers the selection to the preset modal', async () => {
		const user = userEvent.setup();
		selectPlayer(3);
		renderPage();

		await user.click(screen.getByLabelText('Select all in DPS'));
		await user.click(screen.getByLabelText('Apply preset to selected Pals'));

		await waitFor(() => expect(modalState.showModal).toHaveBeenCalledTimes(1));
		expect(modalState.showModal.mock.calls[0][1].selectedPals).toEqual([
			{ character_id: 'Blaze', character_key: 'Blaze' },
			{ character_id: 'Blaze', character_key: 'Blaze' },
			{ character_id: 'Blaze', character_key: 'Blaze' },
			{ character_id: 'Splash', character_key: 'Splash' }
		]);
	});

	it('clones the selection to the universal storage', async () => {
		const user = userEvent.setup();
		selectPlayer(3);
		renderPage();
		modalState.showModal.mockResolvedValue({ collectionId: 'collection-1', tags: [], notes: '' });

		await user.click(screen.getByLabelText('Select all in DPS'));
		await user.click(screen.getByLabelText('Clone 4 Pals to Universal Pal Storage'));

		await waitFor(() => expect(upsState.cloneToUps).toHaveBeenCalledTimes(1));
		expect(upsState.cloneToUps).toHaveBeenCalledWith(
			['pal-0', 'pal-1', 'pal-2', 'pal-3'],
			'dps',
			'player-1',
			'collection-1',
			undefined,
			undefined
		);
	});

	it('deletes the selection by slot index, as numbers', async () => {
		const user = userEvent.setup();
		selectPlayer(3);
		renderPage();
		modalState.showConfirmModal.mockResolvedValue(true);

		await user.click(screen.getByLabelText('Select all in DPS'));
		await user.click(screen.getByLabelText('Delete selected Pals'));

		await waitFor(() => expect(sent).toHaveLength(1));
		const [type, payload] = sent[0] as [string, { player_id: string; pal_indexes: number[] }];
		expect(String(type)).toBe('delete_dps_pals');
		expect(payload).toEqual({ player_id: 'player-1', pal_indexes: [0, 1, 2, 3] });
		await waitFor(() => expect(screen.queryByText(/Pals selected/)).toBeNull());
	});

	it('leaves the storage alone when the delete is not confirmed', async () => {
		const user = userEvent.setup();
		selectPlayer(3);
		renderPage();

		await user.click(screen.getByLabelText('Select all in DPS'));
		await user.click(screen.getByLabelText('Delete selected Pals'));

		// The selection drops once the handler finishes, sent or not.
		await waitFor(() => expect(screen.queryByText(/Pals selected/)).toBeNull());
		expect(sent).toHaveLength(0);
	});

	it('adds a pal to the empty slot that was tapped, keyed by its number', async () => {
		const user = userEvent.setup();
		selectPlayer(3);
		renderPage();
		modalState.showModal.mockResolvedValue(['Blaze', 'Nickname']);

		await user.click(badgeButton(5));

		await waitFor(() => expect(sent).toHaveLength(1));
		const [type, payload] = sent[0] as [string, { storage_slot: number }];
		expect(String(type)).toBe('add_dps_pal');
		expect(payload).toMatchObject({
			player_id: 'player-1',
			character_id: 'Blaze',
			nickname: 'Nickname',
			storage_slot: 5
		});
		expect(typeof payload.storage_slot).toBe('number');
	});
});
