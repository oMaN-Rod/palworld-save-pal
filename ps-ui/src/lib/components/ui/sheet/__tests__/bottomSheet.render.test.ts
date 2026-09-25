// @vitest-environment jsdom
import '$utils/__tests__/fixtures/animatePolyfill';
import { render, screen } from '@testing-library/svelte';
import userEvent from '@testing-library/user-event';
import { tick } from 'svelte';
import { describe, expect, it, vi } from 'vitest';

import { SHEET_SNAP_VH } from '../sheetSnap';
import BottomSheetHarness from './BottomSheetHarness.svelte';

function pointerEvent(type: string, clientY: number): Event {
	const event = new Event(type, { bubbles: true, cancelable: true });
	Object.defineProperty(event, 'clientY', { value: clientY });
	Object.defineProperty(event, 'pointerId', { value: 1 });
	return event;
}

function clickEvent(): Event {
	return new MouseEvent('click', { bubbles: true, cancelable: true });
}

function getHandle(): HTMLElement {
	const handle = screen.getByRole('button', { name: /resize sheet/i });
	handle.setPointerCapture = vi.fn();
	handle.releasePointerCapture = vi.fn();
	return handle;
}

function getDialog(): HTMLElement {
	return screen.getByRole('dialog', { name: 'Map layers' });
}

describe('BottomSheet', () => {
	it('renders nothing while closed', () => {
		render(BottomSheetHarness, { open: false, onClose: vi.fn() });
		expect(screen.queryByRole('dialog')).toBeNull();
	});

	it('renders its title and content when open', async () => {
		render(BottomSheetHarness, { open: true, onClose: vi.fn() });
		await tick();

		expect(screen.getByRole('dialog')).not.toBeNull();
		expect(screen.getByText('Map layers')).not.toBeNull();
		expect(screen.getByText('sheet body')).not.toBeNull();
	});

	it('labels the dialog with its title', async () => {
		render(BottomSheetHarness, { open: true, onClose: vi.fn() });
		await tick();

		expect(screen.getByRole('dialog', { name: 'Map layers' })).not.toBeNull();
	});

	it('closes on Escape', async () => {
		const onClose = vi.fn();
		render(BottomSheetHarness, { open: true, onClose });
		await tick();

		await userEvent.keyboard('{Escape}');

		expect(onClose).toHaveBeenCalledTimes(1);
	});

	it('closes when the backdrop is clicked', async () => {
		const onClose = vi.fn();
		render(BottomSheetHarness, { open: true, onClose });
		await tick();

		await userEvent.click(screen.getByTestId('sheet-backdrop'));

		expect(onClose).toHaveBeenCalledTimes(1);
	});

	it('gives the drag handle a keyboard-operable control', async () => {
		render(BottomSheetHarness, { open: true, onClose: vi.fn() });
		await tick();

		const handle = screen.getByRole('button', { name: /resize sheet/i });
		expect(handle.tagName).toBe('BUTTON');
	});

	it('moves focus into the dialog when opened', async () => {
		render(BottomSheetHarness, { open: true, onClose: vi.fn() });
		await tick();

		const dialog = getDialog();
		expect(dialog.contains(document.activeElement)).toBe(true);
	});

	describe('drag handle', () => {
		it('steps up one stop on an upward drag', async () => {
			render(BottomSheetHarness, { open: true, onClose: vi.fn() });
			await tick();

			const handle = getHandle();
			handle.dispatchEvent(pointerEvent('pointerdown', 300));
			handle.dispatchEvent(pointerEvent('pointerup', 200));
			await tick();

			expect(getDialog().style.height).toBe(`${SHEET_SNAP_VH.tall}vh`);
		});

		it('steps down one stop on a downward drag', async () => {
			render(BottomSheetHarness, { open: true, snap: 'tall', onClose: vi.fn() });
			await tick();

			const handle = getHandle();
			handle.dispatchEvent(pointerEvent('pointerdown', 100));
			handle.dispatchEvent(pointerEvent('pointerup', 200));
			await tick();

			expect(getDialog().style.height).toBe(`${SHEET_SNAP_VH.peek}vh`);
		});

		it('closes when dragged down from the smallest stop', async () => {
			const onClose = vi.fn();
			render(BottomSheetHarness, { open: true, onClose });
			await tick();

			const handle = getHandle();
			handle.dispatchEvent(pointerEvent('pointerdown', 100));
			handle.dispatchEvent(pointerEvent('pointerup', 200));
			await tick();

			expect(onClose).toHaveBeenCalledTimes(1);
		});

		it('leaves the snap unchanged and does not toggle on a sub-threshold drag', async () => {
			render(BottomSheetHarness, { open: true, onClose: vi.fn() });
			await tick();

			const handle = getHandle();
			handle.dispatchEvent(pointerEvent('pointerdown', 100));
			handle.dispatchEvent(pointerEvent('pointerup', 80));
			handle.dispatchEvent(clickEvent());
			await tick();

			expect(getDialog().style.height).toBe(`${SHEET_SNAP_VH.peek}vh`);
		});

		it('leaves the snap unchanged and does not toggle on an upward drag at the tallest stop', async () => {
			render(BottomSheetHarness, { open: true, snap: 'tall', onClose: vi.fn() });
			await tick();

			const handle = getHandle();
			handle.dispatchEvent(pointerEvent('pointerdown', 300));
			handle.dispatchEvent(pointerEvent('pointerup', 200));
			handle.dispatchEvent(clickEvent());
			await tick();

			expect(getDialog().style.height).toBe(`${SHEET_SNAP_VH.tall}vh`);
		});

		it('cycles the snap on a genuine tap, wrapping back to the smallest stop', async () => {
			render(BottomSheetHarness, { open: true, onClose: vi.fn() });
			await tick();

			const handle = getHandle();
			handle.dispatchEvent(pointerEvent('pointerdown', 150));
			handle.dispatchEvent(pointerEvent('pointerup', 150));
			handle.dispatchEvent(clickEvent());
			await tick();

			expect(getDialog().style.height).toBe(`${SHEET_SNAP_VH.tall}vh`);

			handle.dispatchEvent(pointerEvent('pointerdown', 150));
			handle.dispatchEvent(pointerEvent('pointerup', 150));
			handle.dispatchEvent(clickEvent());
			await tick();

			expect(getDialog().style.height).toBe(`${SHEET_SNAP_VH.peek}vh`);
		});
	});
});

