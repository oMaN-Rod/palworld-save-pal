// Writes psp-ui/.env for the web build: not desktop mode, so the browser build
// shows the upload route and hides desktop/native-only affordances. Force-writes
// unconditionally (mirrors ensure-desktop-env.mjs --force) so a web release can
// never bake in a stale desktop-mode value. .env is gitignored and regenerated
// by each build.
import { writeFileSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';

const WEB_ENV = 'PUBLIC_WS_URL=\nPUBLIC_DESKTOP_MODE=false\n';
const envPath = join(dirname(dirname(fileURLToPath(import.meta.url))), '.env');
writeFileSync(envPath, WEB_ENV);
console.log('[ensure-web-env] wrote psp-ui/.env for web (PUBLIC_DESKTOP_MODE=false)');
