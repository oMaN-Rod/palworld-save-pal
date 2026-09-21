// @vitest-environment jsdom
import '$utils/__tests__/fixtures/animatePolyfill';
import { fireEvent, render, screen, waitFor, within } from '@testing-library/svelte';
import userEvent from '@testing-library/user-event';
import { tick } from 'svelte';
import { beforeEach, describe, expect, it, vi } from 'vitest';

import { installViewportStub } from '$utils/__tests__/fixtures/viewportStub';

/** `DEFAULT_PAGINATION.limit` in `upsState`. */
const PAGE_LIMIT = 56;
/** `PalContainerView`'s own default, which server mode must never use. */
const VIEW_DEFAULT_PAGE_SIZE = 30;

const { appState, palEditor, modalState, sent } = vi.hoisted(() => ({
	appState: {
		saveFile: undefined as unknown,
		selectedPlayer: undefined as unknown,
		settings: { cheat_mode: false, new_pal_prefix: '', clone_prefix: '' },
		addNewUpspal: vi.fn()
	},
	palEditor: { open: vi.fn() },
	modalState: { showModal: vi.fn(), showConfirmModal: vi.fn() },
	sent: [] as unknown[][]
}));

vi.mock('$lib/utils/websocketUtils', () => ({
	isReady: () => true,
	send: (...args: unknown[]) => {
		sent.push(args);
		return Promise.resolve(undefined);
	},
	sendAndWait: (...args: unknown[]) => {
		sent.push(args);
		return Promise.resolve({});
	}
}));

// The real `upsState`: the paging and selection under test live in it.
vi.mock('$states', async () => {
	const { upsState } = await import('$lib/states/upsState.svelte');
	return {
		getUpsState: () => upsState,
		getAppState: () => appState,
		getModalState: () => modalState,
		getToastState: () => ({ add: vi.fn() }),
		getPalEditorState: () => palEditor
	};
});

vi.mock('$components/modals', () => ({
	ImportToUpsModal: {},
	EditTagsModal: {},
	AddToCollectionModal: {},
	ExportPalModal: {},
	NukeUpsConfirmModal: {},
	PalSelectModal: {}
}));

// Stubbed rather than rendered: each belongs to a side panel this suite keeps
// closed, and each pulls in collection/tag/stat fixtures that say nothing
// about the container.
vi.mock('./components/UPSCollectionsPanel.svelte', () => ({ default: {} }));
vi.mock('./components/UPSTagsPanel.svelte', () => ({ default: {} }));
vi.mock('./components/UPSStatsPanel.svelte', () => ({ default: {} }));

