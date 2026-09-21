// @vitest-environment jsdom
import { fireEvent, render, screen, within } from '@testing-library/svelte';
import userEvent from '@testing-library/user-event';
import { createRawSnippet } from 'svelte';
import { afterEach, describe, expect, it, vi } from 'vitest';

import PalListComponent from '../PalList.svelte';

interface TestPal {
	id: number;
	nickname: string;
}

const PalList = PalListComponent<TestPal, number>;

function makePals(): TestPal[] {
	return [
		{ id: 1, nickname: 'Sparky' },
		{ id: 2, nickname: 'Boltbeak' },
		{ id: 3, nickname: 'Icefang' }
	];
}

function portraitSnippet() {
	return createRawSnippet((getPal: () => TestPal) => ({
		render: () => `<span data-testid="portrait-${getPal().id}">portrait</span>`
	}));
}

function columnsSnippet() {
	return createRawSnippet((getPal: () => TestPal) => ({
		render: () => `<span data-testid="columns-${getPal().id}">HP ${getPal().id}00</span>`
	}));
}

interface PalListTestProps {
	pals: TestPal[];
	idOf: (pal: TestPal) => number;
	nicknameOf: (pal: TestPal) => string;
	selectedIds: number[];
	onSelect: (pal: TestPal) => void;
	onLongPress: (pal: TestPal) => void;
	portrait: ReturnType<typeof portraitSnippet>;
	columns: ReturnType<typeof columnsSnippet>;
}

function baseProps(overrides: Partial<PalListTestProps> = {}): PalListTestProps {
	return {
		pals: makePals(),
		idOf: (pal: TestPal) => pal.id,
		nicknameOf: (pal: TestPal) => pal.nickname,
		selectedIds: [],
		onSelect: vi.fn(),
		onLongPress: vi.fn(),
		portrait: portraitSnippet(),
		columns: columnsSnippet(),
		...overrides
	};
}

function pointerEvent(type: string, x = 10, y = 10): Event {
	const event = new Event(type, { bubbles: true, cancelable: true });
	Object.defineProperty(event, 'clientX', { value: x });
	Object.defineProperty(event, 'clientY', { value: y });
	Object.defineProperty(event, 'pointerId', { value: 1 });
	return event;
}

