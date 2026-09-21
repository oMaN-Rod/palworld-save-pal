// @vitest-environment jsdom
import { fireEvent, render, screen } from '@testing-library/svelte';
import { afterEach, describe, expect, it } from 'vitest';

import PalPager from '../PalPager.svelte';

describe('PalPager', () => {
	it('renders one control per page plus prev/next', () => {
		render(PalPager, { page: 1, pageCount: 3, label: 'Palbox pages' });

		expect(screen.getByRole('button', { name: /page 1/i })).not.toBeNull();
		expect(screen.getByRole('button', { name: /page 2/i })).not.toBeNull();
		expect(screen.getByRole('button', { name: /page 3/i })).not.toBeNull();
		expect(screen.getByRole('button', { name: /previous/i })).not.toBeNull();
		expect(screen.getByRole('button', { name: /next/i })).not.toBeNull();

		const nav = screen.getByRole('navigation', { name: 'Palbox pages' });
		expect(nav.querySelectorAll('button').length).toBe(5);
	});

	it('renders exactly the number of controls pageCount reports for a non-multiple pal count', () => {
		render(PalPager, { page: 1, pageCount: 2, label: 'Palbox pages' });

		const nav = screen.getByRole('navigation', { name: 'Palbox pages' });
		const pageButtons = [...nav.querySelectorAll('button')].filter((button) =>
			/^page \d+$/i.test(button.getAttribute('aria-label') ?? '')
		);
		expect(pageButtons).toHaveLength(2);
	});

	it('marks the current page with aria-current="page" and only that page', () => {
		render(PalPager, { page: 2, pageCount: 3, label: 'Palbox pages' });

		expect(screen.getByRole('button', { name: /page 1/i }).getAttribute('aria-current')).toBeNull();
		expect(screen.getByRole('button', { name: /page 2/i }).getAttribute('aria-current')).toBe(
			'page'
		);
		expect(screen.getByRole('button', { name: /page 3/i }).getAttribute('aria-current')).toBeNull();
	});

	it('disables prev on page 1 and next on the last page', async () => {
		const { rerender } = render(PalPager, { page: 1, pageCount: 3, label: 'Palbox pages' });
		expect((screen.getByRole('button', { name: /previous/i }) as HTMLButtonElement).disabled).toBe(
			true
		);
		expect((screen.getByRole('button', { name: /next/i }) as HTMLButtonElement).disabled).toBe(
			false
		);

		await rerender({ page: 3, pageCount: 3, label: 'Palbox pages' });
		expect((screen.getByRole('button', { name: /previous/i }) as HTMLButtonElement).disabled).toBe(
			false
		);
		expect((screen.getByRole('button', { name: /next/i }) as HTMLButtonElement).disabled).toBe(
			true
		);
	});

	it('neither prev nor next is disabled on a middle page', () => {
		render(PalPager, { page: 2, pageCount: 3, label: 'Palbox pages' });
		expect((screen.getByRole('button', { name: /previous/i }) as HTMLButtonElement).disabled).toBe(
			false
		);
		expect((screen.getByRole('button', { name: /next/i }) as HTMLButtonElement).disabled).toBe(
			false
		);
	});

	it('clicking a page number updates the bound value', async () => {
		let page = 1;
		render(PalPager, {
			get page() {
				return page;
			},
			set page(value) {
				page = value;
			},
			pageCount: 3,
			label: 'Palbox pages'
		});

		await fireEvent.click(screen.getByRole('button', { name: /page 3/i }));
		expect(page).toBe(3);
	});

	it('clicking next/prev advances and retreats the bound page', async () => {
		let page = 2;
		render(PalPager, {
			get page() {
				return page;
			},
			set page(value) {
				page = value;
			},
			pageCount: 3,
			label: 'Palbox pages'
		});

		await fireEvent.click(screen.getByRole('button', { name: /next/i }));
		expect(page).toBe(3);

		await fireEvent.click(screen.getByRole('button', { name: /previous/i }));
		expect(page).toBe(2);
	});

	it('clicking prev while disabled on page 1 does not move the page', async () => {
		let page = 1;
		render(PalPager, {
			get page() {
				return page;
			},
			set page(value) {
				page = value;
			},
			pageCount: 3,
			label: 'Palbox pages'
		});

		await fireEvent.click(screen.getByRole('button', { name: /previous/i }));
		expect(page).toBe(1);
	});

	it('every control is a real button element, never a div with a click handler', () => {
		const { container } = render(PalPager, { page: 1, pageCount: 4, label: 'Palbox pages' });

		const nav = container.querySelector('nav');
		expect(nav).not.toBeNull();
		const buttons = nav!.querySelectorAll('button');
		expect(buttons.length).toBe(6);
		buttons.forEach((button) => expect(button.tagName).toBe('BUTTON'));
		expect(nav!.querySelectorAll('div[onclick], [role="button"]:not(button)').length).toBe(0);
	});

	function pageButtonLabels(nav: HTMLElement): string[] {
		return [...nav.querySelectorAll('button')]
			.map((button) => button.getAttribute('aria-label') ?? '')
			.filter((label) => /^page \d+$/i.test(label));
	}

	function renderPager(props: { page: number; pageCount: number; windowSize?: number }) {
		render(PalPager, { ...props, label: 'Palbox pages' });
		return screen.getByRole('navigation', { name: 'Palbox pages' });
	}

	describe('windowing', () => {
		it('renders at most windowSize page bubbles when there are more pages than the window', () => {
			const nav = renderPager({ page: 1, pageCount: 32 });
			expect(pageButtonLabels(nav)).toHaveLength(16);
		});

		it('slides the window so a high current page is shown and low pages are dropped', () => {
			const nav = renderPager({ page: 30, pageCount: 40 });

			expect(pageButtonLabels(nav)).toEqual(Array.from({ length: 16 }, (_, i) => `Page ${22 + i}`));
			expect(screen.getByRole('button', { name: /^page 30$/i })).not.toBeNull();
			expect(screen.queryByRole('button', { name: /^page 1$/i })).toBeNull();
			expect(screen.queryByRole('button', { name: /^page 21$/i })).toBeNull();
			expect(screen.queryByRole('button', { name: /^page 38$/i })).toBeNull();
		});

		it('clamps the window at the low end: page 1 shows pages 1..windowSize', () => {
			const nav = renderPager({ page: 1, pageCount: 32 });

			expect(pageButtonLabels(nav)).toEqual(Array.from({ length: 16 }, (_, i) => `Page ${1 + i}`));
			expect(screen.queryByRole('button', { name: /^page 17$/i })).toBeNull();
		});

		it('clamps the window at the high end: the last page shows the final windowSize pages', () => {
			const nav = renderPager({ page: 32, pageCount: 32 });

			expect(pageButtonLabels(nav)).toEqual(Array.from({ length: 16 }, (_, i) => `Page ${17 + i}`));
			expect(screen.queryByRole('button', { name: /^page 33$/i })).toBeNull();
			expect(screen.queryByRole('button', { name: /^page 16$/i })).toBeNull();
		});

		it('renders every page and no padding when there are fewer pages than the window', () => {
			const nav = renderPager({ page: 2, pageCount: 4 });

			expect(pageButtonLabels(nav)).toEqual(['Page 1', 'Page 2', 'Page 3', 'Page 4']);
			const bubbles = [...nav.querySelectorAll('button')].filter((button) =>
				/^page \d+$/i.test(button.getAttribute('aria-label') ?? '')
			);
			bubbles.forEach((bubble) => expect(bubble.textContent?.trim()).toMatch(/^\d+$/));
		});

		it('honours a smaller custom windowSize', () => {
			const nav = renderPager({ page: 10, pageCount: 20, windowSize: 5 });

			expect(pageButtonLabels(nav)).toEqual(['Page 8', 'Page 9', 'Page 10', 'Page 11', 'Page 12']);
		});

		it('keeps prev/next moving one page at a time while the window is active', async () => {
			let page = 20;
			render(PalPager, {
				get page() {
					return page;
				},
				set page(value) {
					page = value;
				},
				pageCount: 40,
				label: 'Palbox pages'
			});

			await fireEvent.click(screen.getByRole('button', { name: /next/i }));
			expect(page).toBe(21);

			await fireEvent.click(screen.getByRole('button', { name: /previous/i }));
			expect(page).toBe(20);
		});

		it('still disables prev on page 1 and next on the last page with the window active', async () => {
			const { rerender } = render(PalPager, { page: 1, pageCount: 40, label: 'Palbox pages' });
			expect(
				(screen.getByRole('button', { name: /previous/i }) as HTMLButtonElement).disabled
			).toBe(true);
			expect((screen.getByRole('button', { name: /next/i }) as HTMLButtonElement).disabled).toBe(
				false
			);

			await rerender({ page: 40, pageCount: 40, label: 'Palbox pages' });
			expect(
				(screen.getByRole('button', { name: /previous/i }) as HTMLButtonElement).disabled
			).toBe(false);
			expect((screen.getByRole('button', { name: /next/i }) as HTMLButtonElement).disabled).toBe(
				true
			);
		});
	});

	describe('keyboard paging', () => {
		// testing-library's cleanup does not reach fixtures mounted outside the container.
		const FIXTURE = 'data-pager-kb-fixture';

		afterEach(() => {
			document.querySelectorAll(`[${FIXTURE}]`).forEach((node) => node.remove());
		});

		function renderBound(page: number, pageCount: number): { page: number } {
			const state = { page };
			render(PalPager, {
				get page() {
					return state.page;
				},
				set page(value: number) {
					state.page = value;
				},
				pageCount,
				label: 'Palbox pages'
			});
			return state;
		}

		function mount(tag: string, attributes: Record<string, string> = {}): HTMLElement {
			const element = document.createElement(tag);
			element.setAttribute(FIXTURE, '');
			for (const [name, value] of Object.entries(attributes)) {
				element.setAttribute(name, value);
			}
			document.body.append(element);
			return element;
		}

		it('steps back a page on ArrowLeft and on q', async () => {
			const state = renderBound(3, 5);

			await fireEvent.keyDown(window, { key: 'ArrowLeft' });
			expect(state.page).toBe(2);

			await fireEvent.keyDown(window, { key: 'q' });
			expect(state.page).toBe(1);
		});

		it('steps forward a page on ArrowRight and on e', async () => {
			const state = renderBound(1, 5);

			await fireEvent.keyDown(window, { key: 'ArrowRight' });
			expect(state.page).toBe(2);

			await fireEvent.keyDown(window, { key: 'e' });
			expect(state.page).toBe(3);
		});

		it('pages on uppercase Q and E, as the pre-migration handler did', async () => {
			const state = renderBound(3, 5);

			await fireEvent.keyDown(window, { key: 'Q', shiftKey: true });
			expect(state.page).toBe(2);

			await fireEvent.keyDown(window, { key: 'E', shiftKey: true });
			expect(state.page).toBe(3);
		});

		it('clamps at the first page instead of wrapping to the last', async () => {
			const state = renderBound(1, 3);

			await fireEvent.keyDown(window, { key: 'ArrowLeft' });
			expect(state.page).toBe(1);

			await fireEvent.keyDown(window, { key: 'q' });
			expect(state.page).toBe(1);
		});

		it('clamps at the last page instead of wrapping to the first', async () => {
			const state = renderBound(3, 3);

			await fireEvent.keyDown(window, { key: 'ArrowRight' });
			expect(state.page).toBe(3);

			await fireEvent.keyDown(window, { key: 'e' });
			expect(state.page).toBe(3);
		});

		it('ignores keys typed into a text input', async () => {
			const state = renderBound(2, 5);
			const input = mount('input', { type: 'search' });

			await fireEvent.keyDown(input, { key: 'q' });
			await fireEvent.keyDown(input, { key: 'e' });
			await fireEvent.keyDown(input, { key: 'ArrowRight' });

			expect(state.page).toBe(2);
		});

		it('ignores keys typed into a textarea', async () => {
			const state = renderBound(2, 5);
			const textarea = mount('textarea');

			await fireEvent.keyDown(textarea, { key: 'q' });
			await fireEvent.keyDown(textarea, { key: 'ArrowRight' });

			expect(state.page).toBe(2);
		});

		it('ignores keys typed inside a contenteditable region', async () => {
			const state = renderBound(2, 5);
			const editable = mount('div', { contenteditable: 'true' });
			const inner = document.createElement('span');
			editable.append(inner);

			await fireEvent.keyDown(inner, { key: 'q' });
			await fireEvent.keyDown(inner, { key: 'ArrowRight' });

			expect(state.page).toBe(2);
		});

		it('still pages from a target that only looks editable', async () => {
			const state = renderBound(2, 5);
			const inert = mount('div', { contenteditable: 'false' });

			await fireEvent.keyDown(inert, { key: 'ArrowRight' });

			expect(state.page).toBe(3);
		});

		it('ignores keys while a dialog is open', async () => {
			const state = renderBound(2, 5);
			const dialog = mount('div', { role: 'dialog', 'aria-modal': 'true' });

			await fireEvent.keyDown(window, { key: 'q' });
			await fireEvent.keyDown(window, { key: 'ArrowRight' });
			expect(state.page).toBe(2);

			dialog.remove();
			await fireEvent.keyDown(window, { key: 'ArrowRight' });
			expect(state.page).toBe(3);
		});

		it('ignores a keystroke carrying ctrl, meta or alt', async () => {
			const state = renderBound(3, 5);

			await fireEvent.keyDown(window, { key: 'ArrowRight', ctrlKey: true });
			await fireEvent.keyDown(window, { key: 'ArrowLeft', metaKey: true });
			await fireEvent.keyDown(window, { key: 'e', altKey: true });
			await fireEvent.keyDown(window, { key: 'q', ctrlKey: true });

			expect(state.page).toBe(3);
		});

		it('leaves other keys alone', async () => {
			const state = renderBound(2, 5);

			await fireEvent.keyDown(window, { key: 'a' });
			await fireEvent.keyDown(window, { key: 'ArrowUp' });
			await fireEvent.keyDown(window, { key: 'Enter' });

			expect(state.page).toBe(2);
		});

		it('stops listening once unmounted, so a later keystroke moves nothing', async () => {
			const state = { page: 2 };
			const { unmount } = render(PalPager, {
				get page() {
					return state.page;
				},
				set page(value: number) {
					state.page = value;
				},
				pageCount: 5,
				label: 'Palbox pages'
			});

			await fireEvent.keyDown(window, { key: 'ArrowRight' });
			expect(state.page).toBe(3);

			unmount();
			expect(screen.queryAllByRole('navigation', { name: 'Palbox pages' })).toHaveLength(0);

			await fireEvent.keyDown(window, { key: 'ArrowRight' });
			await fireEvent.keyDown(window, { key: 'ArrowLeft' });
			expect(state.page).toBe(3);
		});

		it('moves every mounted pager when two are up at once', async () => {
			const first = renderBound(2, 5);
			const second = renderBound(2, 5);

			expect(screen.queryAllByRole('navigation', { name: 'Palbox pages' })).toHaveLength(2);

			await fireEvent.keyDown(window, { key: 'ArrowRight' });
			expect(first.page).toBe(3);
			expect(second.page).toBe(3);
		});
	});
});
