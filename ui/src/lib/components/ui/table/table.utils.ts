import type { ColumnDef, SortDirection, SortState } from './table.types';

/** Resolve the comparable value for a row in a given column. */
export function getSortValue<T>(row: T, column: ColumnDef<T>): string | number | null | undefined {
	if (column.sortValue) {
		return column.sortValue(row);
	}
	return (row as Record<string, unknown>)[column.key] as string | number | null | undefined;
}

/** Compare two primitive values. Nullish always sorts last, independent of direction. */
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

/** Sort rows by the active sort state. Pure, stable, never mutates the input. */
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

/** Compute the sort state after a header click: new column -> asc; same column -> toggle. */
export function nextSortState(current: SortState, key: string): SortState {
	if (current.key !== key) {
		return { key, direction: 'asc' };
	}
	return { key, direction: current.direction === 'asc' ? 'desc' : 'asc' };
}
