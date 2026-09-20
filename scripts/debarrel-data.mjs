// One-shot codemod: rewrite `import { x } from '$lib/data'` (the barrel) into
// direct `$lib/data/<module>` imports. Rollup warns — correctly — that
// re-exporting through the barrel while the data modules' own dependencies
// (websocket utils, states) reach back to the barrel creates cycles across
// chunk boundaries ("will likely lead to broken execution order"). Direct
// imports are the fix Rollup itself recommends. Run from ps-ui/: bun
// scripts/../../scripts/debarrel-data.mjs (or node); idempotent.
import { readFileSync, writeFileSync, readdirSync, statSync } from 'node:fs';
import { join, dirname, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

const uiRoot = resolve(dirname(fileURLToPath(import.meta.url)), '..', 'ps-ui');
const barrel = join(uiRoot, 'src/lib/data/index.ts');

// symbol -> subpath, parsed from the barrel's own imports.
const symbolToModule = new Map();
for (const match of readFileSync(barrel, 'utf8').matchAll(/import\s+(?:type\s+)?\{([^}]*)\}\s+from\s+'\.\/([^']+)'/g)) {
	for (const raw of match[1].split(',')) {
		const symbol = raw.trim().split(/\s+as\s+/)[0].trim();
		if (symbol) symbolToModule.set(symbol, match[2]);
	}
}

// Collect candidate files without a directory-walk dependency.
const files = [];
(function walk(dir) {
	for (const name of readdirSync(dir)) {
		const full = join(dir, name);
		if (statSync(full).isDirectory()) {
			if (name !== 'node_modules' && name !== '.svelte-kit' && name !== 'paraglide') walk(full);
		} else if (/\.(ts|svelte)$/.test(name)) {
			files.push(full);
		}
	}
})(join(uiRoot, 'src'));

const importRe = /import\s+(type\s+)?\{([^}]*)\}\s+from\s+'\$lib\/data';/gs;
let rewritten = 0;
let statements = 0;
for (const file of files) {
	const source = readFileSync(file, 'utf8');
	if (!source.includes("from '$lib/data'")) continue;
	let changed = false;
	const next = source.replace(importRe, (_all, typeKw, symbols) => {
		const groups = new Map();
		for (const raw of symbols.split(',')) {
			const item = raw.trim();
			if (!item) continue;
			const subpath = symbolToModule.get(item.split(/\s+as\s+/)[0].trim());
			if (!subpath) {
				throw new Error(`${file}: "${item.trim()}" is not exported by src/lib/data/index.ts`);
			}
			if (!groups.has(subpath)) groups.set(subpath, []);
			groups.get(subpath).push(item);
		}
		statements += 1;
		changed = true;
		return [...groups.entries()]
			.map(([subpath, items]) =>
				`import ${typeKw ?? ''}{ ${items.join(', ')} } from '$lib/data/${subpath}';`)
			.join('\n');
	});
	if (changed) {
		writeFileSync(file, next);
		rewritten += 1;
	}
}
console.log(`debarrel: rewrote ${statements} imports in ${rewritten} files (map: ${symbolToModule.size} symbols)`);