describe('PalList', () => {
	afterEach(() => {
		vi.useRealTimers();
	});

	it('renders one activatable row per pal, named by the pal nickname', () => {
		render(PalList, baseProps());

		const rows = screen.getAllByRole('button');
		expect(rows).toHaveLength(3);
		expect(screen.getByRole('button', { name: 'Sparky' })).not.toBeNull();
		expect(screen.getByRole('button', { name: 'Boltbeak' })).not.toBeNull();
		expect(screen.getByRole('button', { name: 'Icefang' })).not.toBeNull();
		rows.forEach((row) => expect(row.tagName).toBe('BUTTON'));
	});

	it('carries the selection state as a suffix on the accessible name, and only on selected rows', () => {
		render(PalList, baseProps({ selectedIds: [2] }));

		expect(screen.getByRole('button', { name: 'Sparky' })).not.toBeNull();
		expect(screen.getByRole('button', { name: 'Boltbeak, Selected' })).not.toBeNull();
		expect(screen.getByRole('button', { name: 'Icefang' })).not.toBeNull();
		expect(screen.queryByRole('button', { name: 'Boltbeak' })).toBeNull();
		expect(screen.queryByRole('button', { name: 'Sparky, Selected' })).toBeNull();
	});

	it('updates the accessible-name selection suffix as selectedIds changes', async () => {
		const { rerender } = render(PalList, baseProps({ selectedIds: [] }));
		expect(screen.getByRole('button', { name: 'Sparky' })).not.toBeNull();
		expect(screen.queryByRole('button', { name: 'Sparky, Selected' })).toBeNull();

		await rerender(baseProps({ selectedIds: [1] }));
		expect(screen.getByRole('button', { name: 'Sparky, Selected' })).not.toBeNull();
		expect(screen.queryByRole('button', { name: 'Sparky' })).toBeNull();
	});

	it('renders the columns snippet exactly once per row, matched to the right pal', () => {
		render(PalList, baseProps());

		const sparkyRow = screen.getByRole('button', { name: 'Sparky' }).closest('li')!;
		expect(within(sparkyRow).getAllByTestId('columns-1')).toHaveLength(1);
		expect(within(sparkyRow).queryByTestId('columns-2')).toBeNull();

		const boltbeakRow = screen.getByRole('button', { name: 'Boltbeak' }).closest('li')!;
		expect(within(boltbeakRow).getAllByTestId('columns-2')).toHaveLength(1);

		expect(screen.getAllByTestId(/^columns-/)).toHaveLength(3);
	});

	it('exposes the columns content to the accessibility tree, not just the DOM', () => {
		render(PalList, baseProps());

		const boltbeakStats = screen.getByText('HP 200');
		expect(boltbeakStats.closest('[aria-hidden="true"]')).toBeNull();
	});

	it('renders the portrait snippet exactly once per row, matched to the right pal', () => {
		render(PalList, baseProps());

		const icefangRow = screen.getByRole('button', { name: 'Icefang' }).closest('li')!;
		expect(within(icefangRow).getAllByTestId('portrait-3')).toHaveLength(1);
		expect(within(icefangRow).queryByTestId('portrait-1')).toBeNull();

		expect(screen.getAllByTestId(/^portrait-/)).toHaveLength(3);
	});

	it('fires onSelect exactly once with the right pal on a plain click with no modifier keys', async () => {
		const onSelect = vi.fn();
		render(PalList, baseProps({ onSelect }));

		const row = screen.getByRole('button', { name: 'Boltbeak' });
		await fireEvent.click(row, { ctrlKey: false, metaKey: false, shiftKey: false, altKey: false });

		expect(onSelect).toHaveBeenCalledTimes(1);
		expect(onSelect).toHaveBeenCalledWith({ id: 2, nickname: 'Boltbeak' });
	});

	it('fires onSelect exactly once with the right pal on keyboard activation', async () => {
		const onSelect = vi.fn();
		const user = userEvent.setup();
		render(PalList, baseProps({ onSelect }));

		const row = screen.getByRole('button', { name: 'Icefang' });
		row.focus();
		await user.keyboard('{Enter}');

		expect(onSelect).toHaveBeenCalledTimes(1);
		expect(onSelect).toHaveBeenCalledWith({ id: 3, nickname: 'Icefang' });
	});

	it('fires onLongPress after a 500ms press', () => {
		const onLongPress = vi.fn();
		render(PalList, baseProps({ onLongPress }));
		const row = screen.getByRole('button', { name: 'Sparky' });

		vi.useFakeTimers();
		row.dispatchEvent(pointerEvent('pointerdown'));
		vi.advanceTimersByTime(500);

		expect(onLongPress).toHaveBeenCalledTimes(1);
		expect(onLongPress).toHaveBeenCalledWith({ id: 1, nickname: 'Sparky' });
	});

	it('does not fire onLongPress on a short press', () => {
		const onLongPress = vi.fn();
		render(PalList, baseProps({ onLongPress }));
		const row = screen.getByRole('button', { name: 'Sparky' });

		vi.useFakeTimers();
		row.dispatchEvent(pointerEvent('pointerdown'));
		vi.advanceTimersByTime(200);
		row.dispatchEvent(pointerEvent('pointerup'));
		vi.advanceTimersByTime(500);

		expect(onLongPress).not.toHaveBeenCalled();
	});

	it('every row is a plain real button element, never a div with a click handler', () => {
		const { container } = render(PalList, baseProps());

		const list = container.querySelector('ul');
		expect(list).not.toBeNull();
		const buttons = list!.querySelectorAll('button');
		expect(buttons).toHaveLength(3);
		buttons.forEach((button) => expect(button.getAttribute('role')).toBeNull());
		expect(list!.querySelectorAll('li > div[onclick]')).toHaveLength(0);
	});
});
