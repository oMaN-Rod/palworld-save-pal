// Writes ui/.env for desktop dev if it is absent, so `cargo tauri dev` works on a
// fresh clone without first running a *build* script to produce a *dev* input.
// .env is gitignored and generated everywhere else too (CI, Dockerfile,
// scripts/build-ui-desktop.*). An existing file is never overwritten.
import { existsSync, writeFileSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';

const envPath = join(dirname(dirname(fileURLToPath(import.meta.url))), '.env');

if (existsSync(envPath)) {
	console.log('[ensure-desktop-env] ui/.env exists, leaving it alone');
} else {
	writeFileSync(envPath, 'PUBLIC_WS_URL=127.0.0.1:5174/ws\nPUBLIC_DESKTOP_MODE=true\n');
	console.log('[ensure-desktop-env] wrote ui/.env for desktop dev');
}
