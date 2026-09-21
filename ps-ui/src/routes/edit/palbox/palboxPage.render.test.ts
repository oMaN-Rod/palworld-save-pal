// @vitest-environment jsdom
import '$utils/__tests__/fixtures/animatePolyfill';
import { render, screen } from '@testing-library/svelte';
import userEvent from '@testing-library/user-event';
import { beforeEach, describe, expect, it, vi } from 'vitest';

import { installViewportStub } from '$utils/__tests__/fixtures/viewportStub';

const PALS_PER_PAGE = 30;
const TOTAL_SLOTS = 960;

const { appState, palEditor, sent } = vi.hoisted(() => ({
	appState: {
		selectedPlayer: undefined as unknown,
		selectedPal: undefined as unknown,
		settings: { new_pal_prefix: '', clone_prefix: '' },
		saveState: vi.fn()
	},
	palEditor: { open: vi.fn() },
	sent: [] as unknown[][]
}));

vi.mock('$states', () => ({
	getAppState: () => appState,
	getModalState: () => ({ showModal: vi.fn(), showConfirmModal: vi.fn() }),
	getToastState: () => ({ add: vi.fn() }),
	getUpsState: () => ({ cloneToUps: vi.fn() }),
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
const { default: PalboxPage } = await import('./+page.svelte');

type TestPal = {
	instance_id: string;
	character_id: string;
	character_key: string;
	name: string;
	nickname?: string;
	level: number;
	hp: number;
	max_hp: number;
	storage_id: string;
	storage_slot: number;
};

const PAL_BOX_ID = 'box';
const OTOMO_ID = 'otomo';

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
		storage_id: PAL_BOX_ID,
		storage_slot: index
	};
}

function selectPlayer(count: number): void {
	const pals: Record<string, TestPal> = {};
	for (let index = 0; index < count; index++) {
		const pal = makePal(index, 'Blaze');
		pals[pal.instance_id] = pal;
	}
	const odd = makePal(count, 'Splash');
	pals[odd.instance_id] = odd;

	appState.selectedPlayer = {
		uid: 'player-1',
		level: 50,
		pal_box_id: PAL_BOX_ID,
		otomo_container_id: OTOMO_ID,
		pals
	};
}

function pageButtons(): HTMLElement[] {
	return screen.queryAllByRole('button', { name: /^Page \d+$/ });
}

function renderPage() {
	return render(PalboxPage, { props: {} });
}

beforeEach(() => {
	setViewport(1440);
	sent.length = 0;
	palEditor.open.mockClear();
	appState.selectedPlayer = undefined;
	localStorage.clear();
});

describe('palbox page', () => {
	it('asks for a player before it renders a container', () => {
		renderPage();
		expect(screen.getByRole('heading', { name: /Select a Player/i })).toBeTruthy();
		expect(pageButtons()).toHaveLength(0);
	});

	it('pages the whole box when nothing narrows it', () => {
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

		const rail = document.querySelector('#palbox-actions');
		expect(rail).toBeTruthy();
		const labels = Array.from(rail?.querySelectorAll('button') ?? []).map((button) =>
			button.getAttribute('aria-label')
		);
		expect(labels).toEqual([
			'Add a new Pal to your Palbox',
			'Add all Pals to Palbox',
			'Select all in Palbox',
			'Select all in Palbox + party',
			'Heal all in Palbox'
		]);
	});

	it('hands the action surface to the view once something is selected', async () => {
		const user = userEvent.setup();
		selectPlayer(3);
		renderPage();

		await user.click(screen.getByLabelText('Select all in Palbox'));

		expect(document.querySelector('#palbox-actions')).toBeNull();
		expect(screen.getByText('4 Pals selected')).toBeTruthy();
		expect(screen.getByLabelText('Delete selected Pals')).toBeTruthy();
	});

	it('selects every matching pal, not every slot in the box', async () => {
		const user = userEvent.setup();
		selectPlayer(45);
		renderPage();

		await user.click(screen.getByLabelText('Select all in Palbox'));

		expect(screen.getByText('46 Pals selected')).toBeTruthy();
	});

	it('clears a complete selection when select-all runs again', async () => {
		const user = userEvent.setup();
		selectPlayer(3);
		renderPage();

		await user.click(screen.getByLabelText('Select all in Palbox'));
		await user.click(screen.getByLabelText('Select all in Palbox'));

		expect(screen.queryByText(/Pals selected/)).toBeNull();
		expect(document.querySelector('#palbox-actions')).toBeTruthy();
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

		await user.click(screen.getByLabelText('Select all in Palbox'));
		await user.click(screen.getByLabelText('Clear selected Pals'));

		expect(screen.queryByText(/Pals selected/)).toBeNull();
	});

	it('heals the selected pals and drops the selection', async () => {
		const user = userEvent.setup();
		selectPlayer(3);
		renderPage();

		await user.click(screen.getByLabelText('Select all in Palbox'));
		await user.click(screen.getByLabelText('Heal selected Pals'));

		expect(sent.some(([type]) => String(type).includes('heal'))).toBe(true);
		expect(screen.queryByText(/Pals selected/)).toBeNull();
	});
});
