// @vitest-environment jsdom
import { render, screen } from '@testing-library/svelte';
import userEvent from '@testing-library/user-event';
import { tick } from 'svelte';
import { describe, expect, it } from 'vitest';

import SectionTabsHarness from './SectionTabsHarness.svelte';

describe('SectionTabs', () => {
	it('renders a tab per entry inside a tablist', async () => {
		render(SectionTabsHarness, { active: 'stats' });
		await tick();

		expect(screen.getByRole('tablist')).not.toBeNull();
		expect(screen.getByRole('tab', { name: 'Stats' })).not.toBeNull();
		expect(screen.getByRole('tab', { name: 'Inventory' })).not.toBeNull();
		expect(screen.getByRole('tab', { name: 'Gear' })).not.toBeNull();
	});

	it('marks exactly one tab selected', async () => {
		render(SectionTabsHarness, { active: 'stats' });
		await tick();

		expect(screen.getByRole('tab', { name: 'Stats' }).getAttribute('aria-selected')).toBe('true');
		expect(screen.getByRole('tab', { name: 'Gear' }).getAttribute('aria-selected')).toBe('false');
	});

	it('reports the tab the user picked', async () => {
		render(SectionTabsHarness, { active: 'stats' });
		await tick();

		await userEvent.click(screen.getByRole('tab', { name: 'Gear' }));
		await tick();

		expect(screen.getByTestId('active-value').textContent).toBe('gear');
	});

	function expectRovingTabindex(activeName: string) {
		const tabs = screen.getAllByRole('tab');
		const zeroTabindex = tabs.filter((tab) => tab.getAttribute('tabindex') === '0');
		expect(zeroTabindex).toHaveLength(1);
		expect(zeroTabindex[0]).toBe(screen.getByRole('tab', { name: activeName }));
		for (const tab of tabs) {
			if (tab !== zeroTabindex[0]) {
				expect(tab.getAttribute('tabindex')).toBe('-1');
			}
		}
	}

	it('wraps ArrowRight from the last tab to the first', async () => {
		render(SectionTabsHarness, { active: 'gear' });
		await tick();

		await userEvent.click(screen.getByRole('tab', { name: 'Gear' }));
		await userEvent.keyboard('{ArrowRight}');
		await tick();

		expect(screen.getByTestId('active-value').textContent).toBe('stats');
		expect(document.activeElement).toBe(screen.getByRole('tab', { name: 'Stats' }));
		expectRovingTabindex('Stats');
	});

	it('wraps ArrowLeft from the first tab to the last', async () => {
		render(SectionTabsHarness, { active: 'stats' });
		await tick();

		await userEvent.click(screen.getByRole('tab', { name: 'Stats' }));
		await userEvent.keyboard('{ArrowLeft}');
		await tick();

		expect(screen.getByTestId('active-value').textContent).toBe('gear');
		expect(document.activeElement).toBe(screen.getByRole('tab', { name: 'Gear' }));
		expectRovingTabindex('Gear');
	});

	it('moves to the next tab with ArrowRight from the middle', async () => {
		render(SectionTabsHarness, { active: 'inventory' });
		await tick();

		await userEvent.click(screen.getByRole('tab', { name: 'Inventory' }));
		await userEvent.keyboard('{ArrowRight}');
		await tick();

		expect(screen.getByTestId('active-value').textContent).toBe('gear');
		expect(document.activeElement).toBe(screen.getByRole('tab', { name: 'Gear' }));
		expectRovingTabindex('Gear');
	});

	it('moves to the previous tab with ArrowLeft from the middle', async () => {
		render(SectionTabsHarness, { active: 'inventory' });
		await tick();

		await userEvent.click(screen.getByRole('tab', { name: 'Inventory' }));
		await userEvent.keyboard('{ArrowLeft}');
		await tick();

		expect(screen.getByTestId('active-value').textContent).toBe('stats');
		expect(document.activeElement).toBe(screen.getByRole('tab', { name: 'Stats' }));
		expectRovingTabindex('Stats');
	});

	it('selects the first tab on Home', async () => {
		render(SectionTabsHarness, { active: 'gear' });
		await tick();

		await userEvent.click(screen.getByRole('tab', { name: 'Gear' }));
		await userEvent.keyboard('{Home}');
		await tick();

		expect(screen.getByTestId('active-value').textContent).toBe('stats');
		expect(document.activeElement).toBe(screen.getByRole('tab', { name: 'Stats' }));
		expectRovingTabindex('Stats');
	});

	it('selects the last tab on End', async () => {
		render(SectionTabsHarness, { active: 'stats' });
		await tick();

		await userEvent.click(screen.getByRole('tab', { name: 'Stats' }));
		await userEvent.keyboard('{End}');
		await tick();

		expect(screen.getByTestId('active-value').textContent).toBe('gear');
		expect(document.activeElement).toBe(screen.getByRole('tab', { name: 'Gear' }));
		expectRovingTabindex('Gear');
	});
});
