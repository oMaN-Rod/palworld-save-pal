import { layout } from '$lib/utils/layout.svelte';
import { SvelteSet } from 'svelte/reactivity';

export const VIEW_MODE_STORAGE_PREFIX = 'ps-pal-view-';

export type PalContainerViewMode = 'grid' | 'list';

interface PalContainerStateOptionsBase<TPal, TId extends string | number> {
	key: string;
	idOf: (pal: TPal) => TId;
}

export type PalContainerStateOptions<TPal, TId extends string | number> =
	| (PalContainerStateOptionsBase<TPal, TId> & { pageSize?: undefined; totalOf?: never })
	| (PalContainerStateOptionsBase<TPal, TId> & { pageSize: number; totalOf: () => number });

/** Server-side paging: the caller owns the page and total. */
export interface PalContainerServerPaging {
	/** 1-based. */
	page: number;
	/** Every matching pal on the server, not just those rendered. */
	totalCount: number;
	/** Required so the page count cannot fall back to the view's default `pageSize`. */
	pageSize: number;
	onPageChange: (page: number) => void;
}

/** Reports toggles, not a new set: in server mode the set spans pages the view never sees. */
export interface PalContainerSelection<TId extends string | number> {
	/** May hold ids for pals the view has not been handed. */
	ids: ReadonlySet<TId>;
	onToggle: (id: TId) => void;
}

/** A non-positive or absent `pageSize` disables paging: one page. */
export function pageCountOf(total: number, pageSize: number | undefined): number {
	if (pageSize === undefined || pageSize <= 0) return 1;
	return Math.max(1, Math.ceil(total / pageSize));
}

function readStoredViewMode(storageKey: string): PalContainerViewMode | null {
	try {
		const stored = localStorage.getItem(storageKey);
		return stored === 'grid' || stored === 'list' ? stored : null;
	} catch {
		return null;
	}
}

function writeStoredViewMode(storageKey: string, mode: PalContainerViewMode): void {
	try {
		localStorage.setItem(storageKey, mode);
	} catch {
		// private mode / sandboxed iframe: preference just doesn't persist
	}
}

/** Omitting `pageSize` disables paging. */
export class PalContainerState<TPal, TId extends string | number> {
	readonly idOf: (pal: TPal) => TId;

	readonly #storageKey: string;
	readonly #pageSize?: number;
	readonly #totalOf?: () => number;

	#viewMode = $state<PalContainerViewMode>('grid');
	#page = $state(1);
	#selected = new SvelteSet<TId>();

	constructor(options: PalContainerStateOptions<TPal, TId>) {
		this.idOf = options.idOf;
		this.#pageSize = options.pageSize;
		this.#totalOf = options.pageSize !== undefined ? options.totalOf : undefined;
		this.#storageKey = `${VIEW_MODE_STORAGE_PREFIX}${options.key}`;

		const deviceDefault: PalContainerViewMode = layout.phone ? 'list' : 'grid';
		this.#viewMode = readStoredViewMode(this.#storageKey) ?? deviceDefault;
	}

	get viewMode(): PalContainerViewMode {
		return this.#viewMode;
	}

	set viewMode(mode: PalContainerViewMode) {
		this.setViewMode(mode);
	}

	setViewMode(mode: PalContainerViewMode): void {
		this.#viewMode = mode;
		writeStoredViewMode(this.#storageKey, mode);
	}

	/** Clamped on read too, since the total can shrink after `setPage`. */
	get page(): number {
		return Math.min(this.#page, this.pageCount());
	}

	set page(n: number) {
		this.setPage(n);
	}

	/** `undefined` when paging is disabled. */
	get pageSize(): number | undefined {
		if (this.#pageSize === undefined || this.#pageSize <= 0 || !this.#totalOf) return undefined;
		return this.#pageSize;
	}

	pageCount(): number {
		if (!this.#totalOf) return 1;
		return pageCountOf(this.#totalOf(), this.pageSize);
	}

	setPage(n: number): void {
		this.#page = Math.min(Math.max(1, Math.trunc(n)), this.pageCount());
	}

	nextPage(): void {
		this.setPage(this.page + 1);
	}

	prevPage(): void {
		this.setPage(this.page - 1);
	}

	get selection(): TId[] {
		return Array.from(this.#selected);
	}

	toggleSelected(id: TId): void {
		if (this.#selected.has(id)) {
			this.#selected.delete(id);
		} else {
			this.#selected.add(id);
		}
	}

	isSelected(id: TId): boolean {
		return this.#selected.has(id);
	}

	clearSelection(): void {
		this.#selected.clear();
	}
}

export function createPalContainerState<TPal, TId extends string | number>(
	options: PalContainerStateOptions<TPal, TId>
): PalContainerState<TPal, TId> {
	return new PalContainerState(options);
}
