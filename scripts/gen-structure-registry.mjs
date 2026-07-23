// scripts/gen-structure-registry.mjs
// Bakes the enriched per-type structure registry (collision box + category +
// archetype shape + material) from the FModel dump. Supersedes gen-footprints.mjs.
//
// Usage: bun scripts/gen-structure-registry.mjs <path-to-Exports/Pal/Content>

import { readFileSync, readdirSync, writeFileSync } from 'node:fs';
import { join } from 'node:path';
import { fileURLToPath } from 'node:url';
import { DEFAULT_FOOTPRINT, blueprintStemFromAssetPath, parseBoxComponent } from './lib/footprints.mjs';
import { classifyArchetype, detectMaterial } from './lib/structure-registry.mjs';

const [, , contentRoot] = process.argv;
if (!contentRoot) {
	console.error('usage: bun scripts/gen-structure-registry.mjs <path-to-Exports/Pal/Content>');
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

const staticMeshNames = (exports) =>
	exports
		.filter((e) => e.Type === 'StaticMeshComponent')
		.map((e) => e.Properties?.StaticMesh?.ObjectName ?? '')
		.filter(Boolean);

const dataTableDir = join(contentRoot, 'Pal', 'DataTable', 'MapObject');
const master = readTableRows(join(dataTableDir, 'DT_MapObjectMasterDataTable.json'));
const buildObjects = readTableRows(join(dataTableDir, 'Building', 'DT_BuildObjectDataTable.json'));
const blueprints = indexBlueprints(join(contentRoot, 'Pal', 'Blueprint', 'MapObject'));

const categoryOf = (id) => (buildObjects[id]?.TypeA ?? 'EPalBuildObjectTypeA::Other').split('::')[1];
const materialTypeOf = (id) => master[id]?.MaterialType;

const registry = {};
let defaulted = 0;

for (const [id, row] of Object.entries(master)) {
	const stem = blueprintStemFromAssetPath(row.BlueprintClassSoft?.AssetPathName);
	const path = stem && blueprints.get(stem);
	if (!path) continue;
	const exports = JSON.parse(readFileSync(path, 'utf8'));
	const result = parseBoxComponent(exports);
	if (!result) continue;
	if (result.defaulted) defaulted += 1;
	const typeA = categoryOf(id);
	registry[id] = {
		...result.box,
		typeA,
		archetype: classifyArchetype(id, staticMeshNames(exports), typeA),
		material: detectMaterial(id, materialTypeOf(id))
	};
}

for (const id of Object.keys(buildObjects)) {
	if (registry[id]) continue;
	const typeA = categoryOf(id);
	registry[id] = {
		...DEFAULT_FOOTPRINT,
		typeA,
		archetype: classifyArchetype(id, [], typeA),
		material: detectMaterial(id, materialTypeOf(id))
	};
	defaulted += 1;
}

const outPath = fileURLToPath(new URL('../data/json/map_object_footprints.json', import.meta.url));
writeFileSync(outPath, `${JSON.stringify(registry, null, '\t')}\n`);
console.log(`wrote ${Object.keys(registry).length} registry entries to ${outPath}`);
console.log(`${defaulted} defaulted (no collision box)`);
