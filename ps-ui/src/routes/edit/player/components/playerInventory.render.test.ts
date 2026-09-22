// @vitest-environment jsdom
import '$utils/__tests__/fixtures/animatePolyfill';
import { render, screen } from '@testing-library/svelte';
import userEvent from '@testing-library/user-event';
import { describe, expect, it, vi } from 'vitest';

vi.mock('$lib/data', () => ({
	itemsData: { getByKey: () => undefined },
	palsData: { getByKey: () => undefined },
	buildingsData: { getByKey: () => undefined }
}));

vi.mock('$utils', async (importOriginal) => {
	const actual = await importOriginal<Record<string, unknown>>();
	return {
		...actual,
		assetLoader: { loadImage: (path: string) => path, loadMenuImage: () => '' }
	};
});

const { default: PlayerInventory } = await import('./PlayerInventory.svelte');

function container(slotCount: number) {
	return {
		id: 'c',
		key: '',
		type: '',
		slot_num: slotCount,
		slots: Array.from({ length: slotCount }, (_, slot_index) => ({
			static_id: 'None',
			slot_index,
			count: 0
		}))
	} as never;
}

function badges(panelId: string): HTMLElement[] {
	return Array.from(document.querySelectorAll(`#${panelId} button`)) as HTMLElement[];
}

function renderInventory(props: Record<string, unknown> = {}) {
	const handlers = { onUpdate: vi.fn(), onCopyPaste: vi.fn() };
	render(PlayerInventory, {
		props: {
			commonContainer: container(4),
			essentialContainer: container(2),
			group: 'inventory',
			...handlers,
			...props
		}
	});
	return handlers;
}

describe('PlayerInventory', () => {
	it('gives each container its own panel of slots', () => {
		renderInventory();
		expect(badges('inventory-panel')).toHaveLength(4);
		expect(badges('key-items-panel')).toHaveLength(2);
	});

	it('opens on the tab the page asked for', async () => {
		const user = userEvent.setup();
		renderInventory({ group: 'key_items' });

		const keyItems = screen.getByText('Key Items').closest('button');
		expect(keyItems?.getAttribute('aria-selected')).toBe('true');

		await user.click(screen.getByText('Inventory'));
		expect(keyItems?.getAttribute('aria-selected')).toBe('false');
	});

	it('lets the common container take a paste and the key items refuse one', async () => {
		const user = userEvent.setup();
		const { onCopyPaste } = renderInventory();

		await user.pointer({ keys: '[MouseRight]', target: badges('inventory-panel')[0] });
		await user.pointer({ keys: '[MouseRight]', target: badges('key-items-panel')[0] });

		expect(onCopyPaste).toHaveBeenCalledTimes(2);
		expect(onCopyPaste.mock.calls[0][2]).toBe(true);
		expect(onCopyPaste.mock.calls[1][2]).toBe(false);
	});

	it('reports the slot the badge stands for', async () => {
		const user = userEvent.setup();
		const { onCopyPaste } = renderInventory();

		await user.pointer({ keys: '[MouseRight]', target: badges('inventory-panel')[2] });

		expect(onCopyPaste.mock.calls[0][1]).toMatchObject({ slot_index: 2 });
	});
});
