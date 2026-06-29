import { describe, expect, it } from 'vitest';
import type { ColumnDef, SortState } from './table.types';
import { compareValues, getSortValue, nextSortState, sortRows } from './table.utils';

interface Row {
	id: string;
	name: string;
	level: number | null;
}

const columns: ColumnDef<Row>[] = [
	{ key: 'name', header: 'Name', sortable: true },
	{ key: 'level', header: 'Level', sortable: true }
];

describe('getSortValue', () => {
	it('reads the column key by default', () => {
		expect(getSortValue({ id: 'a', name: 'Zed', level: 3 }, columns[0])).toBe('Zed');
	});

	it('uses sortValue when provided', () => {
		const column: ColumnDef<Row> = { key: 'name', header: 'Name', sortValue: (row) => row.name.length };
		expect(getSortValue({ id: 'a', name: 'Zed', level: 3 }, column)).toBe(3);
	});
});

describe('compareValues', () => {
	it('orders numbers ascending', () => {
		expect(compareValues(1, 2, 'asc')).toBeLessThan(0);
	});

	it('reverses for descending', () => {
		expect(compareValues(1, 2, 'desc')).toBeGreaterThan(0);
	});

	it('sorts nullish last regardless of direction', () => {
		expect(compareValues(null, 5, 'asc')).toBeGreaterThan(0);
		expect(compareValues(null, 5, 'desc')).toBeGreaterThan(0);
	});

	it('compares strings naturally (numeric-aware, case-insensitive)', () => {
		expect(compareValues('item2', 'item10', 'asc')).toBeLessThan(0);
		expect(compareValues('apple', 'Banana', 'asc')).toBeLessThan(0);
	});

	it('sorts undefined last, same as null', () => {
		expect(compareValues(undefined, 5, 'asc')).toBeGreaterThan(0);
		expect(compareValues(5, undefined, 'asc')).toBeLessThan(0);
	});

	it('handles mixed number/string via String() coercion without throwing', () => {
		const result = compareValues(2, 'apple', 'asc');
		expect(typeof result).toBe('number');
	});
});

describe('sortRows', () => {
	const rows: Row[] = [
		{ id: 'a', name: 'Charlie', level: 2 },
		{ id: 'b', name: 'alice', level: 10 },
		{ id: 'c', name: 'Bob', level: null }
	];

	it('returns a new array and does not mutate input', () => {
		const sort: SortState = { key: 'name', direction: 'asc' };
		const result = sortRows(rows, columns, sort);
		expect(result).not.toBe(rows);
		expect(rows[0].id).toBe('a');
		expect(result.map((r) => r.id)).toEqual(['b', 'c', 'a']);
	});

	it('sorts numbers with nullish last', () => {
		const result = sortRows(rows, columns, { key: 'level', direction: 'asc' });
		expect(result.map((r) => r.id)).toEqual(['a', 'b', 'c']);
	});

	it('returns a copy in original order when sort key is null', () => {
		const result = sortRows(rows, columns, { key: null, direction: 'asc' });
		expect(result.map((r) => r.id)).toEqual(['a', 'b', 'c']);
	});

	it('is stable for equal keys', () => {
		const tied: Row[] = [
			{ id: 'x', name: 'same', level: 1 },
			{ id: 'y', name: 'same', level: 1 }
		];
		const result = sortRows(tied, columns, { key: 'name', direction: 'asc' });
		expect(result.map((r) => r.id)).toEqual(['x', 'y']);
	});

	it('returns a copy in original order when sort key is not in the columns array', () => {
		const result = sortRows(rows, columns, { key: 'nonexistent', direction: 'asc' });
		expect(result).not.toBe(rows);
		expect(result.map((r) => r.id)).toEqual(['a', 'b', 'c']);
	});
});

describe('nextSortState', () => {
	it('starts a new column ascending', () => {
		expect(nextSortState({ key: null, direction: 'asc' }, 'level')).toEqual({ key: 'level', direction: 'asc' });
	});

	it('toggles asc -> desc on the same column', () => {
		expect(nextSortState({ key: 'level', direction: 'asc' }, 'level')).toEqual({ key: 'level', direction: 'desc' });
	});

	it('toggles desc -> asc on the same column', () => {
		expect(nextSortState({ key: 'level', direction: 'desc' }, 'level')).toEqual({ key: 'level', direction: 'asc' });
	});
});

import { computePageInfo, paginateRows } from './table.utils';
import type { PageState } from './table.types';

describe('computePageInfo', () => {
	it('computes ranges for a middle page', () => {
		const info = computePageInfo({ page: 2, pageSize: 10 }, 25);
		expect(info).toMatchObject({
			page: 2,
			totalPages: 3,
			startIndex: 10,
			endIndex: 19,
			hasPrev: true,
			hasNext: true
		});
	});

	it('clamps a too-large page down to the last page', () => {
		const info = computePageInfo({ page: 99, pageSize: 10 }, 25);
		expect(info.page).toBe(3);
		expect(info.hasNext).toBe(false);
	});

	it('handles an empty data set', () => {
		const info = computePageInfo({ page: 1, pageSize: 10 }, 0);
		expect(info).toMatchObject({ page: 1, totalPages: 1, startIndex: 0, endIndex: 0, hasPrev: false, hasNext: false });
	});

	it('clamps the last partial page end index to total', () => {
		const info = computePageInfo({ page: 3, pageSize: 10 }, 25);
		expect(info.startIndex).toBe(20);
		expect(info.endIndex).toBe(24);
	});
});

describe('paginateRows', () => {
	const rows = Array.from({ length: 25 }, (_, index) => ({ id: String(index) }));

	it('returns the rows for the requested page', () => {
		const state: PageState = { page: 2, pageSize: 10 };
		expect(paginateRows(rows, state).map((r) => r.id)).toEqual(['10', '11', '12', '13', '14', '15', '16', '17', '18', '19']);
	});

	it('returns the partial final page', () => {
		expect(paginateRows(rows, { page: 3, pageSize: 10 })).toHaveLength(5);
	});

	it('returns an empty array for no rows', () => {
		expect(paginateRows([], { page: 1, pageSize: 10 })).toEqual([]);
	});
});

import { areAllSelected, setPageSelection, toggleSelection } from './table.utils';

describe('toggleSelection', () => {
	it('adds a key when absent and returns a new set', () => {
		const original = new Set<string>(['a']);
		const result = toggleSelection(original, 'b');
		expect(result).not.toBe(original);
		expect([...result].sort()).toEqual(['a', 'b']);
	});

	it('removes a key when present', () => {
		expect([...toggleSelection(new Set(['a', 'b']), 'a')]).toEqual(['b']);
	});
});

describe('setPageSelection', () => {
	it('adds all keys when selecting', () => {
		const result = setPageSelection(new Set(['x']), ['a', 'b'], true);
		expect([...result].sort()).toEqual(['a', 'b', 'x']);
	});

	it('removes all keys when deselecting', () => {
		const result = setPageSelection(new Set(['a', 'b', 'x']), ['a', 'b'], false);
		expect([...result]).toEqual(['x']);
	});
});

describe('areAllSelected', () => {
	it('is true when every key is selected', () => {
		expect(areAllSelected(new Set(['a', 'b', 'c']), ['a', 'b'])).toBe(true);
	});

	it('is false when any key is missing', () => {
		expect(areAllSelected(new Set(['a']), ['a', 'b'])).toBe(false);
	});

	it('is false for an empty key list', () => {
		expect(areAllSelected(new Set(['a']), [])).toBe(false);
	});
});
