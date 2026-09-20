// @vitest-environment jsdom
import { render, screen } from '@testing-library/svelte';
import { tick } from 'svelte';
import { describe, expect, it } from 'vitest';

import ContextBar, { type ContextBarItem } from '../ContextBar.svelte';

const items: ContextBarItem[] = [
	{ id: 'player', label: 'Loadout', href: '/edit/player' },
	{ id: 'technologies', label: 'Technology', href: '/edit/technologies' },
	{ id: 'dps', label: 'DPS', href: '/edit/dps', available: false }
];

describe('ContextBar', () => {
	it('renders a link per available item', async () => {
		render(ContextBar, { items, activeId: 'player' });
		await tick();

		expect(screen.getByRole('link', { name: 'Loadout' })).not.toBeNull();
		expect(screen.getByRole('link', { name: 'Technology' })).not.toBeNull();
	});

	it('omits unavailable items', async () => {
		render(ContextBar, { items, activeId: 'player' });
		await tick();

		expect(screen.queryByRole('link', { name: 'DPS' })).toBeNull();
	});

	it('marks the active item for assistive technology', async () => {
		render(ContextBar, { items, activeId: 'player' });
		await tick();

		expect(screen.getByRole('link', { name: 'Loadout' }).getAttribute('aria-current')).toBe('page');
		expect(
			screen.getByRole('link', { name: 'Technology' }).getAttribute('aria-current')
		).toBeNull();
	});

	it('is a labelled navigation landmark', async () => {
		render(ContextBar, { items, activeId: 'player' });
		await tick();

		expect(screen.getByRole('navigation')).not.toBeNull();
	});
});
