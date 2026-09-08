import { describe, expect, it } from 'vitest';
import { clampToViewport, cornerOrigin, nearestCorner, type PipRect } from './pipSnap';

const VIEWPORT = { width: 1000, height: 800 };

function rectAt(x: number, y: number, width = 320, height = 240): PipRect {
	return { x, y, width, height };
}

describe('nearestCorner', () => {
	it('picks top-left when the centre sits in the top-left quadrant', () => {
		expect(nearestCorner(rectAt(0, 0), VIEWPORT.width, VIEWPORT.height)).toBe('top-left');
	});

	it('picks top-right when the centre sits in the top-right quadrant', () => {
		expect(nearestCorner(rectAt(700, 0), VIEWPORT.width, VIEWPORT.height)).toBe('top-right');
	});

	it('picks bottom-left when the centre sits in the bottom-left quadrant', () => {
		expect(nearestCorner(rectAt(0, 600), VIEWPORT.width, VIEWPORT.height)).toBe('bottom-left');
	});

	it('picks bottom-right when the centre sits in the bottom-right quadrant', () => {
		expect(nearestCorner(rectAt(700, 600), VIEWPORT.width, VIEWPORT.height)).toBe('bottom-right');
	});

	it('breaks a dead-centre tie toward top-left', () => {
		const rect = rectAt(VIEWPORT.width / 2 - 160, VIEWPORT.height / 2 - 120);
		expect(nearestCorner(rect, VIEWPORT.width, VIEWPORT.height)).toBe('top-left');
	});
});

describe('cornerOrigin', () => {
	it('anchors each corner with the margin applied on both axes', () => {
		expect(cornerOrigin('top-left', 320, 240, 1000, 800, 12)).toEqual({ x: 12, y: 12 });
		expect(cornerOrigin('top-right', 320, 240, 1000, 800, 12)).toEqual({ x: 668, y: 12 });
		expect(cornerOrigin('bottom-left', 320, 240, 1000, 800, 12)).toEqual({ x: 12, y: 548 });
		expect(cornerOrigin('bottom-right', 320, 240, 1000, 800, 12)).toEqual({ x: 668, y: 548 });
	});
});

describe('clampToViewport', () => {
	it('leaves an already-inbounds rect untouched', () => {
		expect(clampToViewport(rectAt(100, 100), 1000, 800)).toEqual(rectAt(100, 100));
	});

	it('returns the same object when nothing changes, so reactive callers see no write', () => {
		const rect = rectAt(100, 100);
		expect(clampToViewport(rect, 1000, 800)).toBe(rect);
	});

	it('pulls a negative origin back to the edge', () => {
		expect(clampToViewport(rectAt(-50, -50), 1000, 800)).toEqual(rectAt(0, 0));
	});

	it('pulls an overshooting origin back so the panel stays fully visible', () => {
		expect(clampToViewport(rectAt(900, 700), 1000, 800)).toEqual(rectAt(680, 560));
	});

	it('shrinks a panel wider or taller than the viewport instead of clamping to a negative position', () => {
		expect(clampToViewport(rectAt(0, 0, 1200, 900), 1000, 800)).toEqual(rectAt(0, 0, 1000, 800));
	});

	it('floors width and height to the given minimums', () => {
		expect(clampToViewport(rectAt(0, 0, 100, 80), 1000, 800, 240, 160)).toEqual(
			rectAt(0, 0, 240, 160)
		);
	});

	it('lets the viewport cap win when it is smaller than the minimum', () => {
		expect(clampToViewport(rectAt(0, 0, 100, 80), 200, 150, 240, 160)).toEqual(
			rectAt(0, 0, 200, 150)
		);
	});
});