describe('BottomSheet as a non-modal sheet', () => {
	it('drops the backdrop and the modal flag', async () => {
		render(BottomSheetHarness, { props: { open: true, modal: false, onClose: vi.fn() } });
		await tick();

		expect(screen.queryByTestId('sheet-backdrop')).toBeNull();
		expect(getDialog().getAttribute('aria-modal')).toBeNull();
	});

	it('carries its own close button instead', async () => {
		const onClose = vi.fn();
		render(BottomSheetHarness, { props: { open: true, modal: false, onClose } });
		await tick();

		await userEvent.click(screen.getByRole('button', { name: 'Close' }));

		expect(onClose).toHaveBeenCalledTimes(1);
	});

	it('leaves the close button to the backdrop when it is modal', async () => {
		render(BottomSheetHarness, { props: { open: true, onClose: vi.fn() } });
		await tick();

		expect(screen.getByTestId('sheet-backdrop')).toBeTruthy();
		expect(screen.queryByRole('button', { name: 'Close' })).toBeNull();
	});

	it('still resizes from the handle', async () => {
		render(BottomSheetHarness, { props: { open: true, modal: false, onClose: vi.fn() } });
		await tick();

		const handle = getHandle();
		handle.dispatchEvent(pointerEvent('pointerdown', 400));
		handle.dispatchEvent(pointerEvent('pointerup', 300));
		await tick();

		expect(getDialog().style.height).toBe(`${SHEET_SNAP_VH.tall}vh`);
	});
});
