// @vitest-environment jsdom
import { fireEvent, render, screen } from '@testing-library/svelte';
import { tick } from 'svelte';
import { describe, expect, it, vi } from 'vitest';

globalThis.ResizeObserver ??= class {
	observe() {}
	unobserve() {}
	disconnect() {}
} as unknown as typeof ResizeObserver;

import Combobox from '$components/ui/combobox/Combobox.svelte';
import Select from '../Select.svelte';

const options = [
	{ label: 'Alpha', value: 'a' },
	{ label: 'Beta', value: 'b' },
	{ label: 'Gamma', value: 'c' }
];

describe('dropdown popups', () => {
	it('lifts the Select list out of its container', async () => {
		const { container } = render(Select, { options, value: 'a' });
		await fireEvent.click(screen.getByRole('combobox'));
		await tick();

		const listbox = screen.getByRole('listbox');
		expect(container.contains(listbox)).toBe(false);
		expect(listbox.style.position).toBe('fixed');
	});

	it('closes the Select after picking the option already chosen', async () => {
		render(Select, { options, value: 'a', onChange: vi.fn() });
		await fireEvent.click(screen.getByRole('combobox'));
		await tick();

		await fireEvent.click(screen.getByRole('option', { name: 'Alpha' }));
		await tick();

		expect(screen.queryByRole('listbox')).toBeNull();
	});

	it('lifts the Combobox list out of its container', async () => {
		const { container } = render(Combobox, { options });
		await fireEvent.focus(screen.getByRole('textbox'));
		await tick();

		const listbox = screen.getByRole('listbox');
		expect(container.contains(listbox)).toBe(false);
		expect(listbox.style.position).toBe('fixed');
	});
});
