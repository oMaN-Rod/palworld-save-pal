// @vitest-environment jsdom
import { installViewportStub } from '$lib/utils/__tests__/fixtures/viewportStub';
import { beforeEach, describe, expect, it } from 'vitest';

const { setViewport } = installViewportStub();

const { createPalContainerState, pageCountOf, VIEW_MODE_STORAGE_PREFIX } = await import(
	'./palContainer.svelte'
);

const PHONE_WIDTH = 500;
const DESKTOP_WIDTH = 1440;

beforeEach(() => {
	localStorage.clear();
	setViewport(DESKTOP_WIDTH);
});

describe('view mode default', () => {
	it('defaults to grid at desktop when nothing is stored', () => {
		setViewport(DESKTOP_WIDTH);
		const state = createPalContainerState({ key: 'default-desktop', idOf: (id: string) => id });
		expect(state.viewMode).toBe('grid');
	});

	it('defaults to list on phone when nothing is stored', () => {
		setViewport(PHONE_WIDTH);
		const state = createPalContainerState({ key: 'default-phone', idOf: (id: string) => id });
		expect(state.viewMode).toBe('list');
	});

	it('a stored value beats the device default at every width', () => {
		localStorage.setItem(`${VIEW_MODE_STORAGE_PREFIX}stored-key`, 'list');
		setViewport(DESKTOP_WIDTH);
		expect(createPalContainerState({ key: 'stored-key', idOf: (id: string) => id }).viewMode).toBe(
			'list'
		);

		localStorage.setItem(`${VIEW_MODE_STORAGE_PREFIX}stored-key-2`, 'grid');
		setViewport(PHONE_WIDTH);
		expect(
			createPalContainerState({ key: 'stored-key-2', idOf: (id: string) => id }).viewMode
		).toBe('grid');
	});

	it('survives an orientation change once set', () => {
		setViewport(DESKTOP_WIDTH);
		const state = createPalContainerState({ key: 'orientation', idOf: (id: string) => id });
		state.setViewMode('list');

		setViewport(PHONE_WIDTH);
		expect(state.viewMode).toBe('list');
	});
});

describe('view mode persistence', () => {
	it('setViewMode writes through to localStorage', () => {
		const state = createPalContainerState({ key: 'write-through', idOf: (id: string) => id });
		state.setViewMode('list');
		expect(localStorage.getItem(`${VIEW_MODE_STORAGE_PREFIX}write-through`)).toBe('list');

		state.viewMode = 'grid';
		expect(localStorage.getItem(`${VIEW_MODE_STORAGE_PREFIX}write-through`)).toBe('grid');
	});

	it('degrades to the device default without throwing when localStorage throws', () => {
		const originalGetItem = localStorage.getItem.bind(localStorage);
		const originalSetItem = localStorage.setItem.bind(localStorage);
		localStorage.getItem = () => {
			throw new Error('private mode');
		};
		localStorage.setItem = () => {
			throw new Error('private mode');
		};

		try {
			setViewport(PHONE_WIDTH);
			let state: ReturnType<typeof createPalContainerState<string, string>> | undefined;
			expect(() => {
				state = createPalContainerState({ key: 'blocked', idOf: (id: string) => id });
			}).not.toThrow();
			expect(state?.viewMode).toBe('list');

			expect(() => state?.setViewMode('grid')).not.toThrow();
			expect(state?.viewMode).toBe('grid');
		} finally {
			localStorage.getItem = originalGetItem;
			localStorage.setItem = originalSetItem;
		}
	});
});

