// @vitest-environment jsdom
import { render, screen } from '@testing-library/svelte';
import { describe, expect, it } from 'vitest';
import PalGridHost from './fixtures/PalGridHost.svelte';

describe('PalGrid', () => {
	it('applies the shared grid classes and renders its children', () => {
		render(PalGridHost);

		const child = screen.getByTestId('child');
		expect(child.textContent).toBe('child');

		const grid = child.parentElement as HTMLElement;
		expect(grid.className).toBe(
			'grid grid-cols-3 place-items-center gap-4 p-4 sm:grid-cols-4 md:grid-cols-5 lg:grid-cols-6'
		);
	});
});
