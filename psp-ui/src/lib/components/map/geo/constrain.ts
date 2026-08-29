import { LngLat, MercatorCoordinate } from 'maplibre-gl';

export type ConstrainTransform = {
	minZoom: number;
	maxZoom: number;
	width: number;
	height: number;
	tileSize: number;
	latRange: [number, number] | null;
};

const clamp = (v: number, a: number, b: number) => Math.min(Math.max(v, a), b);

export function worldFittingConstrain(tf: ConstrainTransform) {
	return (lngLat: LngLat, zoom: number): { center: LngLat; zoom: number } => {
		let z = clamp(zoom, tf.minZoom, tf.maxZoom);
		let worldSize = tf.tileSize * Math.pow(2, z);
		const sw = tf.width;
		const sh = tf.height;
		const mc = MercatorCoordinate.fromLngLat(new LngLat(lngLat.lng, lngLat.lat));
		let x = mc.x * worldSize;
		let y = mc.y * worldSize;
		const latRange = tf.latRange;
		if (latRange) {
			let minY = MercatorCoordinate.fromLngLat(new LngLat(0, latRange[1])).y * worldSize;
			let maxY = MercatorCoordinate.fromLngLat(new LngLat(0, latRange[0])).y * worldSize;
			if (maxY - minY < sh) {
				z = z + Math.log2(sh / (maxY - minY));
				worldSize = tf.tileSize * Math.pow(2, z);
				x = mc.x * worldSize;
				minY = MercatorCoordinate.fromLngLat(new LngLat(0, latRange[1])).y * worldSize;
				maxY = MercatorCoordinate.fromLngLat(new LngLat(0, latRange[0])).y * worldSize;
				y = (minY + maxY) / 2;
			} else {
				const h2 = sh / 2;
				if (y - h2 < minY) y = minY + h2;
				if (y + h2 > maxY) y = maxY - h2;
			}
		}
		if (worldSize < sw) {
			x = worldSize / 2;
		} else {
			const w2 = sw / 2;
			if (x - w2 < 0) x = w2;
			if (x + w2 > worldSize) x = worldSize - w2;
		}
		return {
			center: new MercatorCoordinate(x / worldSize, y / worldSize).toLngLat().wrap(),
			zoom: z
		};
	};
}
