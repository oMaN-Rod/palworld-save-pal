import { readdirSync, writeFileSync } from 'node:fs';
import { resolve } from 'node:path';

const jsonDir = resolve('data/json');
const files = readdirSync(jsonDir).filter((f) => f.endsWith('.json') && f !== 'manifest.json');
writeFileSync(resolve(jsonDir, 'manifest.json'), JSON.stringify(files, null, 0));
console.log(`manifest.json: ${files.length} files`);
