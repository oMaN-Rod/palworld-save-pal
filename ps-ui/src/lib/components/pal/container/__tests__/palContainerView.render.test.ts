// @vitest-environment jsdom
import '$utils/__tests__/fixtures/animatePolyfill';
import { fireEvent, render, screen, within } from '@testing-library/svelte';
import userEvent from '@testing-library/user-event';
import { tick } from 'svelte';
import { beforeEach, describe, expect, it, vi } from 'vitest';

import { installViewportStub } from '$utils/__tests__/fixtures/viewportStub';

// `MediaQuery` reads matchMedia once at construction, so everything reaching `layout` imports after the stub.
const { setViewport } = installViewportStub();
const { VIEW_MODE_STORAGE_PREFIX } = await import('$states/palContainer.svelte');
const { default: PalContainerViewHarness } = await import('./PalContainerViewHarness.svelte');

const DESKTOP_WIDTH = 1440;
const TABLET_WIDTH = 1024;
const PHONE_WIDTH = 390;
const PAGE_SIZE = 4;

interface HarnessPal {
	id: number;
	nickname: string;
	element: string;
}

// Six of each element: three pages at the harness size, two once filtered or searched.
function makePals(): HarnessPal[] {
	return Array.from({ length: 12 }, (_, index) => {
		const id = index + 1;
		const element = index % 2 === 0 ? 'fire' : 'water';
		return { id, nickname: `${element === 'fire' ? 'Blaze' : 'Splash'} ${id}`, element };
	});
}

function renderView(props: Partial<Record<string, unknown>> = {}) {
	return render(PalContainerViewHarness, {
		pals: makePals(),
		storageKey: 'view-test',
		pageSize: PAGE_SIZE,
		...props
	});
}

function renderServerFiltered(props: Partial<Record<string, unknown>> = {}) {
	return render(PalContainerViewHarness, {
		pals: makePals().slice(0, PAGE_SIZE),
		storageKey: 'server-filtered',
		pageSize: PAGE_SIZE,
		serverPaging: {
			page: 1,
			totalCount: PAGE_SIZE,
			pageSize: PAGE_SIZE,
			onPageChange: () => {}
		},
		...props
	});
}

function pageButton(n: number): HTMLElement {
	return screen.getByRole('button', { name: `Page ${n}` });
}

function currentPage(): number | null {
	const active = screen
		.getAllByRole('button')
		.find((button) => button.getAttribute('aria-current') === 'page');
	return active ? Number(active.textContent?.trim()) : null;
}

function pagerLabel(): string {
	return screen.queryByRole('navigation')?.getAttribute('aria-label') ?? 'no pager';
}

function grid(): HTMLElement | null {
	return screen.queryByTestId('pal-container-grid');
}

function filterPanel(): HTMLElement | null {
	return screen.queryByTestId('pal-container-filter-panel');
}

function listRows(): HTMLElement[] {
	return screen.queryAllByTestId(/^columns-/);
}

async function openFilters(): Promise<void> {
	await fireEvent.click(screen.getByRole('button', { name: /^filter/i }));
	await tick();
}

function contextMenuEvent(): MouseEvent {
	return new MouseEvent('contextmenu', { bubbles: true, cancelable: true });
}

function pointerEvent(type: string, x = 10, y = 10): Event {
	const event = new Event(type, { bubbles: true, cancelable: true });
	Object.defineProperty(event, 'clientX', { value: x });
	Object.defineProperty(event, 'clientY', { value: y });
	Object.defineProperty(event, 'pointerId', { value: 1 });
	return event;
}

// Only `setTimeout`: the sheet's transitions run on rAF and must stay real.
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

function makePalActions() {
	const healFn = vi.fn();
	const deleteFn = vi.fn();
	const palActions = () => [
		{ id: 'heal', label: 'Heal', icon: 'tabler:bandage', run: healFn },
		{ id: 'delete', label: 'Delete', icon: 'tabler:trash', run: deleteFn, danger: true }
	];
	return { palActions, healFn, deleteFn };
}