describe('paging', () => {
	it('pageCount rounds up and returns 1 for an empty container', () => {
		let total = 45;
		const state = createPalContainerState({
			key: 'paging-count',
			pageSize: 30,
			idOf: (id: string) => id,
			totalOf: () => total
		});
		expect(state.pageCount()).toBe(2);

		total = 0;
		expect(state.pageCount()).toBe(1);
	});

	it('nextPage/prevPage clamp at both ends', () => {
		const state = createPalContainerState({
			key: 'paging-clamp',
			pageSize: 10,
			idOf: (id: string) => id,
			totalOf: () => 25
		});
		expect(state.page).toBe(1);

		state.prevPage();
		expect(state.page).toBe(1);

		state.nextPage();
		state.nextPage();
		state.nextPage();
		expect(state.page).toBe(3);
	});

	it('nextPage cannot exceed pageCount when pageSize and totalOf are supplied', () => {
		const state = createPalContainerState({
			key: 'paging-upper-bound',
			pageSize: 10,
			idOf: (id: string) => id,
			totalOf: () => 25
		});

		for (let i = 0; i < 10; i++) state.nextPage();

		expect(state.page).toBe(state.pageCount());
		expect(state.page).toBe(3);
	});

	it('clamps the page on read when the total shrinks underneath it', () => {
		let total = 320;
		const state = createPalContainerState({
			key: 'paging-shrink',
			pageSize: 10,
			idOf: (id: string) => id,
			totalOf: () => total
		});

		state.setPage(12);
		expect(state.page).toBe(12);

		total = 20;
		expect(state.pageCount()).toBe(2);
		expect(state.page).toBe(2);

		state.prevPage();
		expect(state.page).toBe(1);
	});

	it('is cleanly disabled when pageSize is omitted — nextPage() cannot climb', () => {
		const state = createPalContainerState({ key: 'paging-disabled', idOf: (id: string) => id });
		expect(state.pageCount()).toBe(1);

		state.nextPage();
		state.nextPage();
		state.nextPage();
		expect(state.page).toBe(1);
	});

	it('reports pageSize and pageCount consistently for a size that cannot describe a page', () => {
		const state = createPalContainerState({
			key: 'paging-zero',
			pageSize: 0,
			idOf: (id: string) => id,
			totalOf: () => 320
		});

		expect(state.pageSize).toBeUndefined();
		expect(state.pageCount()).toBe(1);
	});
});

describe('selection', () => {
	it('toggleSelected adds then removes', () => {
		const state = createPalContainerState({ key: 'selection-toggle', idOf: (id: string) => id });
		expect(state.isSelected('pal-1')).toBe(false);

		state.toggleSelected('pal-1');
		expect(state.isSelected('pal-1')).toBe(true);
		expect(state.selection).toEqual(['pal-1']);

		state.toggleSelected('pal-1');
		expect(state.isSelected('pal-1')).toBe(false);
		expect(state.selection).toEqual([]);
	});

	it('clearSelection empties', () => {
		const state = createPalContainerState({ key: 'selection-clear', idOf: (id: string) => id });
		state.toggleSelected('pal-1');
		state.toggleSelected('pal-2');
		expect(state.selection.length).toBe(2);

		state.clearSelection();
		expect(state.selection).toEqual([]);
	});

	it('holds numeric ids for a UPS-style consumer without coercion', () => {
		const state = createPalContainerState({ key: 'selection-numeric', idOf: (id: number) => id });
		state.toggleSelected(42);
		expect(state.isSelected(42)).toBe(true);
		expect(state.selection).toEqual([42]);
	});
});

describe('pageCountOf', () => {
	it('rounds up and floors at one page', () => {
		expect(pageCountOf(112, 56)).toBe(2);
		expect(pageCountOf(113, 56)).toBe(3);
		expect(pageCountOf(1, 56)).toBe(1);
		expect(pageCountOf(0, 56)).toBe(1);
	});

	it('reports one page for a size that cannot describe a page', () => {
		expect(pageCountOf(320, 0)).toBe(1);
		expect(pageCountOf(320, -1)).toBe(1);
		expect(pageCountOf(320, undefined)).toBe(1);
	});

	it('agrees with the container built on the same numbers', () => {
		for (const pageSize of [56, 30, 1, 0]) {
			const state = createPalContainerState({
				key: `agree-${pageSize}`,
				pageSize,
				idOf: (id: string) => id,
				totalOf: () => 113
			});
			expect(state.pageCount()).toBe(pageCountOf(113, state.pageSize));
			expect(state.pageCount()).toBe(pageCountOf(113, pageSize));
		}
	});
});
