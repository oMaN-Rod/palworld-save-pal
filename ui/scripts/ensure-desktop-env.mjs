// Writes ui/.env for desktop builds. Default mode only writes when absent, so
// `cargo tauri dev` works on a fresh clone without first running a *build*
// script to produce a *dev* input. `--force` overwrites unconditionally, which
// the desktop *build* (`build:desktop`) uses so a release can never bake a
// stale/web-mode value. .env is gitignored and generated everywhere (CI,
// Dockerfile, scripts/build-ui-desktop.*).
import { existsSync, writeFileSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';

const DESKTOP_ENV = 'PUBLIC_WS_URL=127.0.0.1:5174/ws\nPUBLIC_DESKTOP_MODE=true\n';
const force = process.argv.includes('--force');
const envPath = join(dirname(dirname(fileURLToPath(import.meta.url))), '.env');

if (!force && existsSync(envPath)) {
	console.log('[ensure-desktop-env] ui/.env exists, leaving it alone');
} else {
	writeFileSync(envPath, DESKTOP_ENV);
	console.log(`[ensure-desktop-env] wrote ui/.env for desktop${force ? ' (forced)' : ''}`);
}
