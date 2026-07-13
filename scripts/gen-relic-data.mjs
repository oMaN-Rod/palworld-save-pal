// Regenerates data/json/relic_data.json from the game's own rank table.
//
// Usage:
//   bun scripts/gen-relic-data.mjs <path-to-DT_PlayerStatusRankMasterDataTable.json> [out]
//
// Source: FModel export of
//   Pal/Content/Pal/DataTable/Player/DT_PlayerStatusRankMasterDataTable.uasset
// Row struct `PalPlayerStatusRankMasterData`:
//   { RelicType, Rank, RequiredRelicNum, EffectRate, ResetRequiredMoney }
//
// `RequiredRelicNum` is the incremental relic cost of each rank, so a player's rank
// is the highest N whose cumulative cost they have paid. `EffectRate` is the stat
// bonus granted at that rank.

import { readFileSync, writeFileSync } from 'node:fs';

const [, , src, out = 'data/json/relic_data.json'] = process.argv;
if (!src) {
	console.error(
		'usage: bun scripts/gen-relic-data.mjs <DT_PlayerStatusRankMasterDataTable.json> [out]'
	);
	process.exit(1);
}

// EPalRelicType::CapturePower -> capture_power
const toKey = (relicType) =>
	relicType
		.replace(/^EPalRelicType::/, '')
		.replace(/([a-z0-9])([A-Z])/g, '$1_$2')
		.toLowerCase();

const rows = JSON.parse(readFileSync(src, 'utf8'))[0].Rows;

const byType = new Map();
for (const row of Object.values(rows)) {
	const key = toKey(row.RelicType);
	if (!byType.has(key)) byType.set(key, []);
	byType.get(key).push(row);
}

const result = {};
for (const [key, list] of byType) {
	list.sort((a, b) => a.Rank - b.Rank);

	// Ranks must be a dense 1..N sequence, or the cumulative walk is meaningless.
	list.forEach((row, i) => {
		if (row.Rank !== i + 1) {
			throw new Error(`${key}: expected rank ${i + 1}, got ${row.Rank}`);
		}
	});

	const per_rank = list.map((r) => r.RequiredRelicNum);
	result[key] = {
		cumulative_max: per_rank.reduce((a, b) => a + b, 0),
		max_rank: list.length,
		per_rank,
		effect_rate: list.map((r) => r.EffectRate)
	};
}

writeFileSync(out, JSON.stringify(result, null, 2) + '\n');
console.log(`wrote ${out}: ${Object.keys(result).length} relic types`);