/** Sheet row labels in render order, minus the sheet's own drag handle. */
function sheetRowLabels(dialog: HTMLElement): string[] {
	return within(dialog)
		.getAllByRole('button')
		.map((button) => button.textContent?.trim() ?? '')
		.filter((label) => label !== '');
}

beforeEach(() => {
	localStorage.clear();
	setViewport(DESKTOP_WIDTH);
});

describe('presentation', () => {
	it('renders the grid at desktop when nothing is stored', () => {
		renderView();

		expect(grid()).not.toBeNull();
		expect(listRows()).toHaveLength(0);
		expect(screen.getAllByTestId(/^portrait-/)).toHaveLength(PAGE_SIZE);
	});

	it('renders the list when the stored view mode is list', () => {
		localStorage.setItem(`${VIEW_MODE_STORAGE_PREFIX}stored-list`, 'list');
		renderView({ storageKey: 'stored-list' });

		expect(grid()).toBeNull();
		expect(listRows()).toHaveLength(PAGE_SIZE);
		expect(screen.getByRole('button', { name: 'Blaze 1' })).not.toBeNull();
	});

	it('renders the list by default on a phone with nothing stored', () => {
		setViewport(PHONE_WIDTH);
		renderView({ storageKey: 'phone-default' });

		expect(grid()).toBeNull();
		expect(listRows()).toHaveLength(PAGE_SIZE);
	});

	it('pages the source list rather than rendering all of it', async () => {
		renderView();
		expect(screen.getByTestId('portrait-1')).not.toBeNull();
		expect(screen.queryByTestId('portrait-5')).toBeNull();

		await fireEvent.click(pageButton(2));
		expect(screen.queryByTestId('portrait-1')).toBeNull();
		expect(screen.getByTestId('portrait-5')).not.toBeNull();
	});
});

describe('view mode toggle', () => {
	// The one real unmount: page and selection must survive swapping presentation.
	it('swaps presentation without changing the page number or losing the selection', async () => {
		renderView();

		await fireEvent.click(pageButton(2));
		expect(currentPage()).toBe(2);
		expect(grid()).not.toBeNull();
		expect(screen.getByTestId('portrait-5')).not.toBeNull();

		await fireEvent.click(screen.getByTestId('portrait-5'), { ctrlKey: true });
		await tick();
		expect(screen.getByText(/1 pal selected/i)).not.toBeNull();

		await fireEvent.click(screen.getByRole('button', { name: /list view/i }));
		await tick();

		expect(grid()).toBeNull();
		expect(listRows()).toHaveLength(PAGE_SIZE);
		expect(currentPage()).toBe(2);
		expect(screen.getByTestId('columns-5')).not.toBeNull();
		expect(screen.getByText(/1 pal selected/i)).not.toBeNull();
		expect(screen.getByRole('button', { name: 'Blaze 5, Selected' })).not.toBeNull();

		await fireEvent.click(screen.getByRole('button', { name: /grid view/i }));
		await tick();

		expect(grid()).not.toBeNull();
		expect(currentPage()).toBe(2);
	});
});

describe('detail sheet', () => {
	it('opens on a phone when a pal is selected', async () => {
		setViewport(PHONE_WIDTH);
		renderView({ storageKey: 'phone-detail' });
		expect(screen.queryByRole('dialog')).toBeNull();

		await fireEvent.click(screen.getByRole('button', { name: 'Blaze 1' }));
		await tick();

		expect(screen.getByRole('dialog', { name: 'Blaze 1' })).not.toBeNull();
		expect(screen.getByTestId('detail-body').textContent).toContain('Blaze 1');
	});

	it('opens the editor instead of a sheet at desktop', async () => {
		localStorage.setItem(`${VIEW_MODE_STORAGE_PREFIX}desktop-open`, 'list');
		const onOpenPal = vi.fn();
		renderView({ storageKey: 'desktop-open', onOpenPal });

		await fireEvent.click(screen.getByRole('button', { name: 'Blaze 1' }));
		await tick();

		expect(onOpenPal).toHaveBeenCalledTimes(1);
		expect(onOpenPal.mock.calls[0][0].nickname).toBe('Blaze 1');
		expect(screen.queryByRole('dialog')).toBeNull();
	});
});

