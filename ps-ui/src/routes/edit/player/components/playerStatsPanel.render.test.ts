// @vitest-environment jsdom
import '$utils/__tests__/fixtures/animatePolyfill';
import { render, screen } from '@testing-library/svelte';
import userEvent from '@testing-library/user-event';
import { describe, expect, it, vi } from 'vitest';

vi.mock('$lib/data', () => ({
	expData: {
		expData: { 11: { TotalEXP: 1000, NextEXP: 400 } },
		getExpDataByLevel: vi.fn()
	},
	itemsData: { getByKey: () => undefined },
	palsData: { getByKey: () => undefined }
}));

vi.mock('$components/player', () => ({ PlayerStats: vi.fn(), PlayerHealthBadge: vi.fn() }));
vi.mock('$components/presets', () => ({ PlayerPresets: vi.fn() }));

vi.mock('$utils', async (importOriginal) => {
	const actual = await importOriginal<Record<string, unknown>>();
	return {
		...actual,
		assetLoader: { loadImage: (path: string) => path, loadMenuImage: () => '' }
	};
});

const { default: PlayerStatsPanel } = await import('./PlayerStatsPanel.svelte');

function player(overrides: Record<string, unknown> = {}) {
	return {
		uid: 'p1',
		nickname: 'Tourist',
		level: 10,
		exp: 700,
		last_online_time: 0,
		status_point_list: { max_hp: 0 },
		state: 0,
		...overrides
	} as never;
}

function renderPanel(props: Record<string, unknown> = {}) {
	const handlers = {
		onLevelIncrement: vi.fn(),
		onLevelDecrement: vi.fn(),
		onUpdateNickname: vi.fn()
	};
	render(PlayerStatsPanel, {
		props: { player: player(), maxLevel: 55, health: 500, ...handlers, ...props }
	});
	return handlers;
}

function progress(): HTMLElement | null {
	return document.querySelector('[role="progressbar"], progress');
}

describe('PlayerStatsPanel', () => {
	it('shows the player it was given', () => {
		renderPanel();
		expect(screen.getByText('Tourist')).toBeTruthy();
		expect(document.querySelector('#player-level')?.textContent).toContain('LEVEL');
	});

	it('measures progress against the next level', () => {
		renderPanel();
		expect(screen.getByText('300')).toBeTruthy();
		expect(progress()?.getAttribute('aria-valuemax') ?? '400').toBe('400');
	});

	it('flattens the bar at the level cap', () => {
		renderPanel({ player: player({ level: 55 }), maxLevel: 55 });
		expect(screen.getByText('0')).toBeTruthy();
	});

	it('leaves every level change to the page', async () => {
		const user = userEvent.setup();
		const handlers = renderPanel();
		const buttons = Array.from(document.querySelectorAll('#player-level button')) as HTMLElement[];

		await user.pointer({ keys: '[MouseLeft]', target: buttons[0] });
		await user.pointer({ keys: '[MouseLeft]', target: buttons[1] });

		expect(handlers.onLevelDecrement).toHaveBeenCalledTimes(1);
		expect(handlers.onLevelIncrement).toHaveBeenCalledTimes(1);
	});

	it('leaves renaming to the page', async () => {
		const user = userEvent.setup();
		const { onUpdateNickname } = renderPanel();

		await user.click(document.querySelector('#player-nickname') as HTMLElement);

		expect(onUpdateNickname).toHaveBeenCalledTimes(1);
	});

	it('opens on stats with presets collapsed', () => {
		renderPanel();
		expect(screen.getByText('Stats')).toBeTruthy();
		expect(document.querySelector('#player-presets-control')).toBeTruthy();
	});
});
