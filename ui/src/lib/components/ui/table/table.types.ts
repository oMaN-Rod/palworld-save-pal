export type SortDirection = 'asc' | 'desc';

export interface ColumnDef<T> {
	/** Also used as the sort key. */
	key: string;
	header: string;
	sortable?: boolean;
	/** Falls back to row[key] when omitted. */
	sortValue?: (row: T) => string | number | null | undefined;
	class?: string;
	align?: 'left' | 'center' | 'right';
}

export interface SortState {
	key: string | null;
	direction: SortDirection;
}

export interface PageState {
	/** 1-based. */
	page: number;
	pageSize: number;
}

export interface PageInfo {
	page: number;
	pageSize: number;
	total: number;
	totalPages: number;
	/** 0-based, unlike `page`. */
	startIndex: number;
	/** 0-based and inclusive. */
	endIndex: number;
	hasPrev: boolean;
	hasNext: boolean;
}
