// XYZ tile pyramid for MapLibre, sliced from the game's world map textures
// (same export root as gen-relic-icons.mjs).
//
// Row 0 is the north edge, matching the XYZ convention MapLibre expects.

import { mkdirSync, rmSync, writeFileSync } from 'node:fs';
import { join } from 'node:path';
import sharp from 'sharp';

const [, , contentRoot] = process.argv;
if (!contentRoot) {
	console.error('usage: bun scripts/gen-map-tiles.mjs <path-to-Exports/Pal/Content>');
	process.exit(1);
}

const TILE_SIZE = 512;
const MAX_ZOOM = 4;
const SOURCE_SIZE = 8192;

const srcDir = join(contentRoot, 'Pal', 'Texture', 'UI', 'Map');

const AREAS = [
	{ dir: 'mainmap', src: join(srcDir, 'T_WorldMap.png') },
	{ dir: 'tree', src: join(srcDir, 'T_TreeMap.png') }
];

let total = 0;
let bytes = 0;

for (const { dir, src } of AREAS) {
	const outRoot = join('psp-ui', 'static', 'maps', dir);
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