describe('filter controls', () => {
	it('renders inline at desktop, with no dialog', async () => {
		renderView();
		expect(screen.queryByTestId('pal-container-filters')).toBeNull();

		await openFilters();

		expect(screen.getByTestId('pal-container-filters')).not.toBeNull();
		expect(screen.queryByRole('dialog')).toBeNull();
	});

	it('renders behind a sheet on a phone', async () => {
		setViewport(PHONE_WIDTH);
		renderView({ storageKey: 'phone-filters' });
		expect(screen.queryByRole('dialog')).toBeNull();

		await openFilters();

		const sheet = screen.getByRole('dialog', { name: /filters/i });
		expect(sheet.contains(screen.getByTestId('pal-container-filters'))).toBe(true);
	});

	// The pager sits outside the panel row so opening the panel does not move it.
	it('opens beside the grid, leaving the pager full width', async () => {
		renderView({ storageKey: 'beside-grid' });

		await openFilters();

		const panel = filterPanel();
		const row = panel?.parentElement;
		expect(row?.className).toContain('flex-row');
		expect((panel?.previousElementSibling as HTMLElement | null)?.contains(grid()!)).toBe(true);
		expect(row?.contains(screen.getByRole('navigation'))).toBe(false);
	});

	it('closes again on a second press of the same button', async () => {
		renderView({ storageKey: 'toggle-filters' });

		await openFilters();
		expect(filterPanel()).not.toBeNull();

		await openFilters();

		expect(filterPanel()).toBeNull();
	});

	it('closes from the panel’s own button, for a toolbar scrolled out of reach', async () => {
		renderView({ storageKey: 'panel-close' });
		await openFilters();

		await fireEvent.click(within(filterPanel()!).getByRole('button', { name: /close/i }));
		await tick();

		expect(filterPanel()).toBeNull();
	});

	// Tablets get the panel, not the sheet: only `layout.phone` decides.
	it('renders the panel on a tablet rather than the sheet', async () => {
		setViewport(TABLET_WIDTH);
		renderView({ storageKey: 'tablet-filters' });

		await openFilters();

		expect(filterPanel()).not.toBeNull();
		expect(screen.queryByRole('dialog')).toBeNull();
	});

	it('reports the panel state on the button that controls it', async () => {
		renderView({ storageKey: 'filter-expanded' });
		const button = screen.getByRole('button', { name: /^filter/i });
		expect(button.getAttribute('aria-expanded')).toBe('false');

		await openFilters();

		expect(button.getAttribute('aria-expanded')).toBe('true');
		expect(document.getElementById(button.getAttribute('aria-controls') ?? '')).toBe(filterPanel());
	});
});

describe('Ruling 48 — contextmenu on a coarse pointer', () => {
	it('suppresses the native menu on a coarse pointer, so a long press opens only one', async () => {
		setViewport(DESKTOP_WIDTH, { coarse: true });
		renderView({ storageKey: 'coarse' });

		const event = contextMenuEvent();
		screen.getByTestId('portrait-1').dispatchEvent(event);

		expect(event.defaultPrevented).toBe(true);
	});

	it('leaves the native menu alone on a fine pointer, so right-click still works', async () => {
		setViewport(DESKTOP_WIDTH, { coarse: false });
		renderView({ storageKey: 'fine' });

		const event = contextMenuEvent();
		screen.getByTestId('portrait-1').dispatchEvent(event);

		expect(event.defaultPrevented).toBe(false);
	});
});

describe('Ruling 56 — a narrowed list cannot strand the page', () => {
	it('resets to page 1 when the filter changes', async () => {
		renderView();

		await fireEvent.click(pageButton(3));
		expect(currentPage()).toBe(3);

		await openFilters();
		await fireEvent.click(screen.getByRole('button', { name: 'Fire' }));
		await tick();

		expect(currentPage()).toBe(1);
		expect(screen.queryByRole('button', { name: 'Page 3' })).toBeNull();
		expect(screen.getByTestId('portrait-1')).not.toBeNull();
	});

	it('resets to page 1 when the search query changes', async () => {
		const user = userEvent.setup();
		renderView();

		await fireEvent.click(pageButton(3));
		expect(currentPage()).toBe(3);

		await openFilters();
		await user.type(screen.getByRole('searchbox'), 'Splash');
		await tick();

		expect(currentPage()).toBe(1);
		expect(screen.queryByRole('button', { name: 'Page 3' })).toBeNull();
		expect(screen.getByTestId('portrait-2')).not.toBeNull();
	});
});

