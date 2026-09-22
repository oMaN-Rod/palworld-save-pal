// @vitest-environment jsdom
import '$utils/__tests__/fixtures/animatePolyfill';
import { render, screen, within } from '@testing-library/svelte';
import userEvent from '@testing-library/user-event';
import { tick } from 'svelte';
import { beforeEach, describe, expect, it, vi } from 'vitest';

import { installViewportStub } from '$utils/__tests__/fixtures/viewportStub';

const { appState, modalState, toastState } = vi.hoisted(() => ({
	appState: {
		selectedPlayer: undefined as unknown,
		clipboardItem: null as unknown,
		settings: { cheat_mode: false, debug_mode: false }
	},
	modalState: { showModal: vi.fn(), showConfirmModal: vi.fn() },
	toastState: { add: vi.fn() }
}));

vi.mock('$states', () => ({
	getAppState: () => appState,
	getModalState: () => modalState,
	getToastState: () => toastState
}));

vi.mock('$components/modals', () => ({
	TextInputModal: {},
	NumberInputModal: {},
	ItemSelectModal: {}
}));

vi.mock('$components/player', () => ({ PlayerStats: vi.fn(), PlayerHealthBadge: vi.fn() }));
vi.mock('$components/presets', () => ({ PlayerPresets: vi.fn(), StoragePresets: vi.fn() }));

vi.mock('$lib/data', () => ({
	itemsData: { items: {}, getByKey: () => undefined },
	palsData: { getByKey: () => undefined },
	buildingsData: { getByKey: () => undefined },
	expData: {
		expData: { 11: { TotalEXP: 1000, NextEXP: 400 } },
		getExpDataByLevel: vi.fn()
	}
}));

vi.mock('$utils', async (importOriginal) => {
	const actual = await importOriginal<Record<string, unknown>>();
	return {
		...actual,
		assetLoader: { loadImage: (path: string) => path, loadMenuImage: () => '' }
	};
});

const { setViewport } = installViewportStub();
const { default: PlayerPage } = await import('../+page.svelte');

function container(slotCount: number) {
	return {
		id: 'c',
		key: '',
		type: '',
		slot_num: slotCount,
		slots: Array.from({ length: slotCount }, (_, slot_index) => ({
			static_id: 'None',
			slot_index,
			count: 0
		}))
	};
}

function loadPlayer() {
	appState.selectedPlayer = {
		uid: 'p1',
		nickname: 'Tourist',
		level: 10,
		exp: 700,
		last_online_time: 0,
		state: 0,
		status_point_list: { max_hp: 0 },
		common_container: container(42),
		essential_container: container(8),
		food_equip_container: container(3),
		weapon_load_out_container: container(4),
		player_equipment_armor_container: container(10)
	};
}

const SECTIONS = ['stats', 'inventory', 'gear'] as const;

/** Named because the inventory section has a tablist of its own. */
function sectionTabs(): HTMLElement | null {
	return screen.queryByRole('tablist', { name: 'Player sections' });
}

function tabs(): HTMLElement[] {
	const list = sectionTabs();
	return list ? within(list).getAllByRole('tab') : [];
}

function section(id: string): HTMLElement | null {
	return screen.queryByTestId(`player-${id}`);
}

function presentSections(): string[] {
	return SECTIONS.filter((id) => section(id) !== null);
}

beforeEach(() => {
	setViewport(1440);
	appState.selectedPlayer = undefined;
	appState.clipboardItem = null;
	appState.settings.cheat_mode = false;
	modalState.showModal.mockReset();
	modalState.showModal.mockResolvedValue(undefined);
	toastState.add.mockClear();
});

describe('player edit layout', () => {
	it('asks for a player before it renders any section', () => {
		setViewport(390);
		render(PlayerPage, { props: {} });
		expect(screen.getByText('Select a Player to edit')).toBeTruthy();
		expect(presentSections()).toEqual([]);
	});

	it('shows one section at a time behind a tablist on a phone', async () => {
		setViewport(390);
		loadPlayer();
		render(PlayerPage, { props: {} });
		await tick();

		expect(sectionTabs()).toBeTruthy();
		expect(presentSections()).toEqual(['stats']);
	});

	it('shows every section with no tablist on a tablet', async () => {
		setViewport(820);
		loadPlayer();
		render(PlayerPage, { props: {} });
		await tick();

		expect(sectionTabs()).toBeNull();
		expect(presentSections()).toEqual([...SECTIONS]);
	});

	it('shows every section with no tablist on a desktop', async () => {
		setViewport(1440);
		loadPlayer();
		render(PlayerPage, { props: {} });
		await tick();

		expect(sectionTabs()).toBeNull();
		expect(presentSections()).toEqual([...SECTIONS]);
	});

	it('swaps the section the phone shows', async () => {
		const user = userEvent.setup();
		setViewport(390);
		loadPlayer();
		render(PlayerPage, { props: {} });
		await tick();

		await user.click(within(sectionTabs()!).getByRole('tab', { name: 'Gear' }));
		await tick();

		expect(presentSections()).toEqual(['gear']);
	});

	it('restores every section when a phone widens', async () => {
		const user = userEvent.setup();
		setViewport(390);
		loadPlayer();
		render(PlayerPage, { props: {} });
		await tick();

		await user.click(within(sectionTabs()!).getByRole('tab', { name: 'Gear' }));
		await tick();

		setViewport(1440);
		await tick();

		expect(sectionTabs()).toBeNull();
		expect(presentSections()).toEqual([...SECTIONS]);
	});

	it('names each panel from the tab that opens it', async () => {
		setViewport(390);
		loadPlayer();
		render(PlayerPage, { props: {} });
		await tick();

		const tab = within(sectionTabs()!).getByRole('tab', { name: 'Stats' });
		const panel = section('stats')!;

		expect(tab.getAttribute('aria-controls')).toBe(panel.id);
		expect(panel.getAttribute('aria-labelledby')).toBe(tab.id);
		expect(panel.id).toBeTruthy();
		expect(tab.id).toBeTruthy();
	});

	it('points every tab at the panel it opens, shown or not', async () => {
		setViewport(390);
		loadPlayer();
		render(PlayerPage, { props: {} });
		await tick();

		const controls = tabs().map((tab) => tab.getAttribute('aria-controls'));
		expect(controls).toEqual(['player-stats-panel', 'player-inventory-panel', 'player-gear-panel']);
	});
});
