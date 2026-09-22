// @vitest-environment jsdom
import '$utils/__tests__/fixtures/animatePolyfill';
import { render, screen } from '@testing-library/svelte';
import userEvent from '@testing-library/user-event';
import { describe, expect, it, vi } from 'vitest';

import GuildBasePager from './GuildBasePager.svelte';

function bubbles(): HTMLElement[] {
	return Array.from(document.querySelectorAll('#guild-pager button')).filter((button) =>
		/^\d+$/.test(button.textContent?.trim() ?? '')
	) as HTMLElement[];
}

function labels(): string[] {
	return bubbles().map((button) => button.textContent?.trim() ?? '');
}

function renderPager(props: Partial<Record<string, unknown>> = {}) {
	const handlers = { onSelect: vi.fn(), onPrevious: vi.fn(), onNext: vi.fn() };
	render(GuildBasePager, { props: { total: 3, current: 1, ...handlers, ...props } });
	return handlers;
}

describe('GuildBasePager', () => {
	it('shows one bubble per base', () => {
		renderPager();
		expect(labels()).toEqual(['1', '2', '3']);
	});

	it('reports the base a bubble stands for', async () => {
		const user = userEvent.setup();
		const { onSelect } = renderPager();

		await user.click(bubbles()[2]);

		expect(onSelect).toHaveBeenCalledTimes(1);
		expect(onSelect).toHaveBeenCalledWith(3);
	});

	it('leaves the step buttons to the page', async () => {
		const user = userEvent.setup();
		const { onPrevious, onNext, onSelect } = renderPager({ current: 2 });

		await user.click(screen.getByRole('button', { name: 'Next' }));
		await user.click(screen.getByRole('button', { name: 'Previous' }));

		expect(onNext).toHaveBeenCalledTimes(1);
		expect(onPrevious).toHaveBeenCalledTimes(1);
		expect(onSelect).not.toHaveBeenCalled();
	});

	it('windows a long base list around the current base', () => {
		renderPager({ total: 40, current: 20, visibleCount: 5 });
		expect(labels()).toEqual(['18', '19', '20', '21', '22']);
	});

	it('pins the window at the first base', () => {
		renderPager({ total: 40, current: 1, visibleCount: 5 });
		expect(labels()).toEqual(['1', '2', '3', '4', '5']);
	});

	it('pins the window at the last base', () => {
		renderPager({ total: 40, current: 40, visibleCount: 5 });
		expect(labels()).toEqual(['36', '37', '38', '39', '40']);
	});

	it('never shows more bubbles than there are bases', () => {
		renderPager({ total: 2, current: 1, visibleCount: 5 });
		expect(labels()).toEqual(['1', '2']);
	});
});
