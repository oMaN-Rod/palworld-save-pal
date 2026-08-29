// Bottom to top. The two beam-bearing layers come last because a beam is
// additive and writes no depth, so any opaque layer drawn after one repaints
// it. Anchoring each layer to its nearest mounted successor rather than to a
// single fixed id is what makes the stacking independent of arrival order --
// these layers mount as their data streams in, not in a fixed sequence.
export const LAYER_ORDER_3D = [
	'structure-3d',
	'scenery-3d',
	'blueprint-ghost',
	'map-objects-3d',
	'pals-3d'
] as const;

export type Layer3dId = (typeof LAYER_ORDER_3D)[number];

export const FALLBACK_BEFORE_ID = 'origin-icons';

export function beforeIdFor(layerId: Layer3dId, mounted: readonly string[]): string {
	const index = LAYER_ORDER_3D.indexOf(layerId);
	for (let i = index + 1; i < LAYER_ORDER_3D.length; i++) {
		if (mounted.includes(LAYER_ORDER_3D[i])) return LAYER_ORDER_3D[i];
	}
	return FALLBACK_BEFORE_ID;
}
