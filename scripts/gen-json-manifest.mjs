import { cpSync, rmSync, readdirSync, writeFileSync, existsSync, statSync } from 'node:fs';
import { dirname, resolve, join, relative } from 'node:path';
import { fileURLToPath } from 'node:url';

// Anchor to the repo root from this file's own location, so the result does not
// depend on the caller's working directory (bun runs scripts with cwd = psp-ui/).
const repoRoot = resolve(dirname(fileURLToPath(import.meta.url)), '..');
const srcDir = resolve(repoRoot, 'data/json');
const destDir = resolve(repoRoot, 'psp-ui/static/data/json');

if (existsSync(destDir)) rmSync(destDir, { recursive: true, force: true });
cpSync(srcDir, destDir, { recursive: true });

// Recurse the tree: GameData needs the l10n/ and ui/ subtrees, not just the
// top-level tables. Paths are relative to destDir with forward slashes; the
// worker fetches each and keys it extension-less to match GameData's keys.
function listJson(dir) {
	const out = [];
	for (const name of readdirSync(dir)) {
		const full = join(dir, name);
		if (statSync(full).isDirectory()) {
			out.push(...listJson(full));
		} else if (name.endsWith('.json') && name !== 'manifest.json') {
			out.push(relative(destDir, full).split('\\').join('/'));
		}
	}
	return out;
}

const files = listJson(destDir);
writeFileSync(resolve(destDir, 'manifest.json'), JSON.stringify(files, null, 0));
console.log(`web assets: ${files.length} json files + manifest → psp-ui/static/data/json`);
