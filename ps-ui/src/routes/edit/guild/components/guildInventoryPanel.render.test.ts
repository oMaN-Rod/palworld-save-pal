// @vitest-environment jsdom
import '$utils/__tests__/fixtures/animatePolyfill';
import { render, screen } from '@testing-library/svelte';
import userEvent from '@testing-library/user-event';
import { describe, expect, it, vi } from 'vitest';

import type { GuildInventoryItem } from '../guildStorage';

vi.mock('$lib/data', () => ({
	itemsData: {
		getByKey: (key: string) =>
			key === 'Wood'
				? {
						info: { localized_name: 'Wood', description: 'A log.' },
						details: { icon: 'wood', rarity: 0 }
					}
				: undefined
	},
	buildingsData: {
		getByKey: (key: string) =>
			key === 'Chest' ? { localized_name: 'Wooden Chest', icon: 'chest' } : undefined
	}
}));

vi.mock('$utils', async (importOriginal) => {
	const actual = await importOriginal<Record<string, unknown>>();
	return {
		...actual,
		assetLoader: { loadImage: () => 'data:image/gif;base64,', loadMenuImage: () => '' }
	};
});

const { default: GuildInventoryPanel } = await import('./GuildInventoryPanel.svelte');

const items: GuildInventoryItem[] = [
	{ static_id: 'Wood', containers: { Chest: 120, PalEgg_Common: 3 }, total_count: 123 },
	{ static_id: 'Mystery', containers: {}, total_count: 7 }
];

function renderPanel(props: Record<string, unknown> = {}) {
	const handlers = { onSelect: vi.fn(), onReset: vi.fn() };
	render(GuildInventoryPanel, { props: { items, searchQuery: '', ...handlers, ...props } });
	return handlers;
}

describe('GuildInventoryPanel', () => {
	it('names an item the game knows and totals it', () => {
		renderPanel();
		expect(screen.getByText('Wood')).toBeTruthy();
		expect(screen.getByText('123')).toBeTruthy();
	});

	it('falls back to the raw id for an item it cannot name', () => {
		renderPanel();
		expect(screen.getByText('Mystery')).toBeTruthy();
		expect(screen.getByText('7')).toBeTruthy();
	});

	it('reports the item a row stands for', async () => {
		const user = userEvent.setup();
		const { onSelect } = renderPanel();

		await user.click(screen.getByText('Wood'));

		expect(onSelect).toHaveBeenCalledTimes(1);
		expect(onSelect).toHaveBeenCalledWith('Wood');
	});

	it('leaves clearing the search to the page', async () => {
		const user = userEvent.setup();
		const { onReset, onSelect } = renderPanel({ searchQuery: 'woo' });

		await user.click(document.querySelector('#guild-inventory-reset') as HTMLElement);

		expect(onReset).toHaveBeenCalledTimes(1);
		expect(onSelect).not.toHaveBeenCalled();
	});

	it('shows the search the page gave it', () => {
		renderPanel({ searchQuery: 'woo' });
		expect((screen.getByPlaceholderText('Search Inventory') as HTMLInputElement).value).toBe('woo');
	});

	it('renders nothing but the header for an empty base', () => {
		renderPanel({ items: [] });
		expect(screen.queryByText('Wood')).toBeNull();
		expect(screen.getByText('Inventory')).toBeTruthy();
	});
});
