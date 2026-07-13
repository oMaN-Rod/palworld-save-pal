// Regenerates data/json/l10n/<lang>/relics.json from the game's own UI text tables.
//
// Usage:
//   bun scripts/gen-relic-l10n.mjs <path-to-Exports/Pal/Content>
//
// Source per language:
//   L10N/<lang>/Pal/DataTable/Text/DT_UI_Common_Text_Common.json
// Rows BUILDUP_PLAYER_STATUS_NN (name) and BUILDUP_PLAYER_STATUS_DESC_NN (description),
// where NN is 00..12 -- the EPalRelicType enum value. That enum is dense 0..12 and its
// order matches RELIC_TYPE_MAP in psp-core/src/domain/relic.rs exactly, which is what
// lets us join text to key by index.
//
// Output shape matches the existing l10n convention (see work_suitability.json):
//   { "<relic_key>": { "localized_name": "...", "description": "..." } }

import { readFileSync, writeFileSync, mkdirSync } from 'node:fs';
import { join } from 'node:path';

const [, , contentRoot] = process.argv;
if (!contentRoot) {
	console.error('usage: bun scripts/gen-relic-l10n.mjs <path-to-Exports/Pal/Content>');
	process.exit(1);
}

// EPalRelicType order, index 0..12. Must stay in lockstep with RELIC_TYPE_MAP.
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

// [source dir in the game dump, destination dir under data/json/l10n].
//
// These are NOT always the same string. The game dumps Indonesian as `id`, but this
// repo's Indonesian l10n lives in `id-id` (there is also a stale `id/` directory that
// nothing reads). Writing to `id/` would silently leave Indonesian users with raw keys,
// so the destination is stated explicitly rather than inferred from the source.
const LANGS = [
	['de', 'de'],
	['en', 'en'],
	['es', 'es'],
	['es-MX', 'es-MX'],
	['fr', 'fr'],
	['id', 'id-id'],
	['it', 'it'],
	['ko', 'ko'],
	['pl', 'pl'],
	['pt-BR', 'pt-BR'],
	['ru', 'ru'],
	['th', 'th'],
	['tr', 'tr'],
	['vi', 'vi'],
	['zh-Hans', 'zh-Hans'],
	['zh-Hant', 'zh-Hant']
];

const text = (rows, key) => {
	const raw = rows[key]?.TextData?.LocalizedString;
	return typeof raw === 'string' ? raw.trim() : '';
};

for (const [sourceLang, destLang] of LANGS) {
	const src = join(
		contentRoot,
		'L10N', sourceLang, 'Pal', 'DataTable', 'Text', 'DT_UI_Common_Text_Common.json'
	);
	const rows = JSON.parse(readFileSync(src, 'utf8'))[0].Rows;

	const out = {};
	RELIC_KEYS.forEach((key, index) => {
		const nn = String(index).padStart(2, '0');
		const localized_name = text(rows, `BUILDUP_PLAYER_STATUS_${nn}`);
		const description = text(rows, `BUILDUP_PLAYER_STATUS_DESC_${nn}`);
		if (!localized_name) {
			throw new Error(`${sourceLang}: no BUILDUP_PLAYER_STATUS_${nn} for ${key}`);
		}
		out[key] = { localized_name, description };
	});

	const dir = join('data', 'json', 'l10n', destLang);
	mkdirSync(dir, { recursive: true });
	writeFileSync(join(dir, 'relics.json'), JSON.stringify(out, null, 2) + '\n');
	console.log(`${sourceLang} -> ${destLang}: ${Object.keys(out).length} relics`);
}
