// @vitest-environment jsdom
import { render, screen } from '@testing-library/svelte';
import { tick } from 'svelte';
import { beforeEach, describe, expect, it, vi } from 'vitest';

const { appState, modalState } = vi.hoisted(() => ({
	appState: {
		selectedPlayer: undefined as unknown,
		settings: { cheat_mode: false, debug_mode: false }
	},
	modalState: { showModal: vi.fn() }
}));

vi.mock('$states', () => ({
	getAppState: () => appState,
	getModalState: () => modalState
}));

vi.mock('$components/modals', () => ({ NumberInputModal: {} }));

function technology(id: string, levelCap: number, boss: boolean) {
	return {
		localized_name: id,
		description: '',
		details: {
			level_cap: levelCap,
			is_boss_technology: boss,
			cost: 2,
			icon: 'icon',
			unlock_build_objects: [],
			unlock_item_recipes: []
		}
	};
}

// Eight is the widest tier the game ships.
const TECHNOLOGIES: Record<string, ReturnType<typeof technology>> = {
	...Object.fromEntries(
		Array.from({ length: 8 }, (_, index) => [
			`Tech_${index}`,
			technology(`Tech_${index}`, 5, false)
		])
	),
	Boss_5: technology('Boss_5', 5, true),
	Tech_lone: technology('Tech_lone', 10, false)
};

vi.mock('$lib/data', () => ({
	technologiesData: {
		technologies: TECHNOLOGIES,
		getByKey: (key: string) => TECHNOLOGIES[key]
	},
	buildingsData: { getByKey: () => undefined },
	itemsData: { getByKey: () => undefined }
}));

vi.mock('$utils', async (importOriginal) => {
	const actual = await importOriginal<Record<string, unknown>>();
	return { ...actual, assetLoader: { loadImage: (path: string) => path } };
});

const { default: TechnologiesPage } = await import('../+page.svelte');

beforeEach(() => {
	vi.clearAllMocks();
	appState.selectedPlayer = {
		technologies: [],
		technology_points: 12,
		boss_technology_points: 3,
		state: 0
	};
});

// jsdom applies no CSS, so these pin the breakpoint classes instead.
describe('technologies page', () => {
	it('pins the point counters while the tree scrolls under them on a phone', async () => {
		render(TechnologiesPage);
		await tick();

		const summary = document.querySelector('#tech-summary');
		expect(summary).not.toBeNull();
		expect(summary?.className).toContain('sticky');
		expect(summary?.className).toContain('md:static');
	});

	it('keeps both counters reachable from the pinned header', async () => {
		render(TechnologiesPage);
		await tick();

		expect(document.querySelector('#tech-summary #tech-points')).not.toBeNull();
		expect(document.querySelector('#tech-summary #ancient-tech-points')).not.toBeNull();
	});

	it('wraps a tier instead of running it off the side', async () => {
		render(TechnologiesPage);
		await tick();

		const tiers = document.querySelectorAll('#tech-grid > div');
		expect(tiers.length).toBeGreaterThan(0);
		for (const tier of tiers) {
			// By position: a descendant selector could match a nested div and pass anyway.
			const row = tier.children[1];
			expect(row?.className).toContain('flex-wrap');
			expect(row?.className).toContain('md:flex-nowrap');
		}
	});

	it('drops the alignment spacers where the tier wraps', async () => {
		render(TechnologiesPage);
		await tick();

		const spacers = document.querySelectorAll('#tech-grid div.w-24:not([class*="md:hidden"])');
		for (const spacer of spacers) {
			if (spacer.children.length === 0 && spacer.textContent?.trim() === '') {
				expect(spacer.className).toContain('hidden');
				expect(spacer.className).toContain('md:block');
			}
		}
	});

	it('is a tree, not a tab strip', async () => {
		render(TechnologiesPage);
		await tick();

		expect(screen.queryByRole('tablist')).toBeNull();
	});
});
