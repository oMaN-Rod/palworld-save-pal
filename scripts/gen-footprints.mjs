// Bakes per-structure collision footprints out of the FModel blueprint dump
// so the world map can extrude each placed base structure at its true size.
//
// Usage:
//   bun scripts/gen-footprints.mjs <path-to-Exports/Pal/Content>
//
// Source: DT_MapObjectMasterDataTable (id -> blueprint), DT_BuildObjectDataTable
// (id -> TypeA category), and the BP_*.json blueprints under Blueprint/MapObject.
//
// Blueprints are spread across subdirectories - BuildObject/Furniture alone holds
// 201 - so the tree is walked recursively and indexed by filename stem.

import { readFileSync, readdirSync, writeFileSync } from 'node:fs';
import { join } from 'node:path';
import { fileURLToPath } from 'node:url';
import { DEFAULT_FOOTPRINT, blueprintStemFromAssetPath, parseBoxComponent } from './lib/footprints.mjs';

const [, , contentRoot] = process.argv;
if (!contentRoot) {
	console.error('usage: bun scripts/gen-footprints.mjs <path-to-Exports/Pal/Content>');
	process.exit(1);
}

const readTableRows = (path) => JSON.parse(readFileSync(path, 'utf8'))[0].Rows;

const indexBlueprints = (dir, index = new Map()) => {
	for (const entry of readdirSync(dir, { withFileTypes: true })) {
		const full = join(dir, entry.name);
		if (entry.isDirectory()) indexBlueprints(full, index);
		else if (entry.name.endsWith('.json')) index.set(entry.name.slice(0, -5), full);
	}
	return index;
};

const dataTableDir = join(contentRoot, 'Pal', 'DataTable', 'MapObject');
const master = readTableRows(join(dataTableDir, 'DT_MapObjectMasterDataTable.json'));
const buildObjects = readTableRows(join(dataTableDir, 'Building', 'DT_BuildObjectDataTable.json'));
const blueprints = indexBlueprints(join(contentRoot, 'Pal', 'Blueprint', 'MapObject'));

const categoryOf = (id) =>
	(buildObjects[id]?.TypeA ?? 'EPalBuildObjectTypeA::Other').split('::')[1];

const footprints = {};
const unresolvedMasterIds = [];
let defaulted = 0;

for (const [id, row] of Object.entries(master)) {
	const stem = blueprintStemFromAssetPath(row.BlueprintClassSoft?.AssetPathName);
	const path = stem && blueprints.get(stem);
	if (!path) {
		unresolvedMasterIds.push(id);
		continue;
	}

	const result = parseBoxComponent(JSON.parse(readFileSync(path, 'utf8')));
	if (!result) {
		unresolvedMasterIds.push(id);
		continue;
	}
	if (result.defaulted) defaulted += 1;

	footprints[id] = { ...result.box, typeA: categoryOf(id) };
}

// Build-table ids the loop above never resolved - either missing from the
// master table entirely, or present but with no blueprint/collision box.
for (const id of Object.keys(buildObjects)) {
	if (!footprints[id]) {
		footprints[id] = { ...DEFAULT_FOOTPRINT, typeA: categoryOf(id) };
		defaulted += 1;
	}
}

const skipped = unresolvedMasterIds.filter((id) => !footprints[id]).length;

const outPath = fileURLToPath(new URL('../data/json/map_object_footprints.json', import.meta.url));
writeFileSync(outPath, `${JSON.stringify(footprints, null, '\t')}\n`);
console.log(`wrote ${Object.keys(footprints).length} footprints to ${outPath}`);
console.log(`skipped ${skipped} ids with no blueprint or no collision box, ${defaulted} defaulted`);
