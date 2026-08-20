import type { ColumnDef, PageInfo, PageState, SortDirection, SortState } from './table.types';

export function getSortValue<T>(row: T, column: ColumnDef<T>): string | number | null | undefined {
	if (column.sortValue) {
		return column.sortValue(row);
	}
	return (row as Record<string, unknown>)[column.key] as string | number | null | undefined;
}

// Nullish always sorts last, independent of direction.
export function compareValues(
	a: string | number | null | undefined,
	b: string | number | null | undefined,
	direction: SortDirection
): number {
	const aIsNil = a === null || a === undefined;
	const bIsNil = b === null || b === undefined;
	if (aIsNil && bIsNil) return 0;
	if (aIsNil) return 1;
	if (bIsNil) return -1;

	let result: number;
	if (typeof a === 'number' && typeof b === 'number') {
		result = a - b;
	} else {
		result = String(a).localeCompare(String(b), undefined, { numeric: true, sensitivity: 'base' });
	}
	return direction === 'asc' ? result : -result;
}

export function sortRows<T>(rows: T[], columns: ColumnDef<T>[], sort: SortState): T[] {
	if (!sort.key) {
		return rows.slice();
	}
	const column = columns.find((candidate) => candidate.key === sort.key);
	if (!column) {
		return rows.slice();
	}
	return rows
		.map((row, index) => ({ row, index }))
		.sort((left, right) => {
			const comparison = compareValues(
				getSortValue(left.row, column),
				getSortValue(right.row, column),
				sort.direction
			);
			return comparison !== 0 ? comparison : left.index - right.index;
		})
		.map((wrapped) => wrapped.row);
}

export function nextSortState(current: SortState, key: string): SortState {
	if (current.key !== key) {
		return { key, direction: 'asc' };
	}
	return { key, direction: current.direction === 'asc' ? 'desc' : 'asc' };
}

export function computePageInfo(state: PageState, total: number): PageInfo {
	const pageSize = Math.max(1, state.pageSize);
	const totalPages = Math.max(1, Math.ceil(total / pageSize));
	const page = Math.min(Math.max(1, state.page), totalPages);
	const startIndex = total === 0 ? 0 : (page - 1) * pageSize;
	const endIndex = total === 0 ? 0 : Math.min(startIndex + pageSize, total) - 1;
	return {
		page,
		pageSize,
		total,
		totalPages,
		startIndex,
		endIndex,
		hasPrev: page > 1,
		hasNext: page < totalPages
	};
}

export function paginateRows<T>(rows: T[], state: PageState): T[] {
	if (rows.length === 0) {
		return [];
	}
	const info = computePageInfo(state, rows.length);
	return rows.slice(info.startIndex, info.endIndex + 1);
}

export function toggleSelection(selected: Set<string>, key: string): Set<string> {
	const next = new Set(selected);
	if (next.has(key)) {
		next.delete(key);
	} else {
		next.add(key);
	}
	return next;
}

export function setPageSelection(
	selected: Set<string>,
	keys: string[],
	shouldSelect: boolean
): Set<string> {
	const next = new Set(selected);
	for (const key of keys) {
		if (shouldSelect) {
			next.add(key);
		} else {
			next.delete(key);
		}
	}
	return next;
}

// Empty key list is never "all selected".
export function areAllSelected(selected: Set<string>, keys: string[]): boolean {
	return keys.length > 0 && keys.every((key) => selected.has(key));
}
