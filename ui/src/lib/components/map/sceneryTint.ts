// Loads the map raster already shipped for the 2D map into a mosaic the scenery
// shader samples per fragment -- no new art.
import * as THREE from 'three';
import { MAP_TILE_DIR, type MapArea } from './utils';

export const TINT_ZOOM = 3;
export const TINT_TILE_PX = 512;
export const TINT_MOSAIC_PX = TINT_TILE_PX * 2 ** TINT_ZOOM; // 8x8 tiles * 512, the full zoom-3 grid

const TILES_PER_SIDE = 2 ** TINT_ZOOM;

export type TintMosaic = { data: Uint8ClampedArray; size: number };

// mercX/mercY are MercatorCoordinate's [0,1] world space, the same space
// sceneryInstanceMatrix writes into, so callers pass those straight through.
// Clamped per axis so an out-of-range coordinate samples the nearest edge rather
// than wrapping onto the opposite one.
export function tintOffsetAt(mercX: number, mercY: number, size: number): number {
	const col = Math.min(size - 1, Math.max(0, Math.floor(mercX * size)));
	const row = Math.min(size - 1, Math.max(0, Math.floor(mercY * size)));
	return (row * size + col) * 4;
}

export function sampleTint(
	mosaic: TintMosaic,
	mercX: number,
	mercY: number
): { r: number; g: number; b: number } | null {
	const offset = tintOffsetAt(mercX, mercY, mosaic.size);
	if (mosaic.data[offset + 3] === 0) return null;
	return {
		r: mosaic.data[offset] / 255,
		g: mosaic.data[offset + 1] / 255,
		b: mosaic.data[offset + 2] / 255
	};
}

// flipY stays false so texel row 0 maps to vMapUv.y = 0, and colorSpace stays
// NoColorSpace so the raster's sRGB bytes reach the shader's arithmetic unconverted.
export function mosaicTexture(mosaic: TintMosaic): THREE.DataTexture {
	const texture = new THREE.DataTexture(mosaic.data, mosaic.size, mosaic.size, THREE.RGBAFormat);
	texture.flipY = false;
	texture.colorSpace = THREE.NoColorSpace;
	texture.magFilter = THREE.LinearFilter;
	texture.minFilter = THREE.LinearMipmapLinearFilter;
	texture.generateMipmaps = true;
	texture.wrapS = THREE.ClampToEdgeWrapping;
	texture.wrapT = THREE.ClampToEdgeWrapping;
	texture.anisotropy = 8;
	texture.needsUpdate = true;
	return texture;
}

// Module scope, so every rebuild and area revisit reuses the one fetch+composite.
const mosaicCache = new Map<MapArea, Promise<TintMosaic>>();

export function loadTintMosaic(area: MapArea): Promise<TintMosaic> {
	const cached = mosaicCache.get(area);
	if (cached) return cached;

	const dir = MAP_TILE_DIR[area];
	const promise = (async (): Promise<TintMosaic> => {
		const canvas = new OffscreenCanvas(TINT_MOSAIC_PX, TINT_MOSAIC_PX);
		const ctx = canvas.getContext('2d')!;

		const draws: Promise<void>[] = [];
		for (let x = 0; x < TILES_PER_SIDE; x++) {
			for (let y = 0; y < TILES_PER_SIDE; y++) {
				const dx = x * TINT_TILE_PX;
				const dy = y * TINT_TILE_PX;
				draws.push(
					fetch(`/maps/${dir}/${TINT_ZOOM}/${x}/${y}.webp`)
						.then((res) => (res.ok ? res.blob() : Promise.reject(new Error(String(res.status)))))
						.then((blob) => createImageBitmap(blob))
						.then((bitmap) => {
							ctx.drawImage(bitmap, dx, dy);
							bitmap.close();
						})
						// A missing tile leaves its region transparent, so those instances
						// fall back to the base colour instead of the mosaic rejecting.
						.catch(() => {})
				);
			}
		}
		await Promise.all(draws);

		const { data } = ctx.getImageData(0, 0, TINT_MOSAIC_PX, TINT_MOSAIC_PX);
		return { data, size: TINT_MOSAIC_PX };
	})();

	mosaicCache.set(area, promise);
	return promise;
}
