// @vitest-environment jsdom
import '$utils/__tests__/fixtures/animatePolyfill';
import { render, screen, waitFor } from '@testing-library/svelte';
import userEvent from '@testing-library/user-event';
import { describe, expect, it, vi } from 'vitest';

vi.mock('$lib/data', () => ({
	itemsData: {
		getByKey: (key: string) =>
			key === 'Helmet'
				? {
						id: 'Helmet',
						info: { localized_name: 'Helmet', description: '' },
						details: {
							icon: 'helmet-icon',
							type_a: 'Head',
							type_b: 'None',
							rarity: 0,
							weight: 1,
							max_stack_count: 1
						}
					}
				: undefined
	},
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

const { default: PlayerGear } = await import('./PlayerGear.svelte');

function slot(slot_index: number, static_id = 'None') {
	return { static_id, slot_index, count: 0 } as never;
}

function container(slotCount: number) {
	return {
		id: 'c',
		key: '',
		type: '',
		slot_num: slotCount,
		slots: Array.from({ length: slotCount }, (_, i) => slot(i))
	} as never;
}

function badges(sectionId: string): HTMLElement[] {
	return Array.from(document.querySelectorAll(`#${sectionId} button`)) as HTMLElement[];
}

function renderGear(props: Record<string, unknown> = {}) {
	const handlers = { onUpdate: vi.fn(), onCopyPaste: vi.fn() };
	render(PlayerGear, {
		props: {
			weaponLoadOutContainer: container(4),
			foodEquipContainer: container(3),
			accessoryGear: [slot(0), slot(1)],
			headGear: slot(0),
			bodyGear: slot(1),
			shieldGear: slot(2),
			gliderGear: slot(3),
			sphereModule: slot(4),
			...handlers,
			...props
		}
	});
	return handlers;
}

describe('PlayerGear', () => {
	it('gives every equipment section its own slots', () => {
		renderGear();
		expect(badges('weapon-equip')).toHaveLength(4);
		expect(badges('accessory-equip')).toHaveLength(2);
		expect(badges('food-equip')).toHaveLength(3);
		expect(badges('gear-equip')).toHaveLength(5);
	});

	it('labels the single-slot pieces', () => {
		renderGear();
		for (const label of ['Head', 'Body', 'Shield', 'Glider']) {
			expect(screen.getByText(label)).toBeTruthy();
		}
	});

	it('reports the slot behind a badge', async () => {
		const user = userEvent.setup();
		const { onCopyPaste } = renderGear();

		await user.pointer({ keys: '[MouseRight]', target: badges('weapon-equip')[2] });

		expect(onCopyPaste).toHaveBeenCalledTimes(1);
		expect(onCopyPaste.mock.calls[0][1]).toMatchObject({ slot_index: 2 });
	});

	it('draws the worn head and body pieces', async () => {
		renderGear({ headGear: slot(0, 'Helmet') });
		await waitFor(() => expect(screen.getByAltText('Helmet')).toBeTruthy());
	});

	it('draws nothing for an empty equipment slot', () => {
		renderGear();
		expect(screen.queryByAltText('None')).toBeNull();
	});
});
