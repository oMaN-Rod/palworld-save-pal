import { describe, expect, it, vi } from 'vitest';

import { availableActions, type ActionDescriptor } from './actionDescriptor';

function action(id: string, available?: () => boolean): ActionDescriptor {
	return { id, label: id, icon: 'tabler:circle', run: vi.fn(), available };
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
