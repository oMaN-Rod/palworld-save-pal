import type { ExpressionSpecification } from 'maplibre-gl';
import { describe, expect, it } from 'vitest';
import {
	HALO_PAD,
	ICON_SCALE,
	ICON_ZOOM_MAX,
	ICON_ZOOM_MIN,
	haloRadiusPx,
	zoomScaledIconSize,
	zoomScaledRadius
} from './expressions';

const build = (...args: Parameters<typeof zoomScaledIconSize>): unknown[] =>
	zoomScaledIconSize(...args) as unknown as unknown[];

function findPaths(node: unknown, head: string, path: string[] = []): string[][] {
	if (!Array.isArray(node)) return [];
	if (node[0] === head) return [path];
	return node.flatMap((child, i) => findPaths(child, head, [...path, String(i)]));
}

describe('zoomScaledIconSize', () => {
	// icon-size is a LAYOUT property, and MapLibre's validate_expression rejects any
	// layout value whose expression is not state-constant -- a feature-state reference
	// anywhere inside it fails the whole layer at addLayer, removing every marker.
	it('contains no feature-state reference', () => {
		expect(findPaths(build(0.6, 1.0), 'feature-state')).toEqual([]);
	});

	it('contains no feature-state even when stops are data expressions', () => {
		const watchtower: ExpressionSpecification = ['case', ['get', 'watchtower'], 0.36, 0.45];
		const expr = build(watchtower, ['case', ['get', 'watchtower'], 0.6, 0.75]);
		expect(findPaths(expr, 'feature-state')).toEqual([]);
	});

	// A zoom expression must be the input to a TOP-LEVEL step/interpolate; nesting it
	// inside another operator is a separate validation failure with the same blast radius.
	it('puts interpolate at the top level', () => {
		expect(build(0.6, 1.0)[0]).toBe('interpolate');
	});

	it('feeds zoom directly into the top-level interpolate', () => {
		expect(build(0.6, 1.0)[2]).toEqual(['zoom']);
		expect(findPaths(build(0.6, 1.0), 'zoom')).toEqual([['2']]);
	});

	it('interpolates between the two zoom stops', () => {
		const expr = build(0.6, 1.0);
		expect(expr[3]).toBe(ICON_ZOOM_MIN);
		expect(expr[4]).toBeCloseTo(0.6 * ICON_SCALE);
		expect(expr[5]).toBe(ICON_ZOOM_MAX);
		expect(expr[6]).toBeCloseTo(1.0 * ICON_SCALE);
	});

	it('wraps data expressions in a scale multiply', () => {
		const watchtower: ExpressionSpecification = ['case', ['get', 'watchtower'], 0.36, 0.45];
		const expr = build(watchtower, ['case', ['get', 'watchtower'], 0.6, 0.75]);
		expect(expr[4]).toEqual(['*', watchtower, ICON_SCALE]);
	});

	it('scales every marker by the same factor', () => {
		const relic = build(0.4, 0.6);
		const player = build(0.6, 1.0);
		expect((relic[4] as number) / 0.4).toBeCloseTo((player[4] as number) / 0.6);
	});
});

describe('haloRadiusPx', () => {
	it('derives a radius from rendered pixels, not icon-size', () => {
		expect(haloRadiusPx(48, 0.4)).toBeCloseTo((48 * 0.4 * ICON_SCALE) / 2 + HALO_PAD);
		expect(haloRadiusPx(64, 0.75)).toBeCloseTo((64 * 0.75 * ICON_SCALE) / 2 + HALO_PAD);
	});

	// A watchtower's icon-size is the smallest on the map but its source art is the
	// largest, so an icon-size-only radius would draw its ring inside the icon.
	it('gives a watchtower a larger halo than a relic despite a smaller icon-size', () => {
		expect(haloRadiusPx(100, 0.36)).toBeGreaterThan(haloRadiusPx(48, 0.4));
	});

	it('grows with zoom for a single marker', () => {
		expect(haloRadiusPx(48, 0.6)).toBeGreaterThan(haloRadiusPx(48, 0.4));
	});
});

describe('zoomScaledRadius', () => {
	const buildRadius = (...args: Parameters<typeof zoomScaledRadius>): unknown[] =>
		zoomScaledRadius(...args) as unknown as unknown[];

	it('puts interpolate at the top level', () => {
		expect(buildRadius(10, 20)[0]).toBe('interpolate');
	});

	it('feeds zoom directly into the top-level interpolate', () => {
		expect(findPaths(buildRadius(10, 20), 'zoom')).toEqual([['2']]);
	});

	// haloRadiusPx has already applied ICON_SCALE; scaling again here would push the
	// ring off the icon by 15% and the error would compound with every size change.
	it('does not re-apply ICON_SCALE to already-scaled radii', () => {
		const expr = buildRadius(10, 20);
		expect(expr[3]).toBe(ICON_ZOOM_MIN);
		expect(expr[4]).toBe(10);
		expect(expr[5]).toBe(ICON_ZOOM_MAX);
		expect(expr[6]).toBe(20);
	});

	it('passes per-class case expressions through as stops', () => {
		const perClass: ExpressionSpecification = ['case', ['get', 'watchtower'], 23.7, 19.6];
		expect(buildRadius(perClass, 30.6)[4]).toEqual(perClass);
	});
});
