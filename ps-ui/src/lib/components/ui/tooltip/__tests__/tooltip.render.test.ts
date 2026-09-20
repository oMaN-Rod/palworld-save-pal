// @vitest-environment jsdom
import { fireEvent, render, screen } from '@testing-library/svelte';
import userEvent from '@testing-library/user-event';
import { tick } from 'svelte';
import { describe, expect, it } from 'vitest';

import '$utils/__tests__/fixtures/animatePolyfill';
import { installViewportStub } from '$utils/__tests__/fixtures/viewportStub';

const { setViewport } = installViewportStub();
const { default: TooltipHarness } = await import('./TooltipHarness.svelte');
const { default: TooltipMultiHarness } = await import('./TooltipMultiHarness.svelte');
const { default: TooltipClickThroughHarness } = await import('./TooltipClickThroughHarness.svelte');

function setPointer(coarse: boolean): void {
	setViewport(1440, { coarse });
}

describe('Tooltip', () => {
	it('opens on hover with a fine pointer', async () => {
		setPointer(false);
		render(TooltipHarness);
		await tick();

		await userEvent.hover(screen.getByTestId('tooltip-reference'));
		await tick();

		expect(screen.getByText('Sort inventory')).not.toBeNull();
	});

	it('ignores hover with a coarse pointer', async () => {
		setPointer(true);
		render(TooltipHarness);
		await tick();

		await userEvent.hover(screen.getByTestId('tooltip-reference'));
		await tick();

		expect(screen.queryByText('Sort inventory')).toBeNull();
	});

	it('toggles on tap with a coarse pointer', async () => {
		setPointer(true);
		render(TooltipHarness);
		await tick();

		const reference = screen.getByTestId('tooltip-reference');
		await userEvent.click(reference);
		await tick();
		expect(screen.getByText('Sort inventory')).not.toBeNull();

		await userEvent.click(reference);
		await tick();
		expect(screen.queryByText('Sort inventory')).toBeNull();
	});

	it('puts role="tooltip" on the popup, not the trigger wrapper', async () => {
		setPointer(false);
		render(TooltipHarness);
		await tick();

		const reference = screen.getByTestId('tooltip-reference');
		expect(reference.closest('[role="tooltip"]')).toBeNull();

		await userEvent.hover(reference);
		await tick();

		const popup = screen.getByText('Sort inventory');
		expect(popup.getAttribute('role')).toBe('tooltip');
	});

	it('still fires the child button click under a coarse pointer', async () => {
		setPointer(true);
		render(TooltipClickThroughHarness);
		await tick();

		const reference = screen.getByTestId('tooltip-reference');
		await userEvent.click(reference);
		await tick();

		expect(screen.getByTestId('click-count').textContent).toBe('1');
		expect(screen.getByText('Sort inventory')).not.toBeNull();
	});

	it('dismisses a coarse-pointer tooltip on tap outside', async () => {
		setPointer(true);
		render(TooltipMultiHarness);
		await tick();

		const referenceA = screen.getByTestId('tooltip-reference-a');
		await userEvent.click(referenceA);
		await tick();
		expect(screen.getByText('Sort inventory')).not.toBeNull();

		await userEvent.click(screen.getByTestId('outside'));
		await tick();
		expect(screen.queryByText('Sort inventory')).toBeNull();
	});

	it('closes the first coarse-pointer tooltip when a different one opens', async () => {
		setPointer(true);
		render(TooltipMultiHarness);
		await tick();

		const referenceA = screen.getByTestId('tooltip-reference-a');
		const referenceB = screen.getByTestId('tooltip-reference-b');

		await userEvent.click(referenceA);
		await tick();
		expect(screen.getByText('Sort inventory')).not.toBeNull();

		await userEvent.click(referenceB);
		await tick();
		expect(screen.queryByText('Sort inventory')).toBeNull();
		expect(screen.getByText('Filter inventory')).not.toBeNull();
	});

	it('closes on blur with a coarse pointer', async () => {
		setPointer(true);
		render(TooltipHarness);
		await tick();

		const reference = screen.getByTestId('tooltip-reference');
		reference.focus();
		await tick();
		expect(screen.getByText('Sort inventory')).not.toBeNull();

		reference.blur();
		await tick();
		expect(screen.queryByText('Sort inventory')).toBeNull();
	});

	it('toggles on keyboard activation with a coarse pointer, twice', async () => {
		setPointer(true);
		render(TooltipHarness);
		await tick();

		const reference = screen.getByTestId('tooltip-reference');

		reference.focus();
		await tick();
		expect(screen.getByText('Sort inventory')).not.toBeNull();

		// `fireEvent.click` skips `pointerdown`, standing in for keyboard activation.
		await fireEvent.click(reference);
		await tick();
		expect(screen.queryByText('Sort inventory')).toBeNull();

		await fireEvent.click(reference);
		await tick();
		expect(screen.getByText('Sort inventory')).not.toBeNull();
	});
});
