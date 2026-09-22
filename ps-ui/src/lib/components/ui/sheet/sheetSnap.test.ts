import { describe, expect, it } from 'vitest';

import { resolveDrag, SHEET_SNAP_VH, type SheetSnap } from './sheetSnap';

describe('SHEET_SNAP_VH', () => {
	it('orders its stops from smallest to largest', () => {
		expect(SHEET_SNAP_VH.peek).toBeLessThan(SHEET_SNAP_VH.half);
		expect(SHEET_SNAP_VH.half).toBeLessThan(SHEET_SNAP_VH.tall);
	});

	it('leaves what is behind it visible at every stop', () => {
		for (const height of Object.values(SHEET_SNAP_VH)) {
			expect(height).toBeGreaterThan(0);
			expect(height).toBeLessThan(100);
		}
	});
});

describe('resolveDrag', () => {
	const snaps: SheetSnap[] = ['peek', 'half', 'tall'];

	it('ignores a drag under the threshold', () => {
		expect(resolveDrag('half', 10, snaps)).toBe('half');
		expect(resolveDrag('half', -10, snaps)).toBe('half');
	});

	it('steps up one stop on an upward drag', () => {
		expect(resolveDrag('peek', -80, snaps)).toBe('half');
		expect(resolveDrag('half', -80, snaps)).toBe('tall');
	});

	it('stops at the tallest stop', () => {
		expect(resolveDrag('tall', -80, snaps)).toBe('tall');
	});

	it('steps down one stop on a downward drag', () => {
		expect(resolveDrag('tall', 80, snaps)).toBe('half');
		expect(resolveDrag('half', 80, snaps)).toBe('peek');
	});

	it('closes when dragged down from the smallest stop', () => {
		expect(resolveDrag('peek', 80, snaps)).toBe('closed');
	});

	it('honours a custom threshold', () => {
		expect(resolveDrag('tall', 20, snaps)).toBe('tall');
		expect(resolveDrag('tall', 20, snaps, 10)).toBe('half');
	});

	it('honours a restricted stop list', () => {
		const twoStop: SheetSnap[] = ['peek', 'tall'];
		expect(resolveDrag('peek', -80, twoStop)).toBe('tall');
		expect(resolveDrag('tall', 80, twoStop)).toBe('peek');
		expect(resolveDrag('peek', 80, twoStop)).toBe('closed');
	});
});
