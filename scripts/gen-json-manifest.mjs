import { cpSync, rmSync, readdirSync, writeFileSync, existsSync } from 'node:fs';
import { dirname, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

// Anchor to the repo root from this file's own location, so the result does not
// depend on the caller's working directory (bun runs scripts with cwd = ui/).
const repoRoot = resolve(dirname(fileURLToPath(import.meta.url)), '..');
const srcDir = resolve(repoRoot, 'data/json');
const destDir = resolve(repoRoot, 'ui/static/data/json');

if (existsSync(destDir)) rmSync(destDir, { recursive: true, force: true });
cpSync(srcDir, destDir, { recursive: true });

const files = readdirSync(destDir).filter((f) => f.endsWith('.json') && f !== 'manifest.json');
writeFileSync(resolve(destDir, 'manifest.json'), JSON.stringify(files, null, 0));
console.log(`web assets: ${files.length} json files + manifest → ui/static/data/json`);
