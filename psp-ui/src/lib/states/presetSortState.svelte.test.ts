import { describe, expect, it } from 'vitest';
import { getConfig, presetSort, setDirection, setMode } from './presetSortState.svelte';

// Select.svelte's mount $effect fires onChange -> setMode -> new presetSort.current
// -> re-render -> new onChange -> effect re-run -> ..., so setters must not
// reassign state when the value is unchanged or the effect loops forever.
describe('presetSortState setters are idempotent', () => {
	it('setMode does not reassign state when the mode is unchanged', () => {
		setMode('pal_preset', 'name');
		const before = presetSort.current;
		setMode('pal_preset', 'name');
		expect(presetSort.current).toBe(before);
	});

	it('setMode reassigns and updates when the mode differs', () => {
		setMode('pal_preset', 'name');
		const before = presetSort.current;
		setMode('pal_preset', 'custom');
		expect(presetSort.current).not.toBe(before);
		expect(getConfig('pal_preset').mode).toBe('custom');
	});

	it('setDirection does not reassign state when the direction is unchanged', () => {
		setDirection('inventory', 'asc');
		const before = presetSort.current;
		setDirection('inventory', 'asc');
		expect(presetSort.current).toBe(before);
	});
});
