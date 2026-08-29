import { describe, expect, it } from 'vitest';
import { beforeIdFor, FALLBACK_BEFORE_ID, LAYER_ORDER_3D, type Layer3dId } from './layerOrder';

describe('beforeIdFor', () => {
	it('anchors to origin-icons when no later layer is mounted', () => {
		expect(beforeIdFor('structure-3d', [])).toBe(FALLBACK_BEFORE_ID);
		expect(beforeIdFor('pals-3d', ['structure-3d', 'scenery-3d'])).toBe(FALLBACK_BEFORE_ID);
	});

	it('anchors to the nearest later layer, not the last one', () => {
		expect(beforeIdFor('structure-3d', ['scenery-3d', 'pals-3d'])).toBe('scenery-3d');
	});

	it('ignores layers that belong below it', () => {
		expect(beforeIdFor('map-objects-3d', ['structure-3d', 'scenery-3d'])).toBe(FALLBACK_BEFORE_ID);
	});

	it('puts the ghost below both beam layers', () => {
		expect(beforeIdFor('blueprint-ghost', ['map-objects-3d', 'pals-3d'])).toBe('map-objects-3d');
	});

	// The five layers mount asynchronously as their data streams in, so any arrival
	// order must settle to the same stacking. Every permutation is simulated, since
	// one session's order happening to work proves nothing.
	it('settles to the canonical order for every possible mount order', () => {
		const permutations = (items: Layer3dId[]): Layer3dId[][] =>
			items.length <= 1
				? [items]
				: items.flatMap((item, i) =>
						permutations([...items.slice(0, i), ...items.slice(i + 1)]).map((rest) => [item, ...rest])
					);

		for (const arrival of permutations([...LAYER_ORDER_3D])) {
			const style: string[] = [FALLBACK_BEFORE_ID];
			for (const id of arrival) {
				const before = beforeIdFor(id, style);
				style.splice(style.indexOf(before), 0, id);
			}
			expect(style).toEqual([...LAYER_ORDER_3D, FALLBACK_BEFORE_ID]);
		}
	});
});