describe('long press opens the action sheet', () => {
	it('opens the action sheet from a grid badge rather than changing the selection', async () => {
		const { palActions } = makePalActions();
		renderView({ storageKey: 'lp-grid', palActions });

		await longPress(screen.getByTestId('portrait-1'));

		expect(screen.queryByText(/1 pal selected/i)).toBeNull();
		expect(screen.getByRole('dialog', { name: 'Blaze 1' })).not.toBeNull();
		expect(screen.queryByTestId('detail-body')).toBeNull();
	});

	it('opens the action sheet from a list row rather than changing the selection', async () => {
		localStorage.setItem(`${VIEW_MODE_STORAGE_PREFIX}lp-list`, 'list');
		const { palActions } = makePalActions();
		renderView({ storageKey: 'lp-list', palActions });

		await longPress(screen.getByRole('button', { name: 'Blaze 1' }));

		expect(screen.queryByText(/1 pal selected/i)).toBeNull();
		expect(screen.getByRole('dialog', { name: 'Blaze 1' })).not.toBeNull();
		expect(screen.queryByTestId('detail-body')).toBeNull();
	});

	it("lists the pal's own actions", async () => {
		const { palActions, healFn } = makePalActions();
		renderView({ storageKey: 'lp-actions', palActions });

		await longPress(screen.getByTestId('portrait-1'));
		const sheet = screen.getByRole('dialog', { name: 'Blaze 1' });

		expect(within(sheet).getByRole('button', { name: 'Heal' })).not.toBeNull();
		expect(within(sheet).getByRole('button', { name: 'Delete' })).not.toBeNull();

		await fireEvent.click(within(sheet).getByRole('button', { name: 'Heal' }));
		expect(healFn).toHaveBeenCalledTimes(1);
	});

	it('sorts danger rows last, with the Select row above them', async () => {
		const { palActions } = makePalActions();
		renderView({ storageKey: 'lp-order', palActions });

		await longPress(screen.getByTestId('portrait-1'));
		const sheet = screen.getByRole('dialog', { name: 'Blaze 1' });

		expect(sheetRowLabels(sheet)).toEqual(['Select', 'Heal', 'Delete']);
	});

	it('toggles selection from the Select row, then closes', async () => {
		const { palActions } = makePalActions();
		renderView({ storageKey: 'lp-select', palActions });

		await longPress(screen.getByTestId('portrait-1'));
		await fireEvent.click(screen.getByRole('button', { name: 'Select' }));
		await tick();

		expect(screen.getByText(/1 pal selected/i)).not.toBeNull();
		expect(screen.queryByRole('dialog')).toBeNull();
	});

	it('reads Deselect for a pal that is already selected', async () => {
		const { palActions } = makePalActions();
		renderView({ storageKey: 'lp-deselect', palActions });

		await fireEvent.click(screen.getByTestId('portrait-1'), { ctrlKey: true });
		await tick();
		expect(screen.getByText(/1 pal selected/i)).not.toBeNull();

		await longPress(screen.getByTestId('portrait-1'));
		const sheet = screen.getByRole('dialog', { name: 'Blaze 1' });

		expect(within(sheet).getByRole('button', { name: 'Deselect' })).not.toBeNull();
		expect(within(sheet).queryByRole('button', { name: 'Select' })).toBeNull();

		await fireEvent.click(within(sheet).getByRole('button', { name: 'Deselect' }));
		await tick();
		expect(screen.queryByText(/1 pal selected/i)).toBeNull();
	});

	it('leaves Ctrl-click selecting directly, with no sheet', async () => {
		const { palActions } = makePalActions();
		renderView({ storageKey: 'lp-ctrl', palActions });

		await fireEvent.click(screen.getByTestId('portrait-1'), { ctrlKey: true });
		await tick();

		expect(screen.getByText(/1 pal selected/i)).not.toBeNull();
		expect(screen.queryByRole('dialog')).toBeNull();
	});
});