vi.mock('$lib/data', () => ({
	palsData: {
		getByKey: (key: string) =>
			key && key !== 'None'
				? { localized_name: key, is_pal: true, pal_deck_index: 1, max_full_stomach: 150 }
				: undefined
	},
	elementsData: { elements: {}, getByKey: () => undefined }
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
const { upsState } = await import('$lib/states/upsState.svelte');
const { default: UpsPage } = await import('./+page.svelte');

type TestUpsPal = {
	id: number;
	instance_id: string;
	character_id: string;
	character_key: string;
	nickname: string;
	level: number;
	pal_data: Record<string, unknown>;
	tags: string[];
	notes: string;
	created_at: string;
	updated_at: string;
	transfer_count: number;
	clone_count: number;
};

function makePal(id: number, name = 'Blaze'): TestUpsPal {
	return {
		id,
		instance_id: `instance-${id}`,
		character_id: name,
		character_key: name,
		nickname: `${name} ${id}`,
		level: 10,
		pal_data: {
			instance_id: `instance-${id}`,
			character_id: name,
			character_key: name,
			name,
			nickname: `${name} ${id}`,
			level: 10,
			hp: 100,
			max_hp: 100,
			storage_slot: id
		},
		tags: [],
		notes: '',
		created_at: '2024-01-01T00:00:00.000Z',
		updated_at: '2024-01-01T00:00:00.000Z',
		transfer_count: 0,
		clone_count: 0
	};
}

/** One server response, through the same entry point the websocket handler uses. */
function loadPage(handed: number, totalCount: number, firstId = 1): void {
	const pals = Array.from({ length: handed }, (_, index) => makePal(firstId + index));
	upsState.setPalsData(pals as never, totalCount, 0, upsState.pagination.limit);
}

function pageButtons(): HTMLElement[] {
	return screen.queryAllByRole('button', { name: /^Page \d+$/ });
}

function pageNumbers(): (string | undefined)[] {
	return pageButtons().map((button) => button.textContent?.trim());
}

function badges(): HTMLElement[] {
	return screen.queryAllByTestId('pal-container-badge');
}

function renderPage() {
	return render(UpsPage, { props: {} });
}

function pointerEvent(type: string, x = 10, y = 10): Event {
	const event = new Event(type, { bubbles: true, cancelable: true });
	Object.defineProperty(event, 'clientX', { value: x });
	Object.defineProperty(event, 'clientY', { value: y });
	Object.defineProperty(event, 'pointerId', { value: 1 });
	return event;
}

// Only `setTimeout` is faked: the sheet's transitions need a real rAF.
async function longPress(element: HTMLElement): Promise<void> {
	vi.useFakeTimers({ toFake: ['setTimeout', 'clearTimeout'] });
	try {
		element.dispatchEvent(pointerEvent('pointerdown'));
		vi.advanceTimersByTime(500);
	} finally {
		vi.useRealTimers();
	}
	await tick();
}

beforeEach(() => {
	setViewport(1440);
	sent.length = 0;
	palEditor.open.mockClear();
	modalState.showModal.mockReset();
	modalState.showModal.mockResolvedValue(undefined);
	modalState.showConfirmModal.mockReset();
	modalState.showConfirmModal.mockResolvedValue(false);
	appState.saveFile = undefined;
	appState.addNewUpspal.mockClear();
	upsState.reset();
	upsState.showCollectionsPanel = false;
	upsState.showTagsPanel = false;
	upsState.showStatsPanel = false;
	localStorage.clear();
});

describe('ups page', () => {
	it('offers the add and import panel while the storage is empty and unfiltered', () => {
		renderPage();

		expect(screen.getByText('No Pals in Storage')).toBeTruthy();
		expect(pageButtons()).toHaveLength(0);
		expect(badges()).toHaveLength(0);
	});

	it('waits on the fetch rather than showing an empty container', () => {
		upsState.loading = true;
		renderPage();

		expect(screen.getByText('Loading Universal Pal Storage')).toBeTruthy();
		expect(pageButtons()).toHaveLength(0);
	});

	it('keeps the container when a filter is what emptied the page', () => {
		upsState.updateSearch('Nothing');
		loadPage(0, 0);
		renderPage();

		expect(screen.queryByText('No Pals in Storage')).toBeNull();
		expect(screen.getByRole('button', { name: 'Filter' })).toBeTruthy();
	});

	it('counts the pages from the server total, not from the page it was handed', () => {
		loadPage(3, 112);
		renderPage();

		expect(badges()).toHaveLength(3);
		expect(pageNumbers()).toEqual(['1', '2']);
		expect(screen.getByRole('navigation', { name: 'Page 1 of 2' })).toBeTruthy();
	});

	it('divides the total by the limit the fetch used, not by the view default', () => {
		loadPage(PAGE_LIMIT, 112);
		renderPage();

		expect(Math.ceil(112 / VIEW_DEFAULT_PAGE_SIZE)).toBe(4);
		expect(pageNumbers()).toEqual(['1', '2']);
		expect(pageButtons()).not.toHaveLength(4);
	});

	it('drops the pager when one page holds everything the server matched', () => {
		loadPage(3, 3);
		renderPage();

		expect(pageButtons()).toHaveLength(0);
	});

	it('moves the page through the state object and re-fetches that page', async () => {
		const user = userEvent.setup();
		loadPage(PAGE_LIMIT, 112);
		renderPage();

		await user.click(screen.getByRole('button', { name: 'Page 2' }));

		expect(upsState.pagination.page).toBe(2);
		await waitFor(() => {
			const fetches = sent.filter(([type]) => String(type) === 'get_ups_pals');
			expect(fetches).toHaveLength(1);
			expect(fetches[0][1]).toMatchObject({ offset: PAGE_LIMIT, limit: PAGE_LIMIT });
		});
	});

	it('renders the page the caller moved to, without being told', async () => {
		const user = userEvent.setup();
		loadPage(PAGE_LIMIT, 112);
		renderPage();

		await user.click(screen.getByRole('button', { name: 'Page 2' }));
		await waitFor(() =>
			expect(screen.getByRole('navigation', { name: 'Page 2 of 2' })).toBeTruthy()
		);

		// Filters reset the page under the view.
		upsState.updateCharacterFilter('Blaze');

		await waitFor(() =>
			expect(screen.getByRole('navigation', { name: 'Page 1 of 2' })).toBeTruthy()
		);
	});

	it('offers the two select-alls with nothing selected, counting different sets', () => {
		loadPage(3, 112);
		renderPage();

		const rail = document.querySelector('#ups-actions');
		expect(rail).toBeTruthy();
		const labels = Array.from(rail?.querySelectorAll('button') ?? []).map((button) =>
			button.getAttribute('aria-label')
		);
		expect(labels).toEqual(['Select all Pals on current page (3)', 'Select all Pals in UPS (112)']);
	});

	it('hands the action surface to the view once something is selected', async () => {
		const user = userEvent.setup();
		loadPage(3, 3);
		renderPage();

		await user.click(screen.getByLabelText('Select all Pals on current page (3)'));

		expect(document.querySelector('#ups-actions')).toBeNull();
		expect(screen.getByText('3 Pals selected')).toBeTruthy();
		expect(screen.getByLabelText('Delete selected Pals')).toBeTruthy();
		expect(screen.getByLabelText('Edit Tags for 3 Pals')).toBeTruthy();
		expect(screen.getByLabelText('Export 3 Pals')).toBeTruthy();
	});

	it('selects only the page it was handed, not the whole storage', async () => {
		const user = userEvent.setup();
		loadPage(3, 112);
		renderPage();

		await user.click(screen.getByLabelText('Select all Pals on current page (3)'));

		expect(screen.getByText('3 Pals selected')).toBeTruthy();
		expect(upsState.selectedPals.size).toBe(3);
	});

	it('asks the server for the whole filtered set when the other row is used', async () => {
		const user = userEvent.setup();
		upsState.updateSearch('Blaze');
		loadPage(3, 112);
		renderPage();

		await user.click(screen.getByLabelText('Select all filtered Pals (112)'));

		await waitFor(() =>
			expect(sent.some(([type]) => String(type) === 'get_ups_all_filtered_ids')).toBe(true)
		);
	});

	it('toggles one pal with a Ctrl-click on its grid badge', async () => {
		const user = userEvent.setup();
		loadPage(3, 3);
		renderPage();

		await user.keyboard('{Control>}');
		await user.click(badges()[0]);
		await user.keyboard('{/Control}');

		expect(screen.getByText('1 Pal selected')).toBeTruthy();
		expect(palEditor.open).not.toHaveBeenCalled();
	});

	it('selects a grid pal from a press with no modifier key held', async () => {
		loadPage(3, 3);
		renderPage();

		await longPress(badges()[0]);
		const sheet = screen.getByRole('dialog', { name: 'Blaze 1' });
		await fireEvent.click(within(sheet).getByRole('button', { name: 'Select' }));

		await waitFor(() => expect(screen.getByText('1 Pal selected')).toBeTruthy());
		expect(upsState.selectedPals.has(1)).toBe(true);
	});

	it('selects a list pal from a press with no modifier key held', async () => {
		const user = userEvent.setup();
		loadPage(3, 3);
		renderPage();
		await user.click(screen.getByLabelText('List View'));

		const rows = screen.queryAllByRole('button', { name: 'Blaze 1' });
		expect(rows).toHaveLength(1);
		await longPress(rows[0]);
		const sheet = screen.getByRole('dialog', { name: 'Blaze 1' });
		await fireEvent.click(within(sheet).getByRole('button', { name: 'Select' }));

		await waitFor(() => expect(screen.getByText('1 Pal selected')).toBeTruthy());
		expect(palEditor.open).not.toHaveBeenCalled();
	});

	it('keeps the selection when the page changes underneath it', async () => {
		const user = userEvent.setup();
		loadPage(3, 112);
		renderPage();

		await user.keyboard('{Control>}');
		await user.click(badges()[0]);
		await user.keyboard('{/Control}');
		expect(screen.getByText('1 Pal selected')).toBeTruthy();

		loadPage(3, 112, 57);

		await waitFor(() => expect(screen.getByText('1 Pal selected')).toBeTruthy());
		expect(upsState.selectedPals.has(1)).toBe(true);
	});

	it('counts a selection holding ids that are not on this page', async () => {
		loadPage(3, 112);
		renderPage();

		// As `selectAllFilteredPals` would: ids from other pages.
		for (const id of [1, 80, 81, 82]) upsState.togglePalSelection(id);

		await waitFor(() => expect(screen.getByText('4 Pals selected')).toBeTruthy());
	});

	it('empties the count bar when a bulk delete clears the selection in place', async () => {
		const user = userEvent.setup();
		loadPage(3, 3);
		renderPage();
		modalState.showConfirmModal.mockResolvedValue(true);

		await user.click(screen.getByLabelText('Select all Pals on current page (3)'));
		expect(screen.getByText('3 Pals selected')).toBeTruthy();

		await user.click(screen.getByLabelText('Delete selected Pals'));

		// `deleteSelectedPals` mutates the set in place with `clear()`.
		await waitFor(() => expect(screen.queryByText(/Pals? selected/)).toBeNull());
		const deletes = sent.filter(([type]) => String(type) === 'delete_ups_pals');
		expect(deletes).toHaveLength(1);
		expect(deletes[0][1]).toEqual({ pal_ids: [1, 2, 3] });
	});

	it('clears the selection from its own action', async () => {
		const user = userEvent.setup();
		loadPage(3, 3);
		renderPage();

		await user.click(screen.getByLabelText('Select all Pals on current page (3)'));
		await user.click(screen.getByLabelText('Clear selected Pals'));

		await waitFor(() => expect(screen.queryByText(/Pals? selected/)).toBeNull());
		expect(upsState.selectedPals.size).toBe(0);
	});

	it('puts the page search behind the container filter button', async () => {
		const user = userEvent.setup();
		loadPage(3, 3);
		renderPage();

		expect(document.querySelector('#ups-filters')).toBeNull();
		await user.click(screen.getByRole('button', { name: 'Filter' }));
		expect(document.querySelector('#ups-filters')).toBeTruthy();
	});

	it('renders no search box of its own, having passed no matches', async () => {
		const user = userEvent.setup();
		loadPage(3, 3);
		renderPage();

		await user.click(screen.getByRole('button', { name: 'Filter' }));
		expect(screen.queryByPlaceholderText(/Search by name/i)).toBeNull();
		expect(screen.queryAllByPlaceholderText('Search Pals')).toHaveLength(1);
	});

	it('opens a pal in the editor with the storage markers the editor needs', async () => {
		const user = userEvent.setup();
		loadPage(3, 3);
		renderPage();

		await user.click(screen.getByLabelText('List View'));
		const rows = screen.queryAllByRole('button', { name: 'Blaze 2' });
		expect(rows).toHaveLength(1);
		await user.click(rows[0]);

		expect(palEditor.open).toHaveBeenCalledTimes(1);
		expect(palEditor.open.mock.calls[0][0]).toMatchObject({
			__ups_source: true,
			__ups_id: 2,
			character_id: 'Blaze'
		});
	});

	it('remembers the view mode under the ups key', async () => {
		const user = userEvent.setup();
		loadPage(3, 3);
		renderPage();

		await user.click(screen.getByLabelText('List View'));

		expect(localStorage.getItem('ps-pal-view-ups')).toBe('list');
	});
});
