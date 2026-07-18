import { describe, expect, it } from 'vitest';
import { orderPresets, type PresetSortConfig } from './presetSort';

type P = { id: string; name: string };
const list: P[] = [
	{ id: 'b', name: 'Zoe' },
	{ id: 'a', name: 'Abyssal' },
	{ id: 'c', name: 'Mint' }
];

const cfg = (over: Partial<PresetSortConfig>): PresetSortConfig => ({
	mode: 'name',
	direction: 'asc',
	customOrder: [],
	...over
});

describe('orderPresets', () => {
	it('sorts by name ascending', () => {
		expect(orderPresets(list, cfg({ mode: 'name', direction: 'asc' })).map((p) => p.id)).toEqual([
			'a',
			'c',
			'b'
		]);
	});

	it('sorts by name descending', () => {
		expect(orderPresets(list, cfg({ mode: 'name', direction: 'desc' })).map((p) => p.id)).toEqual([
			'b',
			'c',
			'a'
		]);
	});

	it('applies custom order and appends unknown ids name-sorted', () => {
		const config = cfg({ mode: 'custom', customOrder: ['c', 'a'] });
		// 'b' is not in customOrder -> appended after known, name-sorted
		expect(orderPresets(list, config).map((p) => p.id)).toEqual(['c', 'a', 'b']);
	});

	it('ignores custom-order ids that no longer exist', () => {
		const config = cfg({ mode: 'custom', customOrder: ['deleted', 'c', 'a', 'b'] });
		expect(orderPresets(list, config).map((p) => p.id)).toEqual(['c', 'a', 'b']);
	});

	it('custom mode with empty order falls back to name ascending', () => {
		expect(orderPresets(list, cfg({ mode: 'custom', customOrder: [] })).map((p) => p.id)).toEqual([
			'a',
			'c',
			'b'
		]);
	});

	it('does not mutate the input array', () => {
		const input = [...list];
		orderPresets(input, cfg({ mode: 'name', direction: 'desc' }));
		expect(input.map((p) => p.id)).toEqual(['b', 'a', 'c']);
	});
});
