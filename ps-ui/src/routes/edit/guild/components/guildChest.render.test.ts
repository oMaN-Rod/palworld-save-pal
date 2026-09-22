// @vitest-environment jsdom
import '$utils/__tests__/fixtures/animatePolyfill';
import { render, screen } from '@testing-library/svelte';
import { describe, expect, it, vi } from 'vitest';

vi.mock('$lib/data', () => ({
	buildingsData: {
		getByKey: (key: string) =>
			key === 'GuildChest'
				? { localized_name: 'Guild Chest', icon: 'guild-chest', type_a: 0 }
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

const { default: GuildChest } = await import('./GuildChest.svelte');

function chest(key: string, slotCount: number) {
	return {
		id: 'chest',
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

function renderChest(props: Record<string, unknown> = {}) {
	const handlers = { onUpdate: vi.fn(), onCopyPaste: vi.fn() };
	render(GuildChest, { props: { chest: chest('GuildChest', 4), ...handlers, ...props } });
	return handlers;
}

describe('GuildChest', () => {
	it('lays out one badge per chest slot', () => {
		renderChest({ chest: chest('GuildChest', 6) });
		expect(document.querySelector('.grid')?.children).toHaveLength(6);
	});

	it('reads the chest building however the container is keyed', () => {
		renderChest({ chest: chest('SomethingElse', 2) });
		const icon = screen.getByAltText('Storage Container Icon') as HTMLImageElement;
		expect(icon.getAttribute('src')).toContain('guild-chest');
	});

	it('keeps the id the docs and tours anchor on', () => {
		renderChest();
		expect(document.querySelector('#guild-chest-content')).toBeTruthy();
	});
});
