// Converts the game's 13 relic (buildup) icons to webp assets.
//
// Usage:
//   bun scripts/gen-relic-icons.mjs <path-to-Exports/Pal/Content>
//
// Source: Pal/Texture/UI/IngameMenu/Buildup/T_icon_Buildup_Player_NN.png, NN = 00..12.
// NN is the EPalRelicType enum value; that enum is dense 0..12 and its order matches
// RELIC_TYPE_MAP in psp-core/src/domain/relic.rs, which is how index joins to key.
//
// Output is named by relic KEY, not index, so the mapping is legible at the call site.
// Webp is mandatory: AssetLoader's glob only picks up **/*.webp.

import { readFileSync, writeFileSync } from 'node:fs';
import { join } from 'node:path';
import sharp from 'sharp';

const [, , contentRoot] = process.argv;
if (!contentRoot) {
	console.error('usage: bun scripts/gen-relic-icons.mjs <path-to-Exports/Pal/Content>');
	process.exit(1);
}

const RELIC_KEYS = [
	'capture_power',
	'hunger_reduction',
	'swim_speed',
	'food_decay_reduction',
	'jump_power',
	'glider_speed',
	'climb_speed',
	'status_ailment_resist',
	'stamina_reduction',
	'sphere_homing',
	'exp_bonus',
	'rainbow_passive_rate',
	'move_speed'
];

const srcDir = join(contentRoot, 'Pal', 'Texture', 'UI', 'IngameMenu', 'Buildup');
const outDir = join('ui', 'src', 'lib', 'assets', 'img');

for (const [index, key] of RELIC_KEYS.entries()) {
	const nn = String(index).padStart(2, '0');
	const src = join(srcDir, `T_icon_Buildup_Player_${nn}.png`);
	const out = join(outDir, `relic_${key}.webp`);
	const buffer = await sharp(readFileSync(src)).webp({ quality: 90 }).toBuffer();
	writeFileSync(out, buffer);
	console.log(`${nn} -> relic_${key}.webp (${buffer.length} bytes)`);
}
console.log(`wrote ${RELIC_KEYS.length} icons to ${outDir}`);
