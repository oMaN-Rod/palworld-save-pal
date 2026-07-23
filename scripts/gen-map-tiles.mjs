// Slices the 8192x8192 world map textures into an XYZ pyramid for MapLibre.
//
// Usage:
//   bun scripts/gen-map-tiles.mjs
//
// 512px tiles, z0..z4. z4 is 16x16 = 8192px, exactly 1:1 with the source.
// Row 0 is the north edge, matching the XYZ convention MapLibre expects.

import { mkdirSync, rmSync, writeFileSync } from 'node:fs';
import { join } from 'node:path';
import sharp from 'sharp';

const TILE_SIZE = 512;
const MAX_ZOOM = 4;
const SOURCE_SIZE = 8192;

const AREAS = [
	{ dir: 'mainmap', src: 'ui/src/lib/assets/img/t_worldmap.webp' },
	{ dir: 'tree', src: 'ui/src/lib/assets/img/t_treemap.webp' }
];

let total = 0;
let bytes = 0;

for (const { dir, src } of AREAS) {
	const outRoot = join('ui', 'static', 'maps', dir);
	rmSync(outRoot, { recursive: true, force: true });

	const meta = await sharp(src).metadata();
	if (meta.width !== SOURCE_SIZE || meta.height !== SOURCE_SIZE) {
		throw new Error(`${src} is ${meta.width}x${meta.height}, expected ${SOURCE_SIZE} square`);
	}

	for (let z = 0; z <= MAX_ZOOM; z++) {
		const tiles = 2 ** z;
		const scaled = tiles * TILE_SIZE;
		const resized = await sharp(src)
			.resize(scaled, scaled, { kernel: 'lanczos3' })
			.toBuffer();

		for (let x = 0; x < tiles; x++) {
			mkdirSync(join(outRoot, String(z), String(x)), { recursive: true });
			for (let y = 0; y < tiles; y++) {
				const buffer = await sharp(resized)
					.extract({
						left: x * TILE_SIZE,
						top: y * TILE_SIZE,
						width: TILE_SIZE,
						height: TILE_SIZE
					})
					.webp({ quality: 82 })
					.toBuffer();
				writeFileSync(join(outRoot, String(z), String(x), `${y}.webp`), buffer);
				total++;
				bytes += buffer.length;
			}
		}
		console.log(`${dir} z${z}: ${tiles * tiles} tiles`);
	}
}

console.log(`wrote ${total} tiles, ${(bytes / 1024 / 1024).toFixed(2)} MB total`);
