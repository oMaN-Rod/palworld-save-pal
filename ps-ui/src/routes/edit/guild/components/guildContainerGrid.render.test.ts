// @vitest-environment jsdom
import '$utils/__tests__/fixtures/animatePolyfill';
import { render, screen } from '@testing-library/svelte';
import { describe, expect, it, vi } from 'vitest';

vi.mock('$lib/data', () => ({
	buildingsData: {
		getByKey: (key: string) =>
			key === 'Chest'
				? { localized_name: 'Wooden Chest', icon: 'chest', type_a: 0 }
				: key === 'Fridge'
					? { localized_name: 'Fridge', icon: 'fridge', type_a: 3 }
					: undefined
	},
	itemsData: { getByKey: () => undefined },
	palsData: { getByKey: () => undefined }
}));

vi.mock('$components/presets', () => ({ StoragePresets: vi.fn() }));

vi.mock('$utils', async (importOriginal) => {
	const actual = await importOriginal<Record<string, unknown>>();
	return {
		...actual,
		assetLoader: { loadImage: (path: string) => path, loadMenuImage: () => '' }
	};
});

const { default: GuildContainerGrid } = await import('./GuildContainerGrid.svelte');

function container(key: string, slotCount: number) {
	return {
		id: 'c1',
		key,
		type: '',
		slot_num: slotCount,
		slots: Array.from({ length: slotCount }, (_, slot_index) => ({
			static_id: 'None',
			slot_index,
			count: 0
		}))
	} as never;
}

function renderGrid(props: Record<string, unknown> = {}) {
	const handlers = { onUpdate: vi.fn(), onCopyPaste: vi.fn() };
	render(GuildContainerGrid, {
		props: { container: container('Chest', 3), ...handlers, ...props }
	});
	return handlers;
}

describe('GuildContainerGrid', () => {
	it('lays out one badge per slot the container declares', () => {
		renderGrid({ container: container('Chest', 5) });
		const grid = document.querySelector('.grid');
		expect(grid?.children).toHaveLength(5);
	});

	it('draws the container icon the building data names', () => {
		renderGrid();
		const icon = screen.getByAltText('Storage Container Icon') as HTMLImageElement;
		expect(icon.getAttribute('src')).toContain('chest');
	});

	it('falls back to the unknown icon for an unrecognised container', () => {
		renderGrid({ container: container('Mystery', 1) });
		expect(screen.getByAltText('Storage Container Icon')).toBeTruthy();
	});

	it('takes the building key over the container key when given one', () => {
		renderGrid({ container: container('Mystery', 1), buildingKey: 'Fridge' });
		const icon = screen.getByAltText('Storage Container Icon') as HTMLImageElement;
		expect(icon.getAttribute('src')).toContain('fridge');
	});
});