describe('state above the presentation', () => {
	// View mode is sticky across breakpoints, so nothing unmounts here.
	it('keeps page and selection when the filter controls move behind a sheet', async () => {
		renderView({ storageKey: 'device-change' });

		await fireEvent.click(pageButton(3));
		expect(currentPage()).toBe(3);

		await fireEvent.click(screen.getByTestId('portrait-9'), { ctrlKey: true });
		await tick();
		expect(screen.getByText(/1 pal selected/i)).not.toBeNull();

		setViewport(PHONE_WIDTH);
		await tick();

		expect(currentPage()).toBe(3);
		expect(screen.getByText(/1 pal selected/i)).not.toBeNull();
	});
});

describe('server-paged mode', () => {
	// `pals` is one server page, not the corpus; client-paged math would silently report one page.
	const SERVER_PALS = 4;
	const SERVER_TOTAL = 12;

	function serverProps(overrides: Record<string, unknown> = {}) {
		const onPageChange = vi.fn();
		const {
			page = 1,
			totalCount = SERVER_TOTAL,
			pageSize = PAGE_SIZE,
			...rest
		} = overrides as {
			page?: number;
			totalCount?: number;
			pageSize?: number;
		};
		return {
			onPageChange,
			props: {
				pals: makePals().slice(0, SERVER_PALS),
				storageKey: 'server',
				...rest,
				serverPaging: { page, totalCount, pageSize, onPageChange }
			}
		};
	}

	function renderServer(overrides: Record<string, unknown> = {}) {
		const { props, onPageChange } = serverProps(overrides);
		return { ...render(PalContainerViewHarness, props), onPageChange, props };
	}

	it('counts pages from totalCount, not from the pals it was handed', () => {
		renderServer();

		expect(pagerLabel()).toBe('Page 1 of 3');
		expect(pageButton(3)).not.toBeNull();
		expect(screen.queryByRole('button', { name: 'Page 4' })).toBeNull();
		expect(currentPage()).toBe(1);
	});

	it('renders every pal it is handed, unsliced', () => {
		renderServer({ pals: makePals().slice(0, 6) });

		// Six pals at a page size of four: a surviving slice would drop two.
		expect(screen.getAllByTestId(/^portrait-/)).toHaveLength(6);
		expect(screen.getByTestId('portrait-5')).not.toBeNull();
		expect(screen.getByTestId('portrait-6')).not.toBeNull();
	});

	it('hands a pager move to the caller and keeps showing the page it was given', async () => {
		const { onPageChange } = renderServer();

		await fireEvent.click(pageButton(2));
		await tick();

		expect(onPageChange).toHaveBeenCalledTimes(1);
		expect(onPageChange).toHaveBeenCalledWith(2);
		expect(currentPage()).toBe(1);
		expect(screen.queryAllByTestId(/^portrait-/)).toHaveLength(SERVER_PALS);
		expect(screen.getByTestId('portrait-1')).not.toBeNull();
	});

	it('reflects a page the caller sets', async () => {
		const { rerender, props, onPageChange } = renderServer();
		expect(currentPage()).toBe(1);

		await rerender({ ...props, serverPaging: { ...props.serverPaging, page: 3 } });
		await tick();

		expect(screen.queryAllByTestId(/^portrait-/)).toHaveLength(SERVER_PALS);
		expect(currentPage()).toBe(3);
		expect(onPageChange).not.toHaveBeenCalled();
	});

	it('follows the caller back to page 1 when it resets on a filter change', async () => {
		const { rerender, props, onPageChange } = renderServer({ page: 3 });
		expect(screen.queryAllByTestId(/^portrait-/)).toHaveLength(SERVER_PALS);
		expect(currentPage()).toBe(3);

		await rerender({
			...props,
			serverPaging: { ...props.serverPaging, page: 1, totalCount: 8 }
		});
		await tick();

		expect(currentPage()).toBe(1);
		expect(pagerLabel()).toBe('Page 1 of 2');
		expect(screen.queryByRole('button', { name: 'Page 3' })).toBeNull();
		expect(onPageChange).not.toHaveBeenCalled();
	});

	it('hides the pager when the server reports a single page', () => {
		renderServer({ totalCount: SERVER_PALS });

		expect(pagerLabel()).toBe('no pager');
		expect(screen.queryAllByTestId(/^portrait-/)).toHaveLength(SERVER_PALS);
	});

	it('divides the server count by the server page size, not by its own', () => {
		// 112 over the server's 56 is two pages; over the view's own 30 it would be four.
		render(PalContainerViewHarness, {
			pals: makePals().slice(0, SERVER_PALS),
			storageKey: 'server-limit',
			pageSize: 30,
			serverPaging: { page: 1, totalCount: 112, pageSize: 56, onPageChange: vi.fn() }
		});

		expect(pagerLabel()).toBe('Page 1 of 2');
	});

	it('disables paging for a size that cannot describe a page', () => {
		renderServer({ pageSize: 0 });

		expect(pagerLabel()).toBe('no pager');
		expect(screen.queryAllByTestId(/^portrait-/)).toHaveLength(SERVER_PALS);
	});

	it('does not filter locally for a consumer that filters server-side', async () => {
		const { onPageChange } = renderServer();

		await openFilters();
		await fireEvent.click(screen.getByRole('button', { name: 'Fire' }));
		await tick();

		expect(screen.queryAllByTestId(/^portrait-/)).toHaveLength(SERVER_PALS);
		expect(screen.getByTestId('portrait-2')).not.toBeNull();
		expect(onPageChange).not.toHaveBeenCalled();
		expect(currentPage()).toBe(1);
	});
});

