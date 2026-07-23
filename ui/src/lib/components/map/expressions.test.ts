import { describe, expect, it } from 'vitest';
import type { ExpressionSpecification } from 'maplibre-gl';
import { ICON_ZOOM_MAX, ICON_ZOOM_MIN, zoomScaledIconSize } from './expressions';

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
		expect(expr[4]).toBe(0.6);
		expect(expr[5]).toBe(ICON_ZOOM_MAX);
		expect(expr[6]).toBe(1.0);
	});

	it('passes data expressions through as stop values', () => {
		const watchtower: ExpressionSpecification = ['case', ['get', 'watchtower'], 0.36, 0.45];
		const expr = build(watchtower, ['case', ['get', 'watchtower'], 0.6, 0.75]);
		expect(expr[4]).toEqual(watchtower);
	});
});
