// @vitest-environment jsdom
import '$utils/__tests__/fixtures/animatePolyfill';
import { render, screen } from '@testing-library/svelte';
import userEvent from '@testing-library/user-event';
import { describe, expect, it, vi } from 'vitest';

vi.mock('$lib/data', () => ({
	buildingsData: {
		getByKey: (key: string) =>
			key === 'Chest' ? { localized_name: 'Wooden Chest', icon: 'chest', type_a: 0 } : undefined
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

const { default: GuildStorage } = await import('./GuildStorage.svelte');

function container(id: string, key: string, filled: number, slotCount = 3) {
	return {
		id,
		key,
		type: '',
		slot_num: slotCount,
		slots: Array.from({ length: slotCount }, (_, slot_index) => ({
			static_id: slot_index < filled ? 'Wood' : 'None',
			slot_index,
			count: slot_index < filled ? 10 : 0
		}))
	} as never;
}

function renderStorage(props: Record<string, unknown> = {}) {
	const handlers = { onSelect: vi.fn(), onUpdate: vi.fn(), onCopyPaste: vi.fn() };
	render(GuildStorage, {
		props: {
			containers: [container('c1', 'Chest', 2), container('c2', 'Mystery', 0)],
			selected: undefined,
			...handlers,
			...props
		}
	});
	return handlers;
}

describe('GuildStorage', () => {
	it('lists each container by the name the build data gives it', () => {
		renderStorage();
		expect(screen.getByText('Wooden Chest')).toBeTruthy();
		expect(screen.getByText('Mystery')).toBeTruthy();
	});

	it('asks for a container before it shows a grid', () => {
		renderStorage();
		expect(screen.getByText('Select Storage Container')).toBeTruthy();
		expect(document.querySelector('.grid-cols-3')).toBeNull();
	});

	it('reports the container a row stands for', async () => {
		const user = userEvent.setup();
		const { onSelect } = renderStorage();

		await user.click(screen.getByText('Wooden Chest'));

		expect(onSelect).toHaveBeenCalledTimes(1);
		expect(onSelect.mock.calls[0][0]).toMatchObject({ id: 'c1', key: 'Chest' });
	});

	it('shows the slots of the container the page picked', () => {
		renderStorage({ selected: container('c1', 'Chest', 2, 4) });
		expect(screen.queryByText('Select Storage Container')).toBeNull();
		expect(screen.getByAltText('Storage Container Icon')).toBeTruthy();
	});

	it('stacks the container list over the grid below the breakpoint', () => {
		renderStorage();

		const content = document.querySelector('#guild-storage-content');
		expect(content).not.toBeNull();
		expect(content?.className).toContain('flex-col');
		expect(content?.className).toContain('md:flex-row');
	});

	it('gives the container list the full width it is stacked in', () => {
		renderStorage();

		const list = document.querySelector('#guild-storage-containers');
		expect(list).not.toBeNull();
		const unprefixed = (list?.className ?? '').split(/  */).filter((token) => !token.includes(':'));
		expect(unprefixed).not.toContain('w-1/4');
	});

	it('says so when the base has no containers to show', () => {
		renderStorage({ containers: [] });
		expect(screen.getByText('No Storage Containers')).toBeTruthy();
		expect(document.querySelector('#guild-storage-content')).toBeNull();
	});
});
