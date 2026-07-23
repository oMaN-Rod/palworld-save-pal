export const DEFAULT_FOOTPRINT = { sx: 100, sy: 100, sz: 100, ox: 0, oy: 0, oz: 0 };

const COLLISION_NAME = 'CheckOverlapCollision_GEN_VARIABLE';

export function parseBoxComponent(exports) {
	const component = exports.find(
		(entry) => entry.Type === 'BoxComponent' && entry.Name === COLLISION_NAME
	);
	if (!component) return null;

	const extent = component.Properties?.BoxExtent;
	if (!extent) return { box: { ...DEFAULT_FOOTPRINT }, defaulted: true };

	const offset = component.Properties.RelativeLocation ?? { X: 0, Y: 0, Z: 0 };
	return {
		box: {
			sx: extent.X * 2,
			sy: extent.Y * 2,
			sz: extent.Z * 2,
			ox: offset.X,
			oy: offset.Y,
			oz: offset.Z
		},
		defaulted: false
	};
}

export function blueprintStemFromAssetPath(assetPathName) {
	if (!assetPathName || assetPathName === 'None') return null;
	const withoutClass = assetPathName.split('.')[0];
	return withoutClass.slice(withoutClass.lastIndexOf('/') + 1) || null;
}
