import { describe, expect, it, vi } from 'vitest';

import { availableActions, sheetActions, type ActionDescriptor } from './actionDescriptor';

function action(id: string, available?: () => boolean): ActionDescriptor {
	return { id, label: id, icon: 'tabler:circle', run: vi.fn(), available };
}

function danger(id: string): ActionDescriptor {
	return { id, label: id, icon: 'tabler:trash', run: vi.fn(), danger: true };
}

describe('availableActions', () => {
	it('keeps actions with no availability predicate', () => {
		const actions = [action('sort'), action('fill')];
		expect(availableActions(actions).map((a) => a.id)).toEqual(['sort', 'fill']);
	});

	it('drops actions whose predicate is false', () => {
		const actions = [action('sort'), action('clone', () => false), action('fill')];
		expect(availableActions(actions).map((a) => a.id)).toEqual(['sort', 'fill']);
	});

	it('keeps actions whose predicate is true', () => {
		const actions = [action('clone', () => true)];
		expect(availableActions(actions).map((a) => a.id)).toEqual(['clone']);
	});

	it('returns an empty list when everything is unavailable', () => {
		expect(availableActions([action('a', () => false)])).toEqual([]);
	});
});

describe('sheetActions', () => {
	it('sorts destructive actions last', () => {
		const actions = [danger('delete'), action('sort'), danger('purge'), action('fill')];
		expect(sheetActions(actions).map((a) => a.id)).toEqual(['sort', 'fill', 'delete', 'purge']);
	});

	it('keeps declared order among everything that is not destructive', () => {
		const actions = [action('select'), action('heal'), action('clone')];
		expect(sheetActions(actions).map((a) => a.id)).toEqual(['select', 'heal', 'clone']);
	});

	it('drops unavailable actions before ordering, and does not mutate the input', () => {
		const actions = [danger('delete'), action('clone', () => false), action('sort')];
		expect(sheetActions(actions).map((a) => a.id)).toEqual(['sort', 'delete']);
		expect(actions.map((a) => a.id)).toEqual(['delete', 'clone', 'sort']);
	});
});
