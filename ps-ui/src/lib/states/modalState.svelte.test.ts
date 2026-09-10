import { describe, expect, it } from 'vitest';
import type { Component } from 'svelte';
import { getModalState } from './modalState.svelte';

const Outer = (() => {}) as unknown as Component;
const Inner = (() => {}) as unknown as Component;

describe('modalState', () => {
	it('resolves a single modal with the value it was closed with', async () => {
		const modal = getModalState();
		const pending = modal.showModal<string>(Outer, { a: 1 });
		expect(modal.isOpen).toBe(true);
		expect(modal.component).toBe(Outer);
		expect(modal.props).toEqual({ a: 1 });

		modal.closeModal('done');
		await expect(pending).resolves.toBe('done');
		expect(modal.isOpen).toBe(false);
	});

	it('keeps the outer modal open when an inner one closes', async () => {
		const modal = getModalState();
		let outerResolved: unknown = 'not-yet';
		const outer = modal.showModal<string>(Outer, { a: 1 }).then((value) => {
			outerResolved = value;
			return value;
		});

		const inner = modal.showModal<number>(Inner, { b: 2 });
		expect(modal.component).toBe(Inner);

		modal.closeModal(7);
		await expect(inner).resolves.toBe(7);

		expect(modal.isOpen).toBe(true);
		expect(modal.component).toBe(Outer);
		expect(modal.props).toEqual({ a: 1 });
		expect(outerResolved).toBe('not-yet');

		modal.closeModal('outer');
		await expect(outer).resolves.toBe('outer');
		expect(modal.isOpen).toBe(false);
	});

	it('exposes every open modal so a stacked one stays mounted', () => {
		const modal = getModalState();
		modal.showModal(Outer, { a: 1 });
		modal.showModal(Inner, { b: 2 });

		expect(modal.stack.map((entry) => entry.component)).toEqual([Outer, Inner]);
		expect(modal.stack.map((entry) => entry.props)).toEqual([{ a: 1 }, { b: 2 }]);

		modal.closeModal();
		modal.closeModal();
		expect(modal.stack).toEqual([]);
	});

	it('closing an entry below the top resolves the ones above it too', async () => {
		const modal = getModalState();
		const outer = modal.showModal<string>(Outer);
		const inner = modal.showModal<string>(Inner);
		const [bottom] = modal.stack;

		modal.closeEntry(bottom.id, 'bottom');

		await expect(inner).resolves.toBeUndefined();
		await expect(outer).resolves.toBe('bottom');
		expect(modal.isOpen).toBe(false);
	});
});
