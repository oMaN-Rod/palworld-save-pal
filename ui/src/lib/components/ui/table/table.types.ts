export type SortDirection = 'asc' | 'desc';

export interface ColumnDef<T> {
	/** Stable identifier for the column; also the default sort key. */
	key: string;
	/** Header label text. */
	header: string;
	/** Whether the header is clickable to sort. Defaults to false. */
	sortable?: boolean;
	/** Extracts the comparable value for sorting. Falls back to row[key]. */
	sortValue?: (row: T) => string | number | null | undefined;
	/** Extra classes applied to this column's cells and header. */
	class?: string;
	/** Cell text alignment. Defaults to 'left'. */
	align?: 'left' | 'center' | 'right';
}

export interface SortState {
	key: string | null;
	direction: SortDirection;
}

export interface PageState {
	/** 1-based page number. */
	page: number;
	pageSize: number;
}

export interface PageInfo {
	page: number;
	pageSize: number;
	total: number;
	totalPages: number;
	/** 0-based index of the first row on the page within the full set. */
	startIndex: number;
	/** 0-based inclusive index of the last row on the page. */
	endIndex: number;
	hasPrev: boolean;
	hasNext: boolean;
}