describe('client-paged mode is unchanged', () => {
	it('still counts pages from the list it is given', () => {
		renderView();

		expect(pagerLabel()).toBe('Page 1 of 3');
		expect(pageButton(3)).not.toBeNull();
		expect(screen.queryByRole('button', { name: 'Page 4' })).toBeNull();
		expect(screen.getAllByTestId(/^portrait-/)).toHaveLength(PAGE_SIZE);
	});

	it('shows no pager for a list that fits one page', () => {
		renderView({ pals: makePals().slice(0, PAGE_SIZE), storageKey: 'client-one-page' });

		expect(pagerLabel()).toBe('no pager');
		expect(screen.queryByRole('button', { name: 'Page 1' })).toBeNull();
		expect(screen.getAllByTestId(/^portrait-/)).toHaveLength(PAGE_SIZE);
	});

	it('still owns its own page, slicing locally with no callback', async () => {
		renderView({ storageKey: 'client-owns-page' });

		await fireEvent.click(pageButton(2));
		await tick();

		expect(currentPage()).toBe(2);
		expect(screen.getByTestId('portrait-5')).not.toBeNull();
		expect(screen.queryByTestId('portrait-1')).toBeNull();
		expect(screen.getAllByTestId(/^portrait-/)).toHaveLength(PAGE_SIZE);
	});

	it('still resets its own page when its own filter narrows the list', async () => {
		renderView({ storageKey: 'client-resets' });

		await fireEvent.click(pageButton(3));
		expect(currentPage()).toBe(3);

		await openFilters();
		await fireEvent.click(screen.getByRole('button', { name: 'Fire' }));
		await tick();

		expect(currentPage()).toBe(1);
		expect(screen.getAllByTestId(/^portrait-/)).toHaveLength(PAGE_SIZE);
		expect(screen.getByTestId('portrait-1')).not.toBeNull();
	});
});

