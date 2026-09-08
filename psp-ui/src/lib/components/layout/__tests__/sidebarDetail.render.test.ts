// @vitest-environment jsdom
import { render, screen } from '@testing-library/svelte';
import { describe, expect, it } from 'vitest';
import SidebarDetailHost from './fixtures/SidebarDetailHost.svelte';

function spacer(container: HTMLElement): HTMLElement {
	return container.querySelector('.transition-all') as HTMLElement;
}

describe('SidebarDetail', () => {
	it('always renders the sidebar', () => {
		render(SidebarDetailHost);

		expect(screen.getByTestId('sidebar-content')).toBeTruthy();
	});

	it('omits the detail pane when detailActive is false', () => {
		render(SidebarDetailHost, { props: { detailActive: false } });

		expect(screen.queryByTestId('detail-content')).toBeNull();
	});

	it('renders the detail pane when detailActive is true', async () => {
		render(SidebarDetailHost, { props: { detailActive: true } });

		expect(await screen.findByTestId('detail-content')).toBeTruthy();
	});

	it('collapses the spacer when detailActive is true', () => {
		const { container: inactive } = render(SidebarDetailHost, {
			props: { detailActive: false }
		});
		expect(spacer(inactive).style.flexGrow).toBe('1');

		const { container: active } = render(SidebarDetailHost, { props: { detailActive: true } });
		expect(spacer(active).style.flexGrow).toBe('0');
	});
});
