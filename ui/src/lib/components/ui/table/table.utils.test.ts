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
