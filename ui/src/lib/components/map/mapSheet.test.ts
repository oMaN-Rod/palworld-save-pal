import { describe, expect, it } from 'vitest';
import { SHEET_SNAP_VH, resolveDrag, type SheetSnap } from './mapSheet';

describe('SHEET_SNAP_VH', () => {
	it('leaves the map visible at every snap', () => {
		for (const height of Object.values(SHEET_SNAP_VH)) {
			expect(height).toBeGreaterThan(0);
			expect(height).toBeLessThan(100);
		}
	});

	it('makes tall taller than peek', () => {
		expect(SHEET_SNAP_VH.tall).toBeGreaterThan(SHEET_SNAP_VH.peek);
	});
});

describe('resolveDrag', () => {
	const snaps: SheetSnap[] = ['peek', 'tall'];

	it('holds the current snap inside the threshold', () => {
		for (const snap of snaps) {
			expect(resolveDrag(snap, 0)).toBe(snap);
			expect(resolveDrag(snap, 47)).toBe(snap);
			expect(resolveDrag(snap, -47)).toBe(snap);
		}
	});

	it('collapses tall to peek on a downward drag', () => {
		expect(resolveDrag('tall', 60)).toBe('peek');
	});

	it('dismisses from peek on a downward drag', () => {
		expect(resolveDrag('peek', 60)).toBe('closed');
	});

	it('expands peek to tall on an upward drag', () => {
		expect(resolveDrag('peek', -60)).toBe('tall');
	});

	it('keeps tall pinned when dragged further up', () => {
		expect(resolveDrag('tall', -200)).toBe('tall');
	});

	it('honours a custom threshold', () => {
		expect(resolveDrag('tall', 20)).toBe('tall');
		expect(resolveDrag('tall', 20, 10)).toBe('peek');
	});
});