describe('caller-owned selection', () => {
	function renderSelected(ids: number[], overrides: Record<string, unknown> = {}) {
		const onToggle = vi.fn();
		const selection = { ids: new Set(ids), onToggle };
		const props = {
			pals: makePals(),
			storageKey: 'selection',
			pageSize: PAGE_SIZE,
			...overrides,
			selection
		};
		return { ...render(PalContainerViewHarness, props), onToggle, props, selection };
	}

	function selectionCount(): string {
		return screen.queryByText(/pals? selected/i)?.textContent?.trim() ?? 'nothing selected';
	}

	it("counts the caller's selection, not one of its own", () => {
		renderSelected([1, 3]);

		expect(selectionCount()).toBe('2 Pals selected');
	});

	it('marks the pals the caller has selected', () => {
		localStorage.setItem(`${VIEW_MODE_STORAGE_PREFIX}selection-list`, 'list');
		renderSelected([1, 3], { storageKey: 'selection-list' });

		expect(screen.queryAllByRole('button', { name: /, Selected$/ })).toHaveLength(2);
		expect(screen.getByRole('button', { name: 'Blaze 1, Selected' })).not.toBeNull();
		expect(screen.getByRole('button', { name: 'Blaze 3, Selected' })).not.toBeNull();
		expect(screen.getByRole('button', { name: 'Splash 2' })).not.toBeNull();
	});

	it('reports a toggle rather than selecting anything itself', async () => {
		const { onToggle } = renderSelected([]);

		await fireEvent.click(screen.getByTestId('portrait-1'), { ctrlKey: true });
		await tick();

		expect(onToggle).toHaveBeenCalledTimes(1);
		expect(onToggle).toHaveBeenCalledWith(1);
		expect(selectionCount()).toBe('nothing selected');
	});

	it('follows the caller when it changes the selection', async () => {
		const { rerender, props, selection } = renderSelected([]);
		expect(selectionCount()).toBe('nothing selected');

		await rerender({ ...props, selection: { ...selection, ids: new Set([2, 4]) } });
		await tick();

		expect(selectionCount()).toBe('2 Pals selected');
	});

	it('keeps ids for pals that are not on this page', async () => {
		// Ids on pages the view was never handed must survive.
		const { onToggle } = renderSelected([1, 900, 901]);

		expect(selectionCount()).toBe('3 Pals selected');

		await fireEvent.click(screen.getByTestId('portrait-2'), { ctrlKey: true });
		await tick();

		expect(onToggle).toHaveBeenCalledTimes(1);
		expect(onToggle).toHaveBeenCalledWith(2);
		expect(selectionCount()).toBe('3 Pals selected');
	});

	it("labels the action sheet's row from the caller's selection", async () => {
		const { palActions } = makePalActions();
		const { onToggle } = renderSelected([1], { storageKey: 'selection-sheet', palActions });

		await longPress(screen.getByTestId('portrait-1'));
		const sheet = screen.getByRole('dialog', { name: 'Blaze 1' });

		expect(sheetRowLabels(sheet)).toEqual(['Deselect', 'Heal', 'Delete']);

		await fireEvent.click(within(sheet).getByRole('button', { name: 'Deselect' }));
		await tick();

		expect(onToggle).toHaveBeenCalledWith(1);
	});

	it('still owns the selection when the caller passes none', async () => {
		renderView({ storageKey: 'selection-uncontrolled' });

		await fireEvent.click(screen.getByTestId('portrait-1'), { ctrlKey: true });
		await tick();

		expect(selectionCount()).toBe('1 Pal selected');
	});
});

describe('no control that cannot do anything', () => {
	// Search appears only with `matches`; otherwise it would drop the query.
	it('renders no search box for a consumer that filters server-side', async () => {
		renderServerFiltered();

		await openFilters();

		expect(screen.getByTestId('harness-filters')).not.toBeNull();
		expect(screen.queryByRole('searchbox')).toBeNull();
	});

	it('still renders the search box for a consumer it can filter for', async () => {
		renderView({ storageKey: 'searchable' });

		await openFilters();

		expect(screen.getByTestId('harness-filters')).not.toBeNull();
		expect(screen.getByRole('searchbox')).not.toBeNull();
	});

	it('offers no filter control at all when there is nothing to filter', () => {
		renderServerFiltered({ storageKey: 'unfilterable', withFilters: false });

		expect(screen.queryByRole('button', { name: /^filter/i })).toBeNull();
		expect(screen.getAllByTestId(/^portrait-/)).toHaveLength(4);
	});

	it('keeps the filter control for a consumer that brings its own filters', () => {
		renderServerFiltered({ storageKey: 'own-filters' });

		expect(screen.getByRole('button', { name: /^filter/i })).not.toBeNull();
	});
});
